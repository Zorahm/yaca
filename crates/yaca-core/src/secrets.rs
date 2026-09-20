//! Where the provider key comes from (§10).
//!
//! §10 puts keys in the system keychain, and that is the only place Yaca ever
//! writes one: Windows Credential Manager, the macOS Keychain, the Secret
//! Service on Linux. The environment variable stays as a fallback, because
//! `yaca-cli` and CI have no keychain to talk to and should not need one.
//!
//! The keychain is asked first. A person who typed their key into the settings
//! screen must see that key used; if a stale `ANTHROPIC_API_KEY` left over in a
//! shell profile quietly won, the screen would be lying about state (§0).
//! Which of the two answered is reported by [`key_source`], so the screen can
//! say where the key came from instead of leaving it to be guessed.
//!
//! Reading can fail — a Linux session with no secret service, a locked
//! keychain — and a failure is not an error here: it means there is no key in
//! that place, and the fallback is next. Writing failures are errors, because
//! a key the human believes they stored and which is not stored is exactly the
//! interface lying about state.
//!
//! Every call goes through one lock and brings a key left under the old name
//! over first — see [`crate::legacy`].

use std::sync::{Mutex, MutexGuard, PoisonError};

use crate::error::CoreError;
use crate::legacy;

/// The name Yaca keeps its secrets under in the platform's store.
pub const SERVICE: &str = "Yaca";

/// One keychain call at a time in this process.
///
/// The Windows Credential Manager loses an occasional write when several
/// arrive together, and the move from the old name writes on what looks like
/// a read: two panels asking for the key source at once would be two writes.
/// A key is touched a handful of times per launch, so waiting costs nothing.
pub(crate) fn serial() -> MutexGuard<'static, ()> {
    static KEYCHAIN: Mutex<()> = Mutex::new(());
    // Nothing is guarded but the order of calls, so a panic mid-call leaves
    // nothing half-done to protect.
    KEYCHAIN.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Which of the two places answered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeySource {
    Keychain,
    /// The provider's environment variable.
    Environment,
    None,
}

/// The environment variable a provider's key may also come from.
pub fn env_var(provider: &str) -> String {
    format!("{}_API_KEY", provider.to_uppercase().replace('-', "_"))
}

/// The key for a provider, and where it came from.
pub fn api_key(provider: &str) -> (Option<String>, KeySource) {
    resolve(from_keychain(provider), from_env(provider))
}

/// Where the key would come from, without carrying the key itself around.
pub fn key_source(provider: &str) -> KeySource {
    api_key(provider).1
}

/// Puts the key in the platform's store, replacing whatever was there.
pub fn store_api_key(provider: &str, key: &str) -> Result<(), CoreError> {
    if key.trim().is_empty() {
        return Err(CoreError::Config("пустой ключ не сохраняется".into()));
    }
    let _serial = serial();
    entry(provider)?
        .set_password(key)
        .map_err(|e| CoreError::Config(format!("не сохранить ключ в кейчейне: {e}")))
}

/// Removes the key. Missing is not a failure: the goal was that it be gone.
///
/// Under the old name too: [`entry`] brings it over first, so a key forgotten
/// here does not come back from where the edoC build kept it.
pub fn forget_api_key(provider: &str) -> Result<(), CoreError> {
    let _serial = serial();
    match entry(provider)?.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(CoreError::Config(format!(
            "не удалить ключ из кейчейна: {e}"
        ))),
    }
}

/// Which place wins, as a function of what each one holds.
///
/// Split out from the lookups so the rule can be read and tested without a
/// keychain in the room.
fn resolve(keychain: Option<String>, env: Option<String>) -> (Option<String>, KeySource) {
    match (keychain, env) {
        (Some(key), _) => (Some(key), KeySource::Keychain),
        (None, Some(key)) => (Some(key), KeySource::Environment),
        (None, None) => (None, KeySource::None),
    }
}

/// The provider's entry under the current name, with a key left under the old
/// one brought over first. Called with [`serial`] held.
fn entry(provider: &str) -> Result<keyring::Entry, CoreError> {
    legacy::adopt_key(legacy::NAME, SERVICE, provider);
    keyring::Entry::new(SERVICE, provider)
        .map_err(|e| CoreError::Config(format!("кейчейн недоступен: {e}")))
}

/// `None` for both "no such entry" and "no keychain at all": from here the two
/// are the same answer, and the caller has a fallback either way.
fn from_keychain(provider: &str) -> Option<String> {
    let _serial = serial();
    non_empty(entry(provider).ok()?.get_password().ok()?)
}

fn from_env(provider: &str) -> Option<String> {
    non_empty(std::env::var(env_var(provider)).ok()?)
}

fn non_empty(value: String) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_variable_is_named_after_the_provider() {
        assert_eq!(env_var("anthropic"), "ANTHROPIC_API_KEY");
        assert_eq!(env_var("open-router"), "OPEN_ROUTER_API_KEY");
    }

    #[test]
    fn the_keychain_wins_over_a_stale_variable() {
        // Otherwise a key typed into the settings screen appears to do
        // nothing, because a shell profile from a year ago outranks it.
        let (key, source) = resolve(Some("stored".into()), Some("stale".into()));
        assert_eq!(key.as_deref(), Some("stored"));
        assert_eq!(source, KeySource::Keychain);
    }

    #[test]
    fn the_variable_answers_when_the_keychain_has_nothing() {
        // `yaca-cli` and CI have no keychain, and should not need one.
        let (key, source) = resolve(None, Some("from-env".into()));
        assert_eq!(key.as_deref(), Some("from-env"));
        assert_eq!(source, KeySource::Environment);
    }

    #[test]
    fn nothing_anywhere_is_said_plainly() {
        assert_eq!(resolve(None, None), (None, KeySource::None));
    }

    #[test]
    fn an_empty_key_is_no_key() {
        // A variable set to "" is how a shell says "unset", and a keychain
        // entry cleared to blanks means the same.
        assert_eq!(non_empty("   ".into()), None);
        assert_eq!(non_empty(" sk-ant ".into()).as_deref(), Some("sk-ant"));
    }

    #[test]
    fn storing_an_empty_key_is_refused_rather_than_stored() {
        let err = store_api_key("yaca-test-never-used", "  ").unwrap_err();
        assert!(matches!(err, CoreError::Config(_)), "got {err:?}");
    }
}
