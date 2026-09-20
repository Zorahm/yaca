//! What the working tree looked like: one line per file.
//!
//! Text, sorted by path, so equal trees encode to equal bytes and therefore to
//! one name in the store. That identity is what makes a snapshot before a
//! read-only command free: the manifest is already there, no checkpoint is
//! recorded, and the feed does not collect a divider that rolls back to the
//! state it is already in.
//!
//! The line carries the mode as well. It is one bit in practice — a restored
//! script that lost `+x` is a script that no longer runs — and everything else
//! a filesystem knows about a file is not worth the risk of restoring it.

use std::io;

use blake3::Hash;

/// The executable bit, the only mode worth carrying across a snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    File,
    Exec,
}

impl Mode {
    fn as_str(self) -> &'static str {
        match self {
            Mode::File => "644",
            Mode::Exec => "755",
        }
    }

    fn parse(raw: &str) -> Option<Mode> {
        match raw {
            "644" => Some(Mode::File),
            "755" => Some(Mode::Exec),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// Relative to the project root, `/` separated on every platform.
    pub path: String,
    pub hash: Hash,
    pub mode: Mode,
    pub len: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Manifest {
    /// Sorted by path. The order is part of the format: it is what makes the
    /// encoding a function of the tree and not of the walk.
    pub entries: Vec<Entry>,
}

impl Manifest {
    /// Sorts on the way in, so no caller has to remember to.
    pub fn new(mut entries: Vec<Entry>) -> Self {
        entries.sort_by(|a, b| a.path.cmp(&b.path));
        Manifest { entries }
    }

    pub fn get(&self, path: &str) -> Option<&Entry> {
        self.entries
            .binary_search_by(|e| e.path.as_str().cmp(path))
            .ok()
            .map(|i| &self.entries[i])
    }

    pub fn encode(&self) -> String {
        let mut out = String::new();
        for e in &self.entries {
            out.push_str(&format!(
                "{} {} {} {}\n",
                e.hash.to_hex(),
                e.mode.as_str(),
                e.len,
                e.path
            ));
        }
        out
    }

    /// A manifest that does not parse is not half a manifest: restoring from
    /// it would delete every file it failed to read.
    pub fn decode(text: &str) -> io::Result<Manifest> {
        let mut entries = Vec::new();
        for line in text.lines() {
            if line.is_empty() {
                continue;
            }
            let mut parts = line.splitn(4, ' ');
            let entry = (|| {
                let hash = Hash::from_hex(parts.next()?).ok()?;
                let mode = Mode::parse(parts.next()?)?;
                let len = parts.next()?.parse().ok()?;
                let path = parts.next()?;
                if path.is_empty() {
                    return None;
                }
                Some(Entry {
                    path: path.to_string(),
                    hash,
                    mode,
                    len,
                })
            })();
            match entry {
                Some(e) => entries.push(e),
                None => return Err(io::Error::other(format!("broken manifest line: {line}"))),
            }
        }
        Ok(Manifest::new(entries))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checkpoint::store::Store;

    fn entry(path: &str, body: &str, mode: Mode) -> Entry {
        Entry {
            path: path.to_string(),
            hash: Store::hash(body.as_bytes()),
            mode,
            len: body.len() as u64,
        }
    }

    #[test]
    fn round_trip_keeps_every_field() {
        let m = Manifest::new(vec![
            entry("src/a.rs", "one\n", Mode::File),
            entry("run.sh", "#!/bin/sh\n", Mode::Exec),
        ]);
        assert_eq!(Manifest::decode(&m.encode()).unwrap(), m);
    }

    #[test]
    fn equal_trees_encode_to_equal_bytes_whatever_the_walk_order() {
        let one = Manifest::new(vec![
            entry("b.rs", "x\n", Mode::File),
            entry("a.rs", "y\n", Mode::File),
        ]);
        let other = Manifest::new(vec![
            entry("a.rs", "y\n", Mode::File),
            entry("b.rs", "x\n", Mode::File),
        ]);
        // The store names a snapshot by this; if it depended on the walk,
        // every scan would look like a change.
        assert_eq!(one.encode(), other.encode());
    }

    #[test]
    fn a_path_with_spaces_survives() {
        let m = Manifest::new(vec![entry("src/two words.rs", "x\n", Mode::File)]);
        let back = Manifest::decode(&m.encode()).unwrap();
        assert_eq!(back.entries[0].path, "src/two words.rs");
    }

    #[test]
    fn a_broken_line_fails_instead_of_dropping_a_file() {
        // Half a manifest restores as "delete everything it could not read".
        assert!(Manifest::decode("not a manifest\n").is_err());
    }

    #[test]
    fn lookup_finds_the_entry_by_path() {
        let m = Manifest::new(vec![
            entry("a.rs", "x\n", Mode::File),
            entry("b.rs", "y\n", Mode::File),
        ]);
        assert_eq!(m.get("b.rs").unwrap().len, 2);
        assert!(m.get("c.rs").is_none());
    }
}
