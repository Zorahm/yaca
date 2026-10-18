//! The project: root, boundaries and the registry of what was read.
//!
//! The registry is needed already, although edits arrive in slice 3: §7.2 requires
//! registering a file attached via `@` as read, otherwise it would be pointless:
//! the human attached the file, and the agent must re-read it before an
//! edit.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::error::{CoreError, ToolError};

pub(crate) mod roots;

/// The fingerprint of a file at the time it was read. The snapshot slice 3 checks
/// freshness against: a file that changed after it was read is edited blind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReadRecord {
    pub mtime_ms: u64,
    pub len: u64,
}

#[derive(Debug)]
pub struct Workspace {
    /// Where the tools work: the project, or the chat's own worktree of it
    /// (`isolation`).
    root: PathBuf,
    /// Further folders the session may work in, besides `root` (`with_roots`).
    /// Canonical, directories, and none of them inside `root` or inside
    /// another of them. Fixed for the life of the session.
    extra: Vec<PathBuf>,
    /// Where the project keeps `.yaca`. The same as `root` unless the chat is
    /// isolated: settings, journals, memory and the shadow store belong to the
    /// project, not to one chat's checkout.
    home: PathBuf,
    read: Mutex<HashMap<PathBuf, ReadRecord>>,
    /// Directory names the project added to the snapshot's skip list (§10).
    /// Set once at session start from `.yaca/settings.json`: the permission
    /// questions need the same list the snapshot walks with, or a directory
    /// the project excludes would look saved when it is not.
    shadow_exclude: Vec<String>,
}

impl Workspace {
    pub fn open(root: impl AsRef<Path>) -> Result<Self, CoreError> {
        let root = root.as_ref();
        let canon = std::fs::canonicalize(root)
            .map_err(|e| CoreError::Config(format!("could not open project {}: {e}", root.display())))?;
        if !canon.is_dir() {
            return Err(CoreError::Config(format!(
                "{} is not a directory",
                canon.display()
            )));
        }
        Ok(Self {
            home: canon.clone(),
            root: canon,
            extra: Vec::new(),
            read: Mutex::new(HashMap::new()),
            shadow_exclude: Vec::new(),
        })
    }

    /// The same project, worked in from `tree`: a worktree of it.
    ///
    /// The boundary of the tools moves to `tree`, and the project's `.yaca`
    /// stays where it was — relative paths that start with it keep landing
    /// there, so memory, skills and commands are the project's
    /// in every chat.
    pub fn in_tree(mut self, tree: impl AsRef<Path>) -> Result<Self, CoreError> {
        let tree = tree.as_ref();
        let canon = std::fs::canonicalize(tree).map_err(|e| {
            CoreError::Config(format!("could not open working tree {}: {e}", tree.display()))
        })?;
        if !canon.is_dir() {
            return Err(CoreError::Config(format!("{} is not a directory", canon.display())));
        }
        self.root = canon;
        Ok(self)
    }

    /// Adds the session's further folders, replacing any set given before.
    ///
    /// Checked against the primary root as it is now, so call it after
    /// [`Workspace::in_tree`]: a folder inside the worktree is redundant, a
    /// folder inside the project but outside the worktree is not. See
    /// `roots::validate` for what is dropped and what is refused.
    pub fn with_roots(mut self, extra: &[PathBuf]) -> Result<Self, CoreError> {
        self.extra = roots::validate(&self.root, extra)?;
        Ok(self)
    }

    /// Takes the project's snapshot exclusions (`shadow.exclude`). Trimmed and
    /// emptied the same way the snapshot scanner does, so the two agree on
    /// what a name means.
    pub fn with_shadow_exclude(mut self, extra: &[String]) -> Self {
        self.shadow_exclude = extra
            .iter()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        self
    }

    /// The project's own snapshot exclusions, beside the built-in list.
    pub fn shadow_exclude(&self) -> &[String] {
        &self.shadow_exclude
    }

