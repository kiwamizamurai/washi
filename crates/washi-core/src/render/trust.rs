use std::{
    fs,
    path::{Path, PathBuf},
};

const PROJECT_RC_NAMES: [&str; 2] = ["latexmkrc", ".latexmkrc"];

pub fn project_rc(dir: &Path) -> Option<PathBuf> {
    PROJECT_RC_NAMES.iter().map(|name| dir.join(name)).find(|p| p.is_file())
}

pub fn user_rc() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let xdg = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| home.as_ref().map(|h| h.join(".config")))
        .map(|d| d.join("latexmk/latexmkrc"));
    let traditional = home.map(|h| h.join(".latexmkrc"));
    [xdg, traditional].into_iter().flatten().find(|p| p.is_file())
}

fn stable_hash(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325, |hash, byte| (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3))
}

pub struct Trust {
    store: Option<PathBuf>,
}

impl Trust {
    pub fn system() -> Self {
        let store = std::env::var_os("WASHI_TRUST_FILE").map(PathBuf::from).or_else(|| {
            let home = std::env::var_os("HOME").map(PathBuf::from)?;
            let dir = if cfg!(target_os = "macos") {
                home.join("Library/Application Support/washi")
            } else {
                std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).unwrap_or_else(|| home.join(".local/share")).join("washi")
            };
            Some(dir.join("trusted-latexmkrc"))
        });
        Self { store }
    }

    pub fn at(store: PathBuf) -> Self {
        Self { store: Some(store) }
    }

    fn key(rc: &Path) -> Option<String> {
        let canonical = fs::canonicalize(rc).ok()?;
        let content = fs::read(&canonical).ok()?;
        Some(format!("{:016x}\t{}", stable_hash(&content), canonical.display()))
    }

    pub fn is_trusted(&self, rc: &Path) -> bool {
        let (Some(store), Some(key)) = (&self.store, Self::key(rc)) else { return false };
        fs::read_to_string(store).is_ok_and(|text| text.lines().any(|line| line == key))
    }

    pub fn trust(&self, rc: &Path) -> Result<(), String> {
        let store = self.store.as_ref().ok_or("there is nowhere to remember this choice")?;
        let key = Self::key(rc).ok_or("cannot read the .latexmkrc")?;
        if self.is_trusted(rc) {
            return Ok(());
        }
        if let Some(dir) = store.parent() {
            fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let mut text = fs::read_to_string(store).unwrap_or_default();
        if !text.is_empty() && !text.ends_with('\n') {
            text.push('\n');
        }
        text.push_str(&key);
        text.push('\n');
        fs::write(store, text).map_err(|e| format!("cannot remember this choice: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("washi-trust-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn finds_the_project_rc_file_by_either_name() {
        let d = dir("names");
        assert_eq!(project_rc(&d), None);
        fs::write(d.join(".latexmkrc"), "").unwrap();
        assert_eq!(project_rc(&d), Some(d.join(".latexmkrc")));
        fs::write(d.join("latexmkrc"), "").unwrap();
        assert_eq!(project_rc(&d), Some(d.join("latexmkrc")));
    }

    #[test]
    fn a_file_is_trusted_only_after_it_is_trusted_and_only_while_it_is_unchanged() {
        let d = dir("store");
        let trust = Trust::at(d.join("store/trusted"));
        let rc = d.join(".latexmkrc");
        fs::write(&rc, "$pdf_mode = 3;").unwrap();
        assert!(!trust.is_trusted(&rc));
        trust.trust(&rc).unwrap();
        assert!(trust.is_trusted(&rc));
        trust.trust(&rc).unwrap();
        assert_eq!(fs::read_to_string(d.join("store/trusted")).unwrap().lines().count(), 1, "trusting twice records it once");

        fs::write(&rc, "system('echo changed');").unwrap();
        assert!(!trust.is_trusted(&rc), "a changed file must be confirmed again");
    }

    #[test]
    fn trust_is_per_file() {
        let d = dir("per-file");
        let trust = Trust::at(d.join("trusted"));
        let (a, b) = (d.join("a/.latexmkrc"), d.join("b/.latexmkrc"));
        for f in [&a, &b] {
            fs::create_dir_all(f.parent().unwrap()).unwrap();
            fs::write(f, "$pdf_mode = 3;").unwrap();
        }
        trust.trust(&a).unwrap();
        assert!(trust.is_trusted(&a));
        assert!(!trust.is_trusted(&b), "the same content in another folder is not trusted");
    }

    #[test]
    fn nothing_is_trusted_without_a_store_or_for_a_missing_file() {
        let d = dir("none");
        let trust = Trust { store: None };
        let rc = d.join(".latexmkrc");
        fs::write(&rc, "x").unwrap();
        assert!(!trust.is_trusted(&rc));
        assert!(trust.trust(&rc).is_err());
        assert!(!Trust::at(d.join("t")).is_trusted(&d.join("missing")));
    }
}
