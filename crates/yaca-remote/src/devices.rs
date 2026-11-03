//! The phones allowed to connect, and the proof each one carries.
//!
//! A device gets a random token once, when it pairs, and keeps it. The host
//! stores only the SHA-256 of that token, so a copied `devices.json` does not
//! open the host to anyone: a token cannot be read back out of its hash.
//!
//! A list that cannot be read is empty, which means no device verifies. That is
//! the fail-closed answer: a corrupt file must lock the phones out, never let an
//! unknown one in.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::Error;
use crate::files::{epoch_ms, load_json_or_default, save_json};
use crate::protocol::DeviceId;
use crate::secret::{constant_time_eq, random_token, sha256_hex};

const FILE: &str = "devices.json";
/// 256 bits: a token that a guesser cannot enumerate in any useful time.
const TOKEN_BYTES: usize = 32;
const MAX_NAME_CHARS: usize = 64;
const UNNAMED: &str = "Unnamed device";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredDevice {
    id: DeviceId,
    name: String,
    token_hash: String,
    paired_at: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_seen: Option<u64>,
}

/// The paired devices of one host.
///
/// The file is the only copy: every call reads it and every change writes it
/// back, under one lock per process. Two hosts can share the directory (the
/// desktop and `yaca serve`), and a list cached in one of them would let its
/// next write bring back a device the other had just revoked.
pub struct Devices {
    dir: PathBuf,
    lock: Mutex<()>,
}

impl Devices {
    pub fn load(dir: &Path) -> Self {
        Self {
            dir: dir.to_path_buf(),
            lock: Mutex::new(()),
        }
    }

    /// Pairs a new device and returns its id and its token. The token is not
    /// stored anywhere, so this is the only time it exists in readable form.
    pub fn register(&self, name: &str) -> Result<(DeviceId, String), Error> {
        let id = DeviceId(uuid::Uuid::new_v4().simple().to_string());
        let token = random_token(TOKEN_BYTES)?;
        let device = StoredDevice {
            id: id.clone(),
            name: clean_name(name),
            token_hash: sha256_hex(token.as_bytes()),
            paired_at: epoch_ms(SystemTime::now()),
            last_seen: None,
        };

        let _guard = self.lock();
        let mut stored = self.read();
        stored.push(device);
        self.save(&stored)?;
        Ok((id, token))
    }

    /// Whether `token` is the one `id` was paired with. A device that was
    /// revoked is absent, so it fails the same way an unknown id does.
    pub fn verify(&self, id: &DeviceId, token: &str) -> bool {
        let presented = sha256_hex(token.as_bytes());
        let _guard = self.lock();
        let mut stored = self.read();
        let Some(index) = stored.iter().position(|d| d.id == *id) else {
            return false;
        };
        if !constant_time_eq(stored[index].token_hash.as_bytes(), presented.as_bytes()) {
            return false;
        }

        stored[index].last_seen = Some(epoch_ms(SystemTime::now()));
        // A failed write only loses the "last seen" time, not the access; the
        // next successful verify tries again.
        if let Err(e) = self.save(&stored) {
            tracing::warn!(error = %e, "could not save the device's last-seen time");
        }
        true
    }

    pub fn list(&self) -> Vec<DeviceSummary> {
        let _guard = self.lock();
        self.read()
            .iter()
            .map(|d| DeviceSummary {
                id: d.id.0.clone(),
                name: d.name.clone(),
                paired_at: d.paired_at,
                last_seen: d.last_seen,
            })
            .collect()
    }

    /// Whether `id` is paired right now.
    pub fn contains(&self, id: &DeviceId) -> bool {
        let _guard = self.lock();
        self.read().iter().any(|d| d.id == *id)
    }

    /// Removes a device; its token stops verifying at once. `false` when there
    /// was no such device, which is not an error: the answer is the same either
    /// way.
    pub fn revoke(&self, id: &DeviceId) -> Result<bool, Error> {
        let _guard = self.lock();
        let stored = self.read();
        if !stored.iter().any(|d| d.id == *id) {
            return Ok(false);
        }
        let next: Vec<StoredDevice> = stored.into_iter().filter(|d| d.id != *id).collect();
        self.save(&next)?;
        Ok(true)
    }

    fn read(&self) -> Vec<StoredDevice> {
        load_json_or_default(&self.dir.join(FILE))
    }

    fn save(&self, devices: &[StoredDevice]) -> Result<(), Error> {
        save_json(&self.dir.join(FILE), devices)
    }

    fn lock(&self) -> MutexGuard<'_, ()> {
        // Guards no data, only the read-modify-write order, so a poisoned lock
        // leaves nothing half-done behind it.
        self.lock.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// A device as the host lists it. Never carries the token or its hash.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DeviceSummary {
    pub id: String,
    pub name: String,
    #[ts(type = "number")]
    pub paired_at: u64,
    #[ts(type = "number | null")]
    pub last_seen: Option<u64>,
}

/// The name as the phone sent it, trimmed and capped. The host shows it in its
/// own list, so an empty or huge one would make that list unreadable.
fn clean_name(name: &str) -> String {
    let cut: String = name.trim().chars().take(MAX_NAME_CHARS).collect();
    let cut = cut.trim_end();
    if cut.is_empty() {
        UNNAMED.to_string()
    } else {
        cut.to_string()
    }
}

#[cfg(test)]
mod tests {
    use std::time::UNIX_EPOCH;

