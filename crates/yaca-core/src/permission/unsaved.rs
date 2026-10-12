//! What a deletion takes away that no snapshot keeps (§5.7, §5.8).
//!
//! One rule for both deletion tools. The snapshot walk leaves out directories
//! by name (the built-in list and the project's own `shadow.exclude`), does
//! not follow links, skips names it cannot spell, and skips files over
//! [`MAX_FILE_BYTES`]. A rollback restores what the snapshot saved, so
//! deleting any of those is a loss no rollback undoes, and the question says
//! so before it happens.

use std::io;
use std::path::Path;

use crate::checkpoint::scan::{excluded_dirs, MAX_FILE_BYTES};
use crate::workspace::Workspace;

use super::Unsaved;

/// How many skipped directories the question names. The rest are counted.
pub const LISTED_DIRS: usize = 5;

/// What an action in additional roots loses for good, whatever it is.
///
/// The checkpoints snapshot the primary root and nothing else, so a write,
/// delete or move that lands in an additional root is not covered by any
/// rollback. Every such action is reported this way. `None` when none of the
/// paths is in an additional root.
pub fn outside_checkpoint<'a>(
    ws: &Workspace,
    paths: impl IntoIterator<Item = &'a Path>,
) -> Option<Unsaved> {
    let mut roots: Vec<String> = Vec::new();
    for path in paths {
        if ws.in_additional(path) {
            let text = crate::workspace::roots::plain(ws.root_for(path));
            if !roots.contains(&text) {
                roots.push(text);
            }
        }
    }
    (!roots.is_empty()).then_some(Unsaved {
        dirs: Vec::new(),
        dir_count: 0,
        files: 0,
        roots,
    })
}

/// A shell command may reach any additional root, and nothing in the command
/// line says which it reaches: a `cd` or an absolute path can be built at run
/// time. So a command that is not read-only is reported with every additional
/// root it could touch. `None` when there are none.
pub fn command_reach(ws: &Workspace) -> Option<Unsaved> {
    let roots = ws.additional_text();
    (!roots.is_empty()).then_some(Unsaved {
        dirs: Vec::new(),
        dir_count: 0,
        files: 0,
        roots,
    })
}

/// Whether the snapshot walk leaves out a directory called `name`.
pub fn skipped_name(ws: &Workspace, name: &str) -> bool {
    excluded_dirs().contains(&name) || ws.shadow_exclude().iter().any(|e| e == name)
}

/// Whether the walk never enters `path`: a component of it, below the project
/// root, is a skipped name. The directory itself counts, so `target/debug` is
/// under `target`.
fn under_skipped(ws: &Workspace, path: &Path) -> bool {
    path.strip_prefix(ws.root()).is_ok_and(|rel| {
        rel.components()
            .any(|c| c.as_os_str().to_str().is_some_and(|n| skipped_name(ws, n)))
    })
}

/// A name the manifest can spell (§5.8). A name with a newline has no line in
/// the format, so the snapshot never holds it.
fn spellable(ws: &Workspace, path: &Path) -> bool {
    path.strip_prefix(ws.root())
        .ok()
        .and_then(|rel| rel.to_str())
        .is_some_and(|rel| !rel.contains(['\n', '\r']))
}

/// What deleting the file `path` loses, or `None` when a snapshot holds it.
///
/// A file inside a skipped directory is lost although it is one file: the
/// question counts the file, not the directory it sits in.
pub fn of_file(ws: &Workspace, path: &Path) -> Option<Unsaved> {
    let meta = std::fs::symlink_metadata(path).ok()?;
    if meta.is_dir() {
        return None;
    }
    if let Some(outside) = outside_checkpoint(ws, [path]) {
        return Some(outside);
    }
    let untracked = path.parent().is_some_and(|dir| under_skipped(ws, dir))
        || !meta.is_file()
        || !spellable(ws, path)
        || meta.len() > MAX_FILE_BYTES;
    untracked.then_some(Unsaved {
        dirs: Vec::new(),
        dir_count: 0,
        files: 1,
        roots: Vec::new(),
    })
}

