//! Проект: корень, границы и реестр прочитанного.
//!
//! Реестр нужен уже сейчас, хотя правки приходят срезом 3: §7.2 требует
//! регистрировать приложенный через `@` файл как прочитанный, иначе получается
//! глупость — человек сам приложил файл, а агент обязан перечитать его перед
//! правкой.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::error::{CoreError, ToolError};

/// Отпечаток файла на момент чтения. Слепок, по которому срез 3 проверит
/// свежесть: файл, изменившийся после чтения, правится вслепую.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReadRecord {
    pub mtime_ms: u64,
    pub len: u64,
}

#[derive(Debug)]
pub struct Workspace {
    root: PathBuf,
    read: Mutex<HashMap<PathBuf, ReadRecord>>,
}

impl Workspace {
    pub fn open(root: impl AsRef<Path>) -> Result<Self, CoreError> {
        let root = root.as_ref();
        let canon = std::fs::canonicalize(root)
            .map_err(|e| CoreError::Config(format!("не открыть проект {}: {e}", root.display())))?;
        if !canon.is_dir() {
            return Err(CoreError::Config(format!(
                "{} — не каталог",
                canon.display()
            )));
        }
        Ok(Self {
            root: canon,
            read: Mutex::new(HashMap::new()),
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Путь относительно корня — то, что видит человек в ленте.
    pub fn display(&self, path: &Path) -> String {
        path.strip_prefix(&self.root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/")
    }

    /// Разрешает путь из аргументов тула и проверяет границы воркспейса.
    ///
    /// Порядок проверок принципиален: сначала граница, потом существование.
    /// Наоборот — и `../../secrets.env` вернул бы «не найдено» вместо отказа,
    /// то есть инвариант превратился бы в отчёт о том, что лежит снаружи.
    pub fn resolve_existing(&self, raw: &str) -> Result<PathBuf, ToolError> {
        let candidate = self.bounded(raw)?;
        let canon = std::fs::canonicalize(&candidate)
            .map_err(|_| ToolError::failed(format!("не найдено: {raw}")))?;
        // Второй раз — уже после разыменования ссылок: символическая ссылка
        // наружу проходит лексическую проверку и не должна проходить эту.
        self.ensure_inside(&canon, raw)?;
        Ok(canon)
    }

    /// То же для пути, которого может ещё не быть. Нужен тулам записи
    /// (срез 3); объявлен здесь, чтобы правило границ жило в одном месте.
    pub fn resolve_new(&self, raw: &str) -> Result<PathBuf, ToolError> {
        self.bounded(raw)
    }

    /// Приводит путь к нормальной форме и проверяет, что он внутри проекта.
    ///
    /// Две проверки, и обе нужны. Лексическая ловит `..` у пути, которого ещё
    /// нет. Канонизация ближайшего существующего предка ловит символическую
    /// ссылку наружу — лексически такой путь выглядит безупречно.
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
        Ok(normalized)
    }

    fn join(&self, raw: &str) -> PathBuf {
        let p = Path::new(raw);
        if p.is_absolute() {
            p.to_path_buf()
        } else {
            self.root.join(p)
        }
    }

    fn ensure_inside(&self, canon: &Path, raw: &str) -> Result<(), ToolError> {
        if canon.starts_with(&self.root) {
            Ok(())
        } else {
            Err(ToolError::rejected(format!(
                "путь вне проекта: {raw} → {}",
                canon.display()
            )))
        }
    }

    /// Текущий отпечаток файла на диске. `None` — файла нет.
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

    /// Отметить файл прочитанным. Вызывается тулом чтения и при вложении `@`.
    pub fn mark_read(&self, path: &Path) {
        if let Some(record) = self.fingerprint(path) {
            self.read.lock().unwrap().insert(path.to_path_buf(), record);
        }
    }

    /// Файл создан агентом в текущем наборе: на диске его нет, но содержимое
    /// агент знает — читать заново незачем.
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

    /// Забыть отпечаток: файл изменился извне и должен быть перечитан (§11).
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

    /// Читался ли файл в этой сессии — инвариант read-before-edit (§8).
    pub fn was_read(&self, path: &Path) -> bool {
        self.read.lock().unwrap().contains_key(path)
    }
}

/// Свернуть `.` и `..` не трогая диск. Именно поэтому проверка границ
/// работает и для путей, которых ещё нет.
fn normalize(path: &Path) -> PathBuf {
    use std::path::Component;

    let mut components = path.components().peekable();
    let mut out = match components.peek() {
        Some(Component::Prefix(..)) => {
            let first = components.next().expect("префикс только что подсмотрен");
            PathBuf::from(first.as_os_str())
        }
        _ => PathBuf::new(),
    };

    for component in components {
        match component {
            Component::Prefix(..) => unreachable!("префикс бывает только первым"),
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
        dir.write("a.txt", "привет");
        dir.mkdir("sub");
        let ws = Workspace::open(dir.path()).unwrap();
        (dir, ws)
    }

    #[test]
    fn относительный_путь_внутри_проекта_разрешается() {
        let (_d, ws) = temp_project();
        let p = ws.resolve_existing("a.txt").unwrap();
        assert!(p.ends_with("a.txt"));
        assert_eq!(ws.display(&p), "a.txt");
    }

    #[test]
    fn выход_за_корень_отклоняется_как_инвариант() {
        let (_d, ws) = temp_project();
        let err = ws.resolve_existing("../..").unwrap_err();
        assert!(matches!(err, ToolError::Rejected(_)), "получили {err:?}");
    }

    #[test]
    fn несуществующий_путь_снаружи_тоже_отклоняется_а_не_теряется() {
        let (_d, ws) = temp_project();
        // Иначе отказ подменяется отчётом о том, чего снаружи нет.
        let err = ws.resolve_existing("../../secrets.env").unwrap_err();
        assert!(matches!(err, ToolError::Rejected(_)), "получили {err:?}");
    }

    #[test]
    fn точки_внутри_проекта_сворачиваются_и_не_мешают() {
        let (_d, ws) = temp_project();
        let p = ws.resolve_existing("sub/../a.txt").unwrap();
        assert_eq!(ws.display(&p), "a.txt");
    }

    #[test]
    fn новый_файл_проверяется_по_существующему_предку() {
        let (_d, ws) = temp_project();
        assert!(ws.resolve_new("sub/новый.rs").is_ok());
        assert!(matches!(
            ws.resolve_new("../снаружи.rs"),
            Err(ToolError::Rejected(_))
        ));
    }

    #[test]
    fn реестр_прочитанного_помнит_файл() {
        let (_d, ws) = temp_project();
        let p = ws.resolve_existing("a.txt").unwrap();
        assert!(!ws.was_read(&p));
        ws.mark_read(&p);
        assert!(ws.was_read(&p));
        assert!(ws.read_record(&p).unwrap().len > 0);
    }
}
