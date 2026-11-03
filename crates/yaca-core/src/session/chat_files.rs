//! Copies user-supplied files into a chat without modifying the originals.

use std::path::{Path, PathBuf};

pub(super) fn copy(source: &Path, root: &Path) -> std::io::Result<PathBuf> {
    let source = std::fs::canonicalize(source)?;
    let root = std::fs::canonicalize(root)?;
    if root.starts_with(&source) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "attachment contains the chat directory",
        ));
    }
    let dir = root.join("attachments");
    std::fs::create_dir_all(&dir)?;
    let dir = std::fs::canonicalize(dir)?;
    if !dir.starts_with(&root) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "attachments directory is outside the chat",
        ));
    }
    let name = source
        .file_name()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "file has no name"))?
        .to_string_lossy();
    for index in 0..10_000 {
        let name = if index == 0 {
            name.to_string()
        } else {
            format!("{index}-{name}")
        };
        let target = dir.join(name);
        if source.is_dir() {
            match std::fs::create_dir(&target) {
                Ok(()) => {
                    crate::store::copy_workspace(&source, &target)?;
                    return Ok(target);
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)
        {
            Ok(mut output) => {
                if let Err(error) = std::fs::File::open(&source)
                    .and_then(|mut input| std::io::copy(&mut input, &mut output))
                {
                    drop(output);
                    let _ = std::fs::remove_file(&target);
                    return Err(error);
                }
                return Ok(target);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "too many files with this name",
    ))
}