/// What deleting the directory `dir` takes away that no snapshot holds. `None`
/// when all of it is saved.
///
/// Walks the way the snapshot walks, and does not enter a skipped directory:
/// what is in `node_modules` is already known to be unsaved, and entering it
/// would walk the very tree the skip exists to avoid. A directory that sits
/// inside a skipped one is unsaved as a whole, because the snapshot never
/// looked in there at all.
pub fn of_dir(ws: &Workspace, dir: &Path) -> io::Result<Option<Unsaved>> {
    if let Some(outside) = outside_checkpoint(ws, [dir]) {
        return Ok(Some(outside));
    }
    if under_skipped(ws, dir) {
        return Ok(Some(Unsaved {
            dirs: vec![ws.display(dir)],
            dir_count: 1,
            files: 0,
            roots: Vec::new(),
        }));
    }

    let mut out = Unsaved {
        dirs: Vec::new(),
        dir_count: 0,
        files: 0,
        roots: Vec::new(),
    };
    let mut pending = vec![dir.to_path_buf()];
    while let Some(current) = pending.pop() {
        for entry in std::fs::read_dir(&current)? {
            let entry = entry?;
            let path = entry.path();
            let kind = entry.file_type()?;

            if kind.is_dir() {
                let name = entry.file_name();
                if name.to_str().is_some_and(|n| skipped_name(ws, n)) {
                    out.dir_count += 1;
                    if out.dirs.len() < LISTED_DIRS {
                        out.dirs.push(ws.display(&path));
                    }
                } else {
                    pending.push(path);
                }
                continue;
            }

            // A link is not followed, so nothing holds it. Neither does a name
            // the snapshot cannot spell, nor a file over its size limit.
            let untracked = !kind.is_file()
                || !spellable(ws, &path)
                || entry.metadata()?.len() > MAX_FILE_BYTES;
            if untracked {
                out.files += 1;
            }
        }
    }

    Ok((out.dir_count > 0 || out.files > 0).then_some(out))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checkpoint::scan::MAX_FILE_BYTES;
    use crate::settings::Settings;
    use crate::testutil::TempDir;
    use crate::workspace::roots;

    /// A workspace the way a session opens one: the project's own settings
    /// file is read, and its `shadow.exclude` goes into the workspace.
    fn project_with_settings(dir: &TempDir, json: &str) -> Workspace {
        dir.write(".yaca/settings.json", json);
        let root = Workspace::open(dir.path()).unwrap();
        let settings = Settings::load(root.root()).unwrap();
        root.with_shadow_exclude(&settings.shadow.exclude)
    }

    fn make_file_link(target: &std::path::Path, link: &std::path::Path) -> std::io::Result<()> {
        #[cfg(unix)]
        return std::os::unix::fs::symlink(target, link);
        #[cfg(windows)]
        return std::os::windows::fs::symlink_file(target, link);
    }

    fn big_file(dir: &TempDir, rel: &str) -> std::path::PathBuf {
        dir.write(rel, &"x".repeat(MAX_FILE_BYTES as usize + 1))
    }

    #[test]
    fn a_project_exclusion_from_settings_makes_its_directory_unsaved() {
        let dir = TempDir::new();
        dir.write("fixtures/big/a.json", "{}\n");
        let ws = project_with_settings(&dir, r#"{ "shadow": { "exclude": ["fixtures"] } }"#);

        let whole = of_dir(&ws, &dir.path().join("fixtures")).unwrap().unwrap();
        assert_eq!(whole.dirs, vec!["fixtures".to_string()]);
        assert_eq!(whole.dir_count, 1);

        // A file inside it is lost with it, and the question counts the file.
        let inside = of_file(&ws, &dir.path().join("fixtures/big/a.json")).unwrap();
        assert_eq!(inside.files, 1);
        assert_eq!(inside.dir_count, 0);
    }

    #[test]
    fn without_the_setting_the_same_directory_is_saved() {
        // The built-in list does not know `fixtures`; the snapshot keeps it.
        let dir = TempDir::new();
        dir.write("fixtures/a.json", "{}\n");
        let ws = Workspace::open(dir.path()).unwrap();
        assert_eq!(of_dir(&ws, &dir.path().join("fixtures")).unwrap(), None);
        assert_eq!(of_file(&ws, &dir.path().join("fixtures/a.json")), None);
    }

    #[test]
    fn a_blank_or_padded_exclusion_is_read_the_way_the_scanner_reads_it() {
        let dir = TempDir::new();
        dir.write("fixtures/a.json", "{}\n");
        let ws =
            project_with_settings(&dir, r#"{ "shadow": { "exclude": ["  ", " fixtures "] } }"#);
        assert_eq!(ws.shadow_exclude(), ["fixtures".to_string()]);
        assert!(of_file(&ws, &dir.path().join("fixtures/a.json")).is_some());
    }

    #[test]
    fn a_file_over_the_limit_is_unsaved_and_a_small_one_is_not() {
        let dir = TempDir::new();
        let big = big_file(&dir, "data/big.bin");
        let small = dir.write("data/small.txt", "x\n");
        let ws = Workspace::open(dir.path()).unwrap();
        assert_eq!(
            of_file(&ws, &big),
            Some(Unsaved {
                dirs: Vec::new(),
                dir_count: 0,
                files: 1,
                roots: Vec::new(),
            })
        );
        assert_eq!(of_file(&ws, &small), None);
    }

    #[test]
    fn a_link_is_unsaved_because_the_snapshot_does_not_keep_links() {
        let dir = TempDir::new();
        dir.write("real.txt", "x\n");
        let link = dir.path().join("alias.txt");
        make_file_link(&dir.path().join("real.txt"), &link)
            .expect("the test host can make a file link");
        let ws = Workspace::open(dir.path()).unwrap();
        assert!(of_file(&ws, &link).is_some());
    }

    #[test]
    fn a_directory_is_not_a_file_question() {
        let dir = TempDir::new();
        dir.write("a/b.txt", "x\n");
        let ws = Workspace::open(dir.path()).unwrap();
        assert_eq!(of_file(&ws, &dir.path().join("a")), None);
    }

    #[test]
    fn a_missing_file_asks_nothing() {
        let dir = TempDir::new();
        let ws = Workspace::open(dir.path()).unwrap();
        assert_eq!(of_file(&ws, &dir.path().join("nope.txt")), None);
    }

    /// A primary root with one file, and an additional root with one file.
    fn with_extra() -> (TempDir, TempDir, Workspace) {
        let main = TempDir::new();
        main.write("a.txt", "x\n");
        let extra = TempDir::new();
        extra.write("sub/b.txt", "x\n");
        let ws = Workspace::open(main.path())
            .unwrap()
            .with_roots(&[extra.path().to_path_buf()])
            .unwrap();
        (main, extra, ws)
    }

    #[test]
    fn a_file_in_an_additional_root_is_unsaved_as_a_whole() {
        let (_m, extra, ws) = with_extra();
        let file = ws.resolve_existing(&roots::plain(&extra.path().join("sub/b.txt"))).unwrap();
        let lost = of_file(&ws, &file).expect("the checkpoint never saw this root");
        assert_eq!(lost.roots, vec![roots::plain(extra.path())]);
        assert_eq!(lost.files, 0);
        assert!(lost.dirs.is_empty());
    }

    #[test]
    fn a_directory_in_an_additional_root_is_unsaved_as_a_whole() {
        let (_m, extra, ws) = with_extra();
        let dir = ws.resolve_existing(&roots::plain(&extra.path().join("sub"))).unwrap();
        let lost = of_dir(&ws, &dir).unwrap().expect("unsaved");
        assert_eq!(lost.roots, vec![roots::plain(extra.path())]);
    }

    #[test]
    fn a_write_to_the_primary_root_is_not_reported_as_unsaved() {
        let (_m, _e, ws) = with_extra();
        let inside = ws.resolve_new("a.txt").unwrap();
        assert_eq!(outside_checkpoint(&ws, [inside.as_path()]), None);
    }

    #[test]
    fn a_move_across_roots_names_each_root_once() {
        let (_m, extra, ws) = with_extra();
        let from = ws.resolve_new("a.txt").unwrap();
        let to = ws.resolve_new(&roots::plain(&extra.path().join("moved.txt"))).unwrap();
        let both = outside_checkpoint(&ws, [from.as_path(), to.as_path()]).unwrap();
        assert_eq!(both.roots, vec![roots::plain(extra.path())]);
        let twice = outside_checkpoint(&ws, [to.as_path(), to.as_path()]).unwrap();
        assert_eq!(twice.roots.len(), 1);
    }

    #[test]
    fn a_command_is_reported_with_every_additional_root_only_when_there_are_some() {
        let (_m, extra, ws) = with_extra();
        assert_eq!(command_reach(&ws).unwrap().roots, vec![roots::plain(extra.path())]);

        let plain = TempDir::new();
        let alone = Workspace::open(plain.path()).unwrap();
        assert_eq!(command_reach(&alone), None);
    }
}
