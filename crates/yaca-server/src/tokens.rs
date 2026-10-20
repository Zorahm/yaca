//! Who may connect.
//!
//! A token per device, so a lost phone is revoked without logging out the
//! laptop. Only the SHA-256 of a token is kept: the file can be read by
//! whoever can read the server's home directory, and a token that could be
//! lifted from it would be a password stored in plain sight. Tokens are long
//! random strings, not human passwords, so a plain hash is enough — there is
//! nothing to brute-force.
//!
//! The file is read on every check. Revoking is then a command that takes
//! effect at once, with no restart and no signal to send.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const FILE: &str = "server.json";
const PREFIX: &str = "yct_";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenRecord {
    pub name: String,
    /// Hex SHA-256 of the token.
    pub hash: String,
    /// Seconds since the epoch.
    pub created: u64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Store {
    tokens: Vec<TokenRecord>,
}

pub struct Tokens {
    path: PathBuf,
}

/// Read-modify-write of the file, one at a time within a process. The command
/// line is a human typing one command, so two processes racing is not a case
/// worth a lock file; two threads of one process (the tests) were.
static WRITING: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn hash(token: &str) -> String {
    Sha256::digest(token.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Compares without stopping at the first difference, so the time a check
/// takes says nothing about how much of a guess was right.
fn same(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

impl Tokens {
    pub fn in_dir(dir: &Path) -> Tokens {
        Tokens {
            path: dir.join(FILE),
        }
    }

    fn load(&self) -> Result<Store> {
        match fs::read_to_string(&self.path) {
            Ok(text) => serde_json::from_str(&text)
                .with_context(|| format!("{} does not parse", self.path.display())),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Store::default()),
            Err(e) => Err(e).with_context(|| format!("could not read {}", self.path.display())),
        }
    }

    fn save(&self, store: &Store) -> Result<()> {
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir)?;
        }
        let text = serde_json::to_string_pretty(store)?;
        // A name of its own: two `token add` at once must not rename each
        // other's half-written file away.
        let temp = self.path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4().simple()));
        write_private(&temp, text.as_bytes())?;
        fs::rename(&temp, &self.path)?;
        Ok(())
    }

    /// Makes a token for `name` and returns it. This is the only time it exists
    /// in the clear.
    pub fn add(&self, name: &str) -> Result<String> {
        let _writing = WRITING.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let name = name.trim();
        if name.is_empty() || name.chars().any(char::is_control) {
            bail!("a token needs a name, like \"phone\"");
        }
        let mut store = self.load()?;
        if store.tokens.iter().any(|t| t.name == name) {
            bail!("there is already a token named \"{name}\"; revoke it first");
        }
        // Two v4 UUIDs: 244 random bits from the OS generator.
        let token = format!(
            "{PREFIX}{}{}",
            uuid::Uuid::new_v4().simple(),
            uuid::Uuid::new_v4().simple()
        );
        let created = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        store.tokens.push(TokenRecord {
            name: name.to_string(),
            hash: hash(&token),
            created,
        });
        self.save(&store)?;
        Ok(token)
    }

    pub fn list(&self) -> Result<Vec<TokenRecord>> {
        Ok(self.load()?.tokens)
    }

    pub fn revoke(&self, name: &str) -> Result<bool> {
        let _writing = WRITING.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut store = self.load()?;
        let before = store.tokens.len();
        store.tokens.retain(|t| t.name != name);
        let found = store.tokens.len() != before;
        if found {
            self.save(&store)?;
        }
        Ok(found)
    }

    /// The name of the device the token belongs to, if it is a live one.
    pub fn check(&self, token: &str) -> Option<String> {
        let wanted = hash(token);
        // A file that cannot be read admits nobody: failing open here would be
        // the one way this check could make things worse.
        let store = self.load().ok()?;
        store
            .tokens
            .into_iter()
            .find(|t| same(&t.hash, &wanted))
            .map(|t| t.name)
    }
}

#[cfg(unix)]
fn write_private(path: &Path, bytes: &[u8]) -> io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(bytes)
}

#[cfg(not(unix))]
fn write_private(path: &Path, bytes: &[u8]) -> io::Result<()> {
    fs::write(path, bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp() -> PathBuf {
        use std::sync::atomic::{AtomicU32, Ordering};
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "yaca-tokens-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_token_admits_its_device_and_nobody_else() {
        let dir = temp();
        let tokens = Tokens::in_dir(&dir);
        let phone = tokens.add("phone").unwrap();
        let laptop = tokens.add("laptop").unwrap();

        assert_eq!(tokens.check(&phone).as_deref(), Some("phone"));
        assert_eq!(tokens.check(&laptop).as_deref(), Some("laptop"));
        assert_eq!(tokens.check("yct_wrong"), None);
        assert_eq!(tokens.check(""), None);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_file_holds_a_hash_and_not_the_token() {
        let dir = temp();
        let tokens = Tokens::in_dir(&dir);
        let token = tokens.add("phone").unwrap();
        let text = fs::read_to_string(dir.join(FILE)).unwrap();
        assert!(!text.contains(&token));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn revoking_takes_effect_on_the_next_check() {
        let dir = temp();
        let tokens = Tokens::in_dir(&dir);
        let token = tokens.add("phone").unwrap();
        assert!(tokens.revoke("phone").unwrap());
        assert_eq!(tokens.check(&token), None);
        assert!(!tokens.revoke("phone").unwrap());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_name_is_taken_once_and_must_be_a_name() {
        let dir = temp();
        let tokens = Tokens::in_dir(&dir);
        tokens.add("phone").unwrap();
        assert!(tokens.add("phone").is_err());
        assert!(tokens.add("  ").is_err());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn the_file_is_private() {
        use std::os::unix::fs::PermissionsExt;
        let dir = temp();
        Tokens::in_dir(&dir).add("phone").unwrap();
        let mode = fs::metadata(dir.join(FILE)).unwrap().permissions().mode();
        assert_eq!(mode & 0o077, 0, "mode {mode:o}");
        fs::remove_dir_all(&dir).unwrap();
    }
}