    /// The same project with nothing read yet.
    ///
    /// For a subagent: what it reads goes into its own conversation, and the
    /// agent that sent it gets a report, not the file. Sharing the registry
    /// would let that agent edit a file it has never seen, which is the one
    /// thing read-before-edit (§8) exists to prevent.
    pub fn fork(&self) -> Self {
        Self {
            root: self.root.clone(),
            extra: self.extra.clone(),
            home: self.home.clone(),
            read: Mutex::new(HashMap::new()),
            shadow_exclude: self.shadow_exclude.clone(),
        }
    }

    /// The primary root: where relative paths start, where `.yaca` is looked
    /// for beside, and what the checkpoints snapshot.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The additional roots, canonical, in the order they were given.
    pub fn additional(&self) -> &[PathBuf] {
        &self.extra
    }

    /// The additional roots as shown to a person or a model: absolute, with
    /// forward slashes and no verbatim prefix. Recorded in the journal and
    /// compared on resume.
    pub fn additional_text(&self) -> Vec<String> {
        self.extra.iter().map(|root| roots::plain(root)).collect()
    }

    /// Every root, the primary one first.
    pub fn roots(&self) -> Vec<&Path> {
        std::iter::once(self.root.as_path())
            .chain(self.extra.iter().map(PathBuf::as_path))
            .collect()
    }

    /// Whether `path` is one of the roots themselves, primary or additional.
    pub fn is_a_root(&self, path: &Path) -> bool {
        self.roots().contains(&path)
    }

    /// Whether `path` lies in an additional root (not the primary one).
    pub fn in_additional(&self, path: &Path) -> bool {
        !path.starts_with(&self.root) && self.extra.iter().any(|root| path.starts_with(root))
    }

    /// The root that holds `path`. Used wherever a check depends on which root
    /// a path is in: git's configuration (`invariant`), the project's `.yaca`
    /// and instructions, the question about what a rollback covers.
    ///
    /// A path in no root (the project's `.yaca` seen from an isolated chat, for
    /// one) gets the primary root, which is what the checks did before there
    /// were additional roots.
    pub fn root_for(&self, path: &Path) -> &Path {
        if path.starts_with(&self.root) {
            return &self.root;
        }
        self.extra
            .iter()
            .find(|root| path.starts_with(root))
            .map(PathBuf::as_path)
            .unwrap_or(&self.root)
    }

    /// The project's folder, where `.yaca` is. Not [`Workspace::root`] in an
    /// isolated chat: that is the worktree, which has no `.yaca` of its own.
    pub fn home(&self) -> &Path {
        &self.home
    }

    /// The project's `.yaca`, and so the one place a path may leave the root
    /// to.
    fn own_dir(&self) -> PathBuf {
        self.home.join(crate::project::DIR)
    }

    fn is_isolated(&self) -> bool {
        self.root != self.home
    }

    /// The path relative to the root — what the human sees in the feed.
    ///
    /// A file in an additional root is shown whole, by its absolute path:
    /// a relative form could pass for a file of the main root.
    pub fn display(&self, path: &Path) -> String {
        if let Ok(rel) = path.strip_prefix(&self.root) {
            return rel.to_string_lossy().replace('\\', "/");
        }
        if self.in_additional(path) {
            return roots::plain(path);
        }
        path.strip_prefix(&self.home)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/")
    }

    /// `path` in the form the roots are stored in: the same file, spelled so
    /// that it compares equal to a path the tools resolved. For paths that
    /// arrive from outside — a language server names files its own way.
    pub fn in_form(&self, path: &Path) -> PathBuf {
        roots::in_root_form(path, &self.root)
    }