    use super::*;
    use crate::files::TestDir;

    #[test]
    fn register_then_verify_with_the_token() {
        let dir = TestDir::new();
        let devices = Devices::load(dir.path());
        let (id, token) = devices.register("Pixel 8").unwrap();
        assert_eq!(id.0.len(), 32);
        assert!(devices.verify(&id, &token));
    }

    #[test]
    fn wrong_token_and_wrong_id_are_refused() {
        let dir = TestDir::new();
        let devices = Devices::load(dir.path());
        let (id, token) = devices.register("Pixel 8").unwrap();
        assert!(!devices.verify(&id, "not the token"));
        assert!(!devices.verify(&id, ""));
        assert!(!devices.verify(&DeviceId("unknown".into()), &token));
    }

    #[test]
    fn tokens_and_ids_are_unique_per_device() {
        let dir = TestDir::new();
        let devices = Devices::load(dir.path());
        let (a, ta) = devices.register("a").unwrap();
        let (b, tb) = devices.register("b").unwrap();
        assert_ne!(a, b);
        assert_ne!(ta, tb);
        assert_eq!(ta.len(), 43); // 32 bytes in base64url, unpadded
        assert!(!devices.verify(&a, &tb));
    }

    #[test]
    fn revoke_stops_the_token_at_once_and_reports_absence() {
        let dir = TestDir::new();
        let devices = Devices::load(dir.path());
        let (id, token) = devices.register("Pixel 8").unwrap();
        assert!(devices.revoke(&id).unwrap());
        assert!(!devices.verify(&id, &token));
        assert!(!devices.revoke(&id).unwrap());
        assert!(devices.list().is_empty());
    }

    #[test]
    fn list_shows_names_and_no_secrets() {
        let dir = TestDir::new();
        let devices = Devices::load(dir.path());
        let (id, token) = devices.register("Pixel 8").unwrap();
        let listed = devices.list();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, id.0);
        assert_eq!(listed[0].name, "Pixel 8");
        assert!(listed[0].paired_at > 0);
        assert_eq!(listed[0].last_seen, None);
        let text = serde_json::to_string(&listed).unwrap();
        assert!(!text.contains(&token));
    }

    #[test]
    fn verify_sets_last_seen() {
        let dir = TestDir::new();
        let devices = Devices::load(dir.path());
        let (id, token) = devices.register("Pixel 8").unwrap();
        assert!(devices.verify(&id, &token));
        assert!(devices.list()[0].last_seen.is_some());
    }

    #[test]
    fn devices_persist_across_load_and_the_file_holds_no_token() {
        let dir = TestDir::new();
        let (id, token) = {
            let devices = Devices::load(dir.path());
            devices.register("Pixel 8").unwrap()
        };
        let text = std::fs::read_to_string(dir.path().join(FILE)).unwrap();
        assert!(
            !text.contains(&token),
            "devices.json must not contain the token"
        );
        assert!(text.contains("tokenHash"));

        let reloaded = Devices::load(dir.path());
        assert!(reloaded.verify(&id, &token));
        assert_eq!(reloaded.list()[0].name, "Pixel 8");
    }

    #[test]
    fn broken_file_loads_as_empty_and_refuses_everyone() {
        let dir = TestDir::new();
        std::fs::create_dir_all(dir.path()).unwrap();
        std::fs::write(dir.path().join(FILE), b"[{ broken").unwrap();
        let devices = Devices::load(dir.path());
        assert!(devices.list().is_empty());
        assert!(!devices.verify(&DeviceId("anything".into()), "anything"));
    }

    #[test]
    fn missing_file_loads_as_empty() {
        let dir = TestDir::new();
        assert!(Devices::load(dir.path()).list().is_empty());
    }

    #[test]
    fn names_are_trimmed_capped_and_defaulted() {
        let dir = TestDir::new();
        let devices = Devices::load(dir.path());
        devices.register("   ").unwrap();
        devices.register("  Pixel  ").unwrap();
        devices.register(&"x".repeat(100)).unwrap();
        let names: Vec<String> = devices.list().into_iter().map(|d| d.name).collect();
        assert_eq!(names[0], UNNAMED);
        assert_eq!(names[1], "Pixel");
        assert_eq!(names[2].chars().count(), MAX_NAME_CHARS);
    }

    #[test]
    fn name_cap_counts_characters_not_bytes() {
        let name = "ж".repeat(70);
        let cleaned = clean_name(&name);
        assert_eq!(cleaned.chars().count(), MAX_NAME_CHARS);
    }

    #[test]
    fn constant_time_eq_matches_plain_equality() {
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abd"));
        assert!(!constant_time_eq(b"abc", b"ab"));
        assert!(!constant_time_eq(b"", b"x"));
        assert!(constant_time_eq(b"", b""));
    }

    #[test]
    fn random_token_is_base64url_of_the_requested_size() {
        let token = random_token(16).unwrap();
        assert_eq!(token.len(), 22);
        assert!(token
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
        assert_ne!(token, random_token(16).unwrap());
    }

    #[test]
    fn epoch_ms_counts_from_1970() {
        let t = UNIX_EPOCH + std::time::Duration::from_millis(1_234);
        assert_eq!(epoch_ms(t), 1_234);
        assert_eq!(epoch_ms(UNIX_EPOCH - std::time::Duration::from_secs(1)), 0);
    }
}
