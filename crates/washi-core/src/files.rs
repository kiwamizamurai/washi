use std::{
    collections::hash_map::DefaultHasher,
    fs,
    hash::{Hash, Hasher},
    io::Write,
    path::{Path, PathBuf},
};

use serde::Serialize;

const MAX_EDITABLE_BYTES: u64 = 20 * 1024 * 1024;

pub fn hash_text(text: &str) -> String {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DiskText {
    pub text: String,
    pub hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum SaveResult {
    Saved { hash: String },
    Conflict { disk_hash: String },
}

pub fn read_text(path: &Path) -> Result<DiskText, String> {
    let meta = fs::metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if meta.len() > MAX_EDITABLE_BYTES {
        return Err(format!("{}: the file is too large to edit (limit {} MB)", path.display(), MAX_EDITABLE_BYTES >> 20));
    }
    let bytes = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let text = String::from_utf8(bytes).map_err(|_| format!("{}: not UTF-8 text", path.display()))?;
    let hash = hash_text(&text);
    Ok(DiskText { text, hash })
}

pub fn write_file(path: &Path, text: &str, base_hash: Option<&str>, force: bool) -> Result<SaveResult, String> {
    let target = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());

    if !force {
        if let (Some(base), Ok(disk)) = (base_hash, fs::read(&target)) {
            let disk_hash = hash_text(&String::from_utf8_lossy(&disk));
            if disk_hash != base {
                return Ok(SaveResult::Conflict { disk_hash });
            }
        }
    }

    let dir = target.parent().unwrap_or(Path::new("."));
    let temp = temp_path(dir, &target);
    let written = (|| -> std::io::Result<()> {
        let mut file = fs::File::create(&temp)?;
        file.write_all(text.as_bytes())?;
        file.sync_all()?;
        drop(file);
        if let Ok(meta) = fs::metadata(&target) {
            let _ = fs::set_permissions(&temp, meta.permissions());
        }
        fs::rename(&temp, &target)
    })();
    if let Err(e) = written {
        let _ = fs::remove_file(&temp);
        return Err(format!("{}: {e}", target.display()));
    }
    Ok(SaveResult::Saved { hash: hash_text(text) })
}

fn temp_path(dir: &Path, target: &Path) -> PathBuf {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let name = target.file_name().and_then(|n| n.to_str()).unwrap_or("file");
    dir.join(format!(".washi-save-{}-{}-{name}", std::process::id(), COUNTER.fetch_add(1, Ordering::Relaxed)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("washi-files-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn leftovers(dir: &Path) -> Vec<String> {
        fs::read_dir(dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.starts_with(".washi-save-"))
            .collect()
    }

    #[test]
    fn the_hash_depends_only_on_the_content() {
        assert_eq!(hash_text("abc"), hash_text("abc"));
        assert_ne!(hash_text("abc"), hash_text("abd"));
        assert_eq!(hash_text("").len(), 16);
    }

    #[test]
    fn read_returns_the_text_and_its_hash() {
        let d = dir("read");
        let f = d.join("a.md");
        fs::write(&f, "日本語\n").unwrap();
        let disk = read_text(&f).unwrap();
        assert_eq!(disk.text, "日本語\n");
        assert_eq!(disk.hash, hash_text("日本語\n"));
    }

    #[test]
    fn read_rejects_binary_and_missing_files() {
        let d = dir("read-bad");
        fs::write(d.join("bin"), [0xff, 0xfe, 0x00]).unwrap();
        assert!(read_text(&d.join("bin")).unwrap_err().contains("UTF-8"));
        assert!(read_text(&d.join("nope")).is_err());
    }

    #[test]
    fn write_creates_and_replaces_without_leaving_temp_files() {
        let d = dir("write");
        let f = d.join("a.md");
        assert_eq!(write_file(&f, "one", None, false).unwrap(), SaveResult::Saved { hash: hash_text("one") });
        assert_eq!(fs::read_to_string(&f).unwrap(), "one");
        let base = hash_text("one");
        assert_eq!(write_file(&f, "two", Some(&base), false).unwrap(), SaveResult::Saved { hash: hash_text("two") });
        assert_eq!(fs::read_to_string(&f).unwrap(), "two");
        assert!(leftovers(&d).is_empty(), "{:?}", leftovers(&d));
    }

    #[test]
    fn a_changed_disk_is_a_conflict_and_nothing_is_written() {
        let d = dir("conflict");
        let f = d.join("a.md");
        fs::write(&f, "base").unwrap();
        let base = hash_text("base");
        fs::write(&f, "changed elsewhere").unwrap();
        let result = write_file(&f, "mine", Some(&base), false).unwrap();
        assert_eq!(result, SaveResult::Conflict { disk_hash: hash_text("changed elsewhere") });
        assert_eq!(fs::read_to_string(&f).unwrap(), "changed elsewhere");
    }

    #[test]
    fn force_overwrites_a_conflict_on_purpose() {
        let d = dir("force");
        let f = d.join("a.md");
        fs::write(&f, "changed elsewhere").unwrap();
        let result = write_file(&f, "mine", Some(&hash_text("base")), true).unwrap();
        assert_eq!(result, SaveResult::Saved { hash: hash_text("mine") });
        assert_eq!(fs::read_to_string(&f).unwrap(), "mine");
    }

    #[test]
    fn a_save_with_the_same_content_on_disk_is_not_a_conflict() {
        let d = dir("same");
        let f = d.join("a.md");
        fs::write(&f, "same").unwrap();
        assert!(matches!(write_file(&f, "new", Some(&hash_text("same")), false).unwrap(), SaveResult::Saved { .. }));
    }

    #[cfg(unix)]
    #[test]
    fn permissions_are_kept() {
        use std::os::unix::fs::PermissionsExt;
        let d = dir("perm");
        let f = d.join("run.sh");
        fs::write(&f, "old").unwrap();
        fs::set_permissions(&f, fs::Permissions::from_mode(0o750)).unwrap();
        write_file(&f, "new", None, false).unwrap();
        assert_eq!(fs::metadata(&f).unwrap().permissions().mode() & 0o777, 0o750);
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_stays_a_symlink_and_the_target_is_written() {
        let d = dir("link");
        let real = d.join("real.md");
        let link = d.join("link.md");
        fs::write(&real, "old").unwrap();
        std::os::unix::fs::symlink(&real, &link).unwrap();
        write_file(&link, "new", None, false).unwrap();
        assert!(fs::symlink_metadata(&link).unwrap().file_type().is_symlink());
        assert_eq!(fs::read_to_string(&real).unwrap(), "new");
    }

    #[cfg(unix)]
    #[test]
    fn a_failed_save_leaves_the_original_untouched() {
        use std::os::unix::fs::PermissionsExt;
        let d = dir("fail");
        let f = d.join("a.md");
        fs::write(&f, "original").unwrap();
        fs::set_permissions(&d, fs::Permissions::from_mode(0o555)).unwrap();
        let result = write_file(&f, "new", None, false);
        fs::set_permissions(&d, fs::Permissions::from_mode(0o755)).unwrap();
        if result.is_ok() {
            return;
        }
        assert_eq!(fs::read_to_string(&f).unwrap(), "original");
        assert!(leftovers(&d).is_empty());
    }
}