    /// The path of `path` inside the root that holds it, forward slashes.
    ///
    /// The form a glob pattern is matched against: `src/**/*.rs` means the
    /// `src` of every root. [`Workspace::display`] is the form shown to a
    /// person and is absolute for an additional root, so it cannot serve here.
    pub fn within_root(&self, path: &Path) -> String {
        let root = self.root_for(path);
        path.strip_prefix(root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/")
    }

    /// Resolves a path from a tool's arguments and checks the workspace boundaries.
    ///
    /// The order of the checks matters: the boundary first, then existence.
    /// The other way round, and `../../secrets.env` would return «not found» instead of a refusal,
    /// so the invariant would turn into a report of what lies outside.
    pub fn resolve_existing(&self, raw: &str) -> Result<PathBuf, ToolError> {
        let candidate = self.bounded(raw)?;
        let canon = std::fs::canonicalize(&candidate)
            .map_err(|_| ToolError::failed(format!("not found: {raw}")))?;
        // The second time — after the links are dereferenced: a symbolic link
        // pointing outside passes the lexical check and must not pass this one.
        self.ensure_inside(&canon, raw)?;
        Ok(canon)
    }

    /// The same for a path that may not exist yet. Needed by the write tools
    /// (slice 3); declared here so that the boundary rule lives in one place.
    pub fn resolve_new(&self, raw: &str) -> Result<PathBuf, ToolError> {
        self.bounded(raw)
    }

    /// Brings the path to a normal form and checks that it is inside the project.
    ///
    /// Two checks, and both are needed. The lexical one catches `..` in a path that
    /// does not exist yet. Canonicalising the nearest existing ancestor catches a symbolic
    /// link pointing outside — lexically such a path looks flawless.
    fn bounded(&self, raw: &str) -> Result<PathBuf, ToolError> {
        let normalized = normalize(&self.join(raw));
        self.ensure_inside(&normalized, raw)?;

        let mut anchor = normalized.as_path();
        loop {
            if let Ok(real) = std::fs::canonicalize(anchor) {
                self.ensure_inside(&real, raw)?;
                break;
            }
            match anchor.parent() {
                Some(parent) => anchor = parent,
                None => break,
            }
        }
        // Returned in the form the roots are stored in, so an absolute path
        // typed by the model is the same key as the one `canonicalize` gives.
        Ok(roots::in_root_form(&normalized, &self.root))
    }

    fn join(&self, raw: &str) -> PathBuf {
        let p = Path::new(raw);
        if p.is_absolute() {
            p.to_path_buf()
        } else if self.is_isolated() && p.starts_with(crate::project::DIR) {
            // The worktree has no `.yaca`, and one made there would be this
            // chat's alone: its memory would never reach the project.
            self.home.join(p)
        } else {
            self.root.join(p)
        }
    }

    /// Whether `path` is inside the primary root or any additional one.
    fn contains(&self, path: &Path) -> bool {
        path.starts_with(&self.root) || self.extra.iter().any(|root| path.starts_with(root))
    }

    /// The boundary rule. It is the same for every root: a symlink out of any
    /// of them fails the canonical check, a `..` out of any of them fails the
    /// lexical one. The `.yaca` exception is the primary project's only.
    fn ensure_inside(&self, canon: &Path, raw: &str) -> Result<(), ToolError> {
        let form = roots::in_root_form(canon, &self.root);
        if self.contains(&form) || (self.is_isolated() && form.starts_with(self.own_dir())) {
            Ok(())
        } else {
            Err(ToolError::rejected(format!(
                "path outside the project: {raw} → {}",
                canon.display()
            )))
        }
    }

    /// The file's current fingerprint on disk. `None` — the file does not exist.
    pub fn fingerprint(&self, path: &Path) -> Option<ReadRecord> {
        let meta = std::fs::metadata(path).ok()?;
        let mtime_ms = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        Some(ReadRecord {
            mtime_ms,
            len: meta.len(),
        })
    }

    /// Marks the file as read. Called by the read tool and when attached with `@`.
    pub fn mark_read(&self, path: &Path) {
        if let Some(record) = self.fingerprint(path) {
            self.read.lock().unwrap().insert(path.to_path_buf(), record);
        }
    }

    /// The file was created by the agent in the current set: it is not on disk, but
    /// the agent knows its contents — there is no need to read it again.
    pub fn mark_authored(&self, path: &Path) {
        self.read
            .lock()
            .unwrap()
            .entry(path.to_path_buf())
            .or_insert(ReadRecord {
                mtime_ms: 0,
                len: 0,
            });
    }

    /// Forgets the fingerprint: the file changed outside and must be read again (§11).
    pub fn forget(&self, path: &Path) {
        self.read.lock().unwrap().remove(path);
    }

    /// Forget everything read under a directory that is gone.
    pub fn forget_under(&self, dir: &Path) {
        self.read.lock().unwrap().retain(|path, _| !path.starts_with(dir));
    }

    pub fn read_record(&self, path: &Path) -> Option<ReadRecord> {
        self.read.lock().unwrap().get(path).copied()
    }

    /// Whether the file was read in this session — the read-before-edit invariant (§8).
    pub fn was_read(&self, path: &Path) -> bool {
        self.read.lock().unwrap().contains_key(path)
    }
}

/// Collapses `.` and `..` without touching the disk. That is why the boundary check
/// works for paths that do not exist yet.
fn normalize(path: &Path) -> PathBuf {
    use std::path::Component;

    let mut components = path.components().peekable();
    let mut out = match components.peek() {
        Some(Component::Prefix(..)) => {
            let first = components.next().expect("the prefix has just been peeked at");
            PathBuf::from(first.as_os_str())
        }
        _ => PathBuf::new(),
    };

    for component in components {
        match component {
            Component::Prefix(..) => unreachable!("a prefix only ever comes first"),
            Component::RootDir => out.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            Component::Normal(part) => out.push(part),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;

    fn temp_project() -> (TempDir, Workspace) {
        let dir = TempDir::new();
        dir.write("a.txt", "hello");
        dir.mkdir("sub");
        let ws = Workspace::open(dir.path()).unwrap();
        (dir, ws)
    }

    #[test]
    fn relative_path_inside_project_resolves() {
        let (_d, ws) = temp_project();
        let p = ws.resolve_existing("a.txt").unwrap();
        assert!(p.ends_with("a.txt"));
        assert_eq!(ws.display(&p), "a.txt");
    }

    #[test]
    fn leaving_the_root_is_refused_as_an_invariant() {
        let (_d, ws) = temp_project();
        let err = ws.resolve_existing("../..").unwrap_err();
        assert!(matches!(err, ToolError::Rejected(_)), "got {err:?}");
    }

    #[test]
    fn nonexistent_path_outside_is_refused_too_not_lost() {
        let (_d, ws) = temp_project();
        // Otherwise the refusal is replaced by a report of what is not outside.
        let err = ws.resolve_existing("../../secrets.env").unwrap_err();
        assert!(matches!(err, ToolError::Rejected(_)), "got {err:?}");
    }

    #[test]
    fn dots_inside_the_project_collapse_and_do_no_harm() {
        let (_d, ws) = temp_project();
        let p = ws.resolve_existing("sub/../a.txt").unwrap();
        assert_eq!(ws.display(&p), "a.txt");
    }

    #[test]
    fn new_file_is_checked_against_its_existing_ancestor() {
        let (_d, ws) = temp_project();
        assert!(ws.resolve_new("sub/new.rs").is_ok());
        assert!(matches!(
            ws.resolve_new("../outside.rs"),
            Err(ToolError::Rejected(_))
        ));
    }

    #[test]
    fn read_registry_remembers_the_file() {
        let (_d, ws) = temp_project();
        let p = ws.resolve_existing("a.txt").unwrap();
        assert!(!ws.was_read(&p));
        ws.mark_read(&p);
        assert!(ws.was_read(&p));
        assert!(ws.read_record(&p).unwrap().len > 0);
    }

    /// The main root with `a.txt` and an additional one with `b.txt`, and a third directory
    /// outside both.
    fn two_roots() -> (TempDir, TempDir, TempDir, Workspace) {
        let main = TempDir::new();
        main.write("a.txt", "main\n");
        let extra = TempDir::new();
        extra.write("b.txt", "extra\n");
        let outside = TempDir::new();
        outside.write("secret.env", "x\n");
        let ws = Workspace::open(main.path())
            .unwrap()
            .with_roots(&[extra.path().to_path_buf()])
            .unwrap();
        (main, extra, outside, ws)
    }

    /// The path as the model sees it: without `\\?\`, with forward slashes.
    fn as_model_sees(path: &Path) -> String {
        roots::plain(path)
    }

    #[test]
    fn extra_root_file_opens_by_absolute_path() {
        let (_m, extra, _o, ws) = two_roots();
        let by_abs = ws.resolve_existing(&extra.path().join("b.txt").to_string_lossy()).unwrap();
        assert!(by_abs.starts_with(extra.path()));
        // The same file — the same key: the registry of what was read does not diverge.
        let by_model = ws.resolve_existing(&as_model_sees(&extra.path().join("b.txt"))).unwrap();
        assert_eq!(by_abs, by_model);
        assert!(ws.resolve_new(&as_model_sees(&extra.path().join("new.txt"))).is_ok());
    }

    #[test]
    fn relative_path_always_goes_to_the_main_root() {
        let (_m, _e, _o, ws) = two_roots();
        // `b.txt` exists only in the additional root: a relative path does not find it,
        // rather than standing in for it.
        let err = ws.resolve_existing("b.txt").unwrap_err();
        assert!(err.to_string().contains("not found"), "{err}");
        assert!(ws.resolve_existing("a.txt").unwrap().starts_with(ws.root()));
    }

    #[test]
    fn path_outside_all_roots_is_refused_whether_it_exists_or_not() {
        let (_m, _e, outside, ws) = two_roots();
        let secret = outside.path().join("secret.env");
        let err = ws.resolve_existing(&as_model_sees(&secret)).unwrap_err();
        assert!(matches!(err, ToolError::Rejected(_)), "got {err:?}");

        let missing = outside.path().join("missing.env");
        let err = ws.resolve_new(&as_model_sees(&missing)).unwrap_err();
        assert!(matches!(err, ToolError::Rejected(_)), "got {err:?}");
    }

    #[test]
    fn symlink_from_extra_root_pointing_outside_is_refused() {
        let (_m, extra, outside, ws) = two_roots();
        let link = extra.path().join("link.env");
        if make_file_link(&outside.path().join("secret.env"), &link).is_err() {
            // The host does not allow creating a link: the check is skipped, not passed.
            eprintln!("skipped: the host cannot create symbolic links");
            return;
        }
        let err = ws.resolve_existing(&as_model_sees(&link)).unwrap_err();
        assert!(matches!(err, ToolError::Rejected(_)), "got {err:?}");
    }

    #[test]
    fn main_root_display_is_relative_and_extra_root_display_is_absolute() {
        let (_m, extra, _o, ws) = two_roots();
        let main_file = ws.resolve_existing("a.txt").unwrap();
        assert_eq!(ws.display(&main_file), "a.txt");

        let extra_file = ws.resolve_existing(&as_model_sees(&extra.path().join("b.txt"))).unwrap();
        let shown = ws.display(&extra_file);
        assert_eq!(shown, as_model_sees(&extra_file));
        assert!(!shown.contains(r"\\?\") && !shown.contains('\\'), "{shown}");
        assert!(shown.ends_with("/b.txt"), "{shown}");
        // A relative form would be indistinguishable from a file of the main root.
        assert_ne!(shown, "b.txt");
    }

    #[test]
    fn file_root_and_path_for_pattern_matching() {
        let (_m, extra, _o, ws) = two_roots();
        let extra_file = ws.resolve_existing(&as_model_sees(&extra.path().join("b.txt"))).unwrap();
        assert_eq!(ws.root_for(&extra_file), ws.additional()[0].as_path());
        assert_eq!(ws.within_root(&extra_file), "b.txt");

        let main_file = ws.resolve_existing("a.txt").unwrap();
        assert_eq!(ws.root_for(&main_file), ws.root());
        assert_eq!(ws.within_root(&main_file), "a.txt");

        assert!(ws.is_a_root(ws.additional()[0].as_path()));
        assert!(ws.is_a_root(ws.root()));
        assert!(!ws.is_a_root(&extra_file));
    }

    #[test]
    fn edit_and_read_in_an_extra_root_share_one_key() {
        let (_m, extra, _o, ws) = two_roots();
        let path = ws.resolve_existing(&as_model_sees(&extra.path().join("b.txt"))).unwrap();
        ws.mark_read(&path);
        // By the same path the edit gives it, it is recognised as read.
        let edit_path = ws.resolve_new(&as_model_sees(&extra.path().join("b.txt"))).unwrap();
        assert!(ws.was_read(&edit_path));
    }

    #[test]
    fn duplicate_nested_and_main_roots_are_collapsed() {
        let main = TempDir::new();
        let extra = TempDir::new();
        let inside = main.mkdir("sub");
        let ws = Workspace::open(main.path())
            .unwrap()
            .with_roots(&[
                main.path().to_path_buf(),
                inside,
                extra.path().to_path_buf(),
                extra.path().to_path_buf(),
            ])
            .unwrap();
        // The root itself and what is inside it are already covered; one duplicate.
        assert_eq!(ws.additional(), [extra.path().to_path_buf()]);
        assert_eq!(ws.roots().len(), 2);
    }

    #[test]
    fn nested_extra_root_is_absorbed_by_the_enclosing_one() {
        let main = TempDir::new();
        let outer = TempDir::new();
        let inner = outer.mkdir("inner");
        let ws = Workspace::open(main.path())
            .unwrap()
            .with_roots(&[inner, outer.path().to_path_buf()])
            .unwrap();
        assert_eq!(ws.additional(), [outer.path().to_path_buf()]);
    }

    #[test]
    fn root_containing_the_project_is_refused_not_silently() {
        let parent = TempDir::new();
        let project = parent.mkdir("project");
        let ws = Workspace::open(&project).unwrap();
        let err = ws
            .with_roots(&[parent.path().to_path_buf()])
            .unwrap_err();
        assert!(matches!(err, CoreError::Config(_)), "{err:?}");
    }

    #[test]
    fn missing_or_file_extra_root_is_refused() {
        let main = TempDir::new();
        let other = TempDir::new();
        let file = other.write("f.txt", "x\n");
        let missing = main.path().join("missing");
        let ws = Workspace::open(main.path()).unwrap();
        assert!(matches!(
            ws.with_roots(&[missing]),
            Err(CoreError::Config(_))
        ));
        let ws = Workspace::open(main.path()).unwrap();
        assert!(matches!(ws.with_roots(&[file]), Err(CoreError::Config(_))));
    }

    #[test]
    fn fork_keeps_extra_roots() {
        let (_m, extra, _o, ws) = two_roots();
        let forked = ws.fork();
        assert_eq!(forked.additional(), ws.additional());
        assert!(forked.resolve_existing(&as_model_sees(&extra.path().join("b.txt"))).is_ok());
    }

    /// Make a file link, or report that the host cannot (then the caller skips).
    fn make_file_link(target: &Path, link: &Path) -> std::io::Result<()> {
        #[cfg(unix)]
        return std::os::unix::fs::symlink(target, link);
        #[cfg(windows)]
        return std::os::windows::fs::symlink_file(target, link);
    }
}
