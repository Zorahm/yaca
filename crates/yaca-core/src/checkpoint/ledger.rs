//! The list of one session's points, kept on disk (§5.8).
//!
//! The shadow store keeps what the snapshots hold. It does not know which
//! snapshots are a session's, in which order they were taken, or why — that is
//! this file. Without it a reload, a closed and reopened chat or a restart
//! starts with an empty list, and every earlier point is content nobody can
//! name.
//!
//! One file per session, next to its journals:
//!
//! ```text
//! <chats>/<project>-<hash>/<id>.checkpoints.json
//! ```
//!
//! It is rewritten whole, aside and renamed, whenever the list grows: a point
//! is a few dozen bytes and the list is a few hundred at the most, so an append
//! log would buy nothing but a format to maintain.
//!
//! The file is also the root of the sweep ([`super::gc`]): a manifest named by
//! any session's file is live for as long as that session exists, whatever its
//! age. Deleting the session deletes the file, and the snapshots become
//! garbage like any other.
//!
//! A point whose manifest is no longer in the store is dropped on the way in.
//! The list must never offer a rollback that cannot work (§0), and the only way
//! to get there is someone emptying `.yaca/shadow` by hand.

use std::collections::HashSet;
use std::io;
use std::path::{Path, PathBuf};

use blake3::Hash;
use serde::{Deserialize, Serialize};

use super::store::Store;
use super::{CheckpointSummary, Record};
use crate::id::SessionId;

/// What follows the session id in the file's name. The sweep finds the files
/// of every session of a project by it.
const SUFFIX: &str = ".checkpoints.json";

/// `<id>.checkpoints.json` in the project's folder of journals `dir`
/// ([`crate::store::location`]).
pub fn path(dir: &Path, id: &SessionId) -> PathBuf {
    dir.join(format!("{id}{SUFFIX}"))
}

#[derive(Serialize, Deserialize)]
struct File {
    points: Vec<Point>,
}

#[derive(Serialize, Deserialize)]
struct Point {
    summary: CheckpointSummary,
    /// Hex name of the manifest in the store.
    manifest: String,
}

/// Every point the file names, oldest first. A missing file is no points; a
/// file that does not parse is no points either, and is said so in the log —
/// the session still opens, it just has nothing to go back to.
fn read(path: &Path) -> Vec<(CheckpointSummary, Hash)> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Vec::new(),
        Err(e) => {
            tracing::warn!(path = %path.display(), error = %e, "checkpoint list unreadable");
            return Vec::new();
        }
    };
    let file: File = match serde_json::from_str(&text) {
        Ok(file) => file,
        Err(e) => {
            tracing::warn!(path = %path.display(), error = %e, "checkpoint list does not parse");
            return Vec::new();
        }
    };
    file.points
        .into_iter()
        .filter_map(|p| Some((p.summary, Hash::from_hex(&p.manifest).ok()?)))
        .collect()
}

/// The points a session can still go back to, for the session itself.
pub(super) fn load(path: &Path, store: &Store) -> Vec<Record> {
    read(path)
        .into_iter()
        .filter(|(_, manifest)| store.has_manifest(manifest))
        .map(|(summary, manifest)| Record { summary, manifest })
        .collect()
}

/// The same points for the feed of a session that is not running.
///
/// `project` is the project's root, whose shadow store the manifests are
/// checked against; without it the file is taken as it stands. Nothing is
/// created: asking about a project whose `.yaca` is gone must not bring it
/// back.
pub fn alive(path: &Path, project: Option<&Path>) -> Vec<CheckpointSummary> {
    let points = read(path);
    let Some(project) = project else {
        return points.into_iter().map(|(summary, _)| summary).collect();
    };
    let store = Store::existing(project);
    points
        .into_iter()
        .filter(|(_, manifest)| store.as_ref().is_some_and(|s| s.has_manifest(manifest)))
        .map(|(summary, _)| summary)
        .collect()
}

/// Writes the list. Aside and renamed: a reader — or the next start after a
/// crash in the middle — must never find half a list under the real name.
pub(super) fn save(path: &Path, records: &[Record]) -> io::Result<()> {
    let file = File {
        points: records
            .iter()
            .map(|r| Point {
                summary: r.summary.clone(),
                manifest: r.manifest.to_hex().to_string(),
            })
            .collect(),
    };
    let text = serde_json::to_string(&file).map_err(io::Error::other)?;

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".tmp");
    let temp = path.with_file_name(name);
    std::fs::write(&temp, text)?;
    std::fs::rename(&temp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&temp);
    })
}

