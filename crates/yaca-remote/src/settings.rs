//! The remote server's switch and port, kept in the remote directory next to
//! the certificate and the paired devices.
//!
//! The server is off by default: a host that never asked for a listening socket
//! must not get one. A broken settings file falls back to that default rather
//! than failing the application, because the window has to start either way and
//! a silently opened port would be worse than a closed one.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::Error;
use crate::files::{load_json_or_default, save_json};

/// The port the phone is told to connect to when the human does not pick one.
/// Outside the IANA well-known and common service ranges, so it rarely clashes
/// with a local tool.
pub const DEFAULT_PORT: u16 = 47820;

const FILE: &str = "settings.json";

/// `<config_dir>/remote`: where the certificate, the devices and these settings
/// live.
pub fn remote_dir(config_dir: &Path) -> PathBuf {
    config_dir.join("remote")
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RemoteSettings {
    pub enabled: bool,
    pub port: u16,
}

impl Default for RemoteSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            port: DEFAULT_PORT,
        }
    }
}

impl RemoteSettings {
    pub fn load(dir: &Path) -> Self {
        load_json_or_default(&dir.join(FILE))
    }

    pub fn save(&self, dir: &Path) -> Result<(), Error> {
        save_json(&dir.join(FILE), self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::files::TestDir;
    #[cfg(unix)]
    use crate::files::write_atomic;

    #[test]
    fn default_is_disabled_on_the_default_port() {
        let s = RemoteSettings::default();
        assert!(!s.enabled);
        assert_eq!(s.port, DEFAULT_PORT);
        assert_eq!(DEFAULT_PORT, 47820);
    }

    #[test]
    fn remote_dir_is_a_child_of_the_config_dir() {
        assert_eq!(remote_dir(Path::new("/x/yaca")), Path::new("/x/yaca").join("remote"));
    }

    #[test]
    fn missing_file_loads_defaults() {
        let dir = TestDir::new();
        assert_eq!(RemoteSettings::load(dir.path()), RemoteSettings::default());
    }

    #[test]
    fn save_then_load_round_trips_and_writes_camel_case() {
        let dir = TestDir::new();
        let settings = RemoteSettings {
            enabled: true,
            port: 50001,
        };
        settings.save(dir.path()).unwrap();
        assert_eq!(RemoteSettings::load(dir.path()), settings);

        let text = std::fs::read_to_string(dir.path().join(FILE)).unwrap();
        assert!(text.contains("\"enabled\""));
        assert!(text.contains("\"port\""));
    }

    #[test]
    fn save_creates_the_directory_and_leaves_no_temporary_file() {
        let dir = TestDir::new();
        let nested = dir.path().join("remote");
        RemoteSettings::default().save(&nested).unwrap();

        let names: Vec<String> = std::fs::read_dir(&nested)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec![FILE.to_string()]);
    }

    #[test]
    fn broken_file_loads_defaults() {
        let dir = TestDir::new();
        std::fs::create_dir_all(dir.path()).unwrap();
        std::fs::write(dir.path().join(FILE), b"{ not json").unwrap();
        assert_eq!(RemoteSettings::load(dir.path()), RemoteSettings::default());
    }

    #[test]
    fn save_replaces_an_existing_file() {
        let dir = TestDir::new();
        RemoteSettings {
            enabled: true,
            port: 1,
        }
        .save(dir.path())
        .unwrap();
        RemoteSettings {
            enabled: false,
            port: 2,
        }
        .save(dir.path())
        .unwrap();
        assert_eq!(
            RemoteSettings::load(dir.path()),
            RemoteSettings {
                enabled: false,
                port: 2
            }
        );
    }

    #[cfg(unix)]
    #[test]
    fn atomic_write_is_private_on_unix() {
        use std::os::unix::fs::PermissionsExt;
        let dir = TestDir::new();
        write_atomic(&dir.path().join("secret"), b"x").unwrap();
        let mode = std::fs::metadata(dir.path().join("secret")).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}
