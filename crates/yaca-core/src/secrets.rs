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

/// The name the search key is stored under.
///
/// One spelling, not three: the tool reads it, the settings window writes it
/// and asks where it came from, and a literal `"tavily"` in each of those
/// would be three places to keep in step.
pub const TAVILY_ITEM: &str = "tavily";

/// The key Tavily's search API goes out with (§10).
///
/// Tavily is not a model provider, but the store is keyed by an arbitrary
/// item name and `yaca key set --provider <name>` works with any of them.
/// The alias exists so a search tool reads an intent, not the plumbing:
/// `api_key(TAVILY_ITEM)` in `tool/web_search.rs` would look like a provider
/// lookup and send a reader hunting through the provider registry.
pub fn tavily_key() -> Option<String> {
    api_key(TAVILY_ITEM).0
}

/// Puts the key in the platform's store, replacing whatever was there.
pub fn store_api_key(provider: &str, key: &str) -> Result<(), CoreError> {
    if key.trim().is_empty() {
        return Err(CoreError::Config("empty key cannot be saved".into()));
    }
    let _serial = serial();
    entry(provider)?
        .set_password(key)
        .map_err(|e| CoreError::Config(format!("cannot save key in keychain: {e}")))
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
            "cannot delete key from keychain: {e}"
        ))),
    }
}

/// How much of a secret one keychain entry holds. Windows keeps a credential
/// in at most 2560 bytes, as UTF-16: an OAuth grant with two JWTs in it is
/// longer, so a long secret is kept in parts.
const PART_CHARS: usize = 1000;

/// A secret other than a provider's key — an MCP server's OAuth grant — under
/// `account`. The first entry holds the number of parts and the first part;
/// the rest are `account#1`, `account#2`…
pub(crate) fn store_secret(account: &str, value: &str) -> Result<(), CoreError> {
    let chars: Vec<char> = value.chars().collect();
    let parts: Vec<String> = chars.chunks(PART_CHARS).map(|c| c.iter().collect()).collect();
    let parts = if parts.is_empty() { vec![String::new()] } else { parts };
    forget_secret(account);
    let _serial = serial();
    let set = |name: &str, text: &str| {
        keyring::Entry::new(SERVICE, name)
            .and_then(|entry| entry.set_password(text))
            .map_err(|e| CoreError::Config(format!("cannot save to keychain: {e}")))
    };
    set(account, &format!("{}|{}", parts.len(), parts[0]))?;
    for (at, part) in parts.iter().enumerate().skip(1) {
        set(&format!("{account}#{at}"), part)?;
    }
    Ok(())
}

/// The secret under `account`, put back together. `None` when there is none,
/// or a part of it is missing.
pub(crate) fn read_secret(account: &str) -> Option<String> {
    let _serial = serial();
    let get = |name: &str| keyring::Entry::new(SERVICE, name).ok()?.get_password().ok();
    let first = get(account)?;
    let (count, head) = first.split_once('|')?;
    let count: usize = count.parse().ok()?;
    let mut out = head.to_string();
    for at in 1..count {
        out.push_str(&get(&format!("{account}#{at}"))?);
    }
    Some(out)
}

/// Removes the secret under `account`, every part. Missing is not a failure.
pub(crate) fn forget_secret(account: &str) {
    let _serial = serial();
    let delete = |name: &str| {
        if let Ok(entry) = keyring::Entry::new(SERVICE, name) {
            let _ = entry.delete_credential();
        }
    };
    let count = keyring::Entry::new(SERVICE, account)
        .ok()
        .and_then(|e| e.get_password().ok())
        .and_then(|first| first.split_once('|').and_then(|(n, _)| n.parse::<usize>().ok()))
        .unwrap_or(1);
    delete(account);
    for at in 1..count {
        delete(&format!("{account}#{at}"));
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
        .map_err(|e| CoreError::Config(format!("keychain is unavailable: {e}")))
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