/// The manifests named by the files of every session in `dir`: what the sweep
/// must not take.
pub(super) fn manifests(dir: &Path) -> HashSet<Hash> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return HashSet::new();
    };
    entries
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().ends_with(SUFFIX))
        .flat_map(|e| read(&e.path()))
        .map(|(_, manifest)| manifest)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checkpoint::CheckpointReason;
    use crate::event::TsMillis;
    use crate::id::CheckpointId;
    use crate::testutil::TempDir;

    fn record(store: &Store, body: &str, reason: CheckpointReason) -> Record {
        Record {
            summary: CheckpointSummary {
                id: CheckpointId::new(),
                at: TsMillis(1_700_000_000_000),
                reason,
                tracked: 1,
            },
            manifest: store.put_manifest(body).unwrap(),
        }
    }

    #[test]
    fn a_saved_list_reads_back_in_order() {
        let dir = TempDir::new();
        let store = Store::open(dir.path()).unwrap();
        let file = path(dir.path(), &SessionId::from("ses_a"));
        let records = vec![
            record(&store, "one\n", CheckpointReason::Message),
            record(
                &store,
                "two\n",
                CheckpointReason::Command {
                    command: "cargo test".into(),
                },
            ),
        ];

        save(&file, &records).unwrap();

        let back = load(&file, &store);
        assert_eq!(back.len(), 2);
        assert_eq!(back[0].summary, records[0].summary);
        assert_eq!(back[1].summary, records[1].summary);
        assert_eq!(back[1].manifest, records[1].manifest);
    }

    #[test]
    fn a_point_whose_snapshot_is_gone_is_not_offered() {
        let dir = TempDir::new();
        let store = Store::open(dir.path()).unwrap();
        let file = path(dir.path(), &SessionId::from("ses_a"));
        let kept = record(&store, "kept\n", CheckpointReason::Message);
        let lost = record(&store, "lost\n", CheckpointReason::Rollback);
        save(&file, &[kept.clone(), lost.clone()]).unwrap();

        let hex = lost.manifest.to_hex();
        std::fs::remove_file(store.dir().join("manifests").join(&hex[..2]).join(&hex[2..]))
            .unwrap();

        let back = load(&file, &store);
        assert_eq!(back.len(), 1, "a rollback that cannot work is not a point");
        assert_eq!(back[0].summary, kept.summary);
        assert_eq!(alive(&file, Some(dir.path())), vec![kept.summary]);
    }

    #[test]
    fn no_file_and_a_broken_file_are_both_an_empty_list() {
        let dir = TempDir::new();
        let store = Store::open(dir.path()).unwrap();
        let file = path(dir.path(), &SessionId::from("ses_a"));
        assert!(load(&file, &store).is_empty());

        std::fs::write(&file, "{ not json").unwrap();
        assert!(load(&file, &store).is_empty());
        assert!(alive(&file, None).is_empty());
    }

    #[test]
    fn asking_about_a_project_without_a_store_creates_nothing() {
        let dir = TempDir::new();
        let file = path(dir.path(), &SessionId::from("ses_a"));
        std::fs::write(
            &file,
            r#"{"points":[{"summary":{"id":"ckpt_1","at":1,"reason":{"type":"message"},"tracked":1},"manifest":"0000000000000000000000000000000000000000000000000000000000000000"}]}"#,
        )
        .unwrap();

        assert!(alive(&file, Some(dir.path())).is_empty());
        assert!(!crate::project::dir(dir.path()).exists());
    }

    #[test]
    fn the_sweep_sees_the_manifests_of_every_session() {
        let dir = TempDir::new();
        let store = Store::open(dir.path()).unwrap();
        let one = record(&store, "one\n", CheckpointReason::Message);
        let two = record(&store, "two\n", CheckpointReason::Message);
        save(&path(dir.path(), &SessionId::from("ses_a")), &[one.clone()]).unwrap();
        save(&path(dir.path(), &SessionId::from("ses_b")), &[two.clone()]).unwrap();
        // Not a list, and not to be mistaken for one.
        std::fs::write(dir.path().join("ses_a.jsonl"), "{}\n").unwrap();

        let named = manifests(dir.path());
        assert_eq!(named, HashSet::from([one.manifest, two.manifest]));
    }

    #[test]
    fn saving_leaves_no_temporary_file_behind() {
        let dir = TempDir::new();
        let store = Store::open(dir.path()).unwrap();
        let file = path(dir.path(), &SessionId::from("ses_a"));
        save(&file, &[record(&store, "x\n", CheckpointReason::Message)]).unwrap();
        save(&file, &[record(&store, "y\n", CheckpointReason::Message)]).unwrap();

        let names: Vec<String> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.contains("ses_a"))
            .collect();
        assert_eq!(names, vec!["ses_a.checkpoints.json".to_string()]);
    }
}
