use std::{
    collections::{BTreeSet, HashMap},
    path::{Path, PathBuf},
    sync::Mutex,
};

use washi_core::tree;

#[derive(Default)]
pub struct PendingFiles(Mutex<HashMap<String, String>>);

impl PendingFiles {
    pub fn set(&self, label: &str, path: String) {
        self.0.lock().unwrap().insert(label.to_owned(), path);
    }

    pub fn take(&self, label: &str) -> Option<String> {
        self.0.lock().unwrap().remove(label)
    }

    pub fn has(&self, label: &str) -> bool {
        self.0.lock().unwrap().contains_key(label)
    }

    pub fn label_of(&self, path: &str) -> Option<String> {
        let waiting: Vec<(String, String)> = self
            .0
            .lock()
            .unwrap()
            .iter()
            .map(|(label, path)| (label.clone(), path.clone()))
            .collect();
        waiting
            .into_iter()
            .find(|(_, waiting)| same_file(waiting, path))
            .map(|(label, _)| label)
    }
}

#[derive(Default)]
pub struct TreeRoots(Mutex<HashMap<String, PathBuf>>);

impl TreeRoots {
    pub fn get(&self, label: &str) -> Option<PathBuf> {
        self.0.lock().unwrap().get(label).cloned()
    }

    pub fn set(&self, label: &str, root: PathBuf) {
        self.0.lock().unwrap().insert(label.to_owned(), root);
    }

    pub fn clear(&self, label: &str) {
        self.0.lock().unwrap().remove(label);
    }

    pub fn pin(&self, label: &str, document: &Path) {
        let kept = self.get(label).is_some_and(|root| tree::contains(&root, document));
        if kept {
            return;
        }
        if let Some(parent) = document.parent() {
            self.set(label, parent.to_path_buf());
        }
    }
}

#[derive(Default)]
pub struct DirtyWindows(Mutex<BTreeSet<String>>);

impl DirtyWindows {
    pub fn set(&self, label: &str, dirty: bool) {
        let mut labels = self.0.lock().unwrap();
        if dirty {
            labels.insert(label.to_owned());
        } else {
            labels.remove(label);
        }
    }

    pub fn labels(&self) -> Vec<String> {
        self.0.lock().unwrap().iter().cloned().collect()
    }
}

#[derive(Default)]
pub struct Documents(Mutex<HashMap<String, String>>);

impl Documents {
    pub fn open(&self, label: &str, path: &str) {
        self.0.lock().unwrap().insert(label.to_owned(), path.to_owned());
    }

    pub fn close(&self, label: &str) {
        self.0.lock().unwrap().remove(label);
    }

    pub fn has(&self, label: &str) -> bool {
        self.0.lock().unwrap().contains_key(label)
    }

    pub fn owns(&self, label: &str, path: &str) -> bool {
        let registered = self.0.lock().unwrap().get(label).cloned();
        registered.is_some_and(|registered| same_file(&registered, path))
    }

    pub fn path_of(&self, label: &str) -> Option<String> {
        self.0.lock().unwrap().get(label).cloned()
    }

    pub fn root_of(&self, label: &str) -> Option<PathBuf> {
        let path = self.path_of(label)?;
        Path::new(&path).parent().map(Path::to_path_buf)
    }

    pub fn label_of(&self, path: &str) -> Option<String> {
        let opened: Vec<(String, String)> = self
            .0
            .lock()
            .unwrap()
            .iter()
            .map(|(label, path)| (label.clone(), path.clone()))
            .collect();
        opened
            .into_iter()
            .find(|(_, opened)| same_file(opened, path))
            .map(|(label, _)| label)
    }
}

fn same_file(a: &str, b: &str) -> bool {
    a == b
        || matches!(
            (std::fs::canonicalize(a), std::fs::canonicalize(b)),
            (Ok(a), Ok(b)) if a == b
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dirty_windows_are_tracked_per_label() {
        let dirty = DirtyWindows::default();
        assert!(dirty.labels().is_empty());
        dirty.set("a", true);
        dirty.set("b", true);
        dirty.set("a", true);
        assert_eq!(dirty.labels(), vec!["a", "b"]);
        dirty.set("a", false);
        assert_eq!(dirty.labels(), vec!["b"]);
    }

    #[test]
    fn pending_is_per_window_and_taken_once() {
        let pending = PendingFiles::default();
        pending.set("main", "a.md".into());
        pending.set("w2", "b.md".into());
        assert!(pending.has("main"));
        assert_eq!(pending.take("main").as_deref(), Some("a.md"));
        assert_eq!(pending.take("main"), None);
        assert_eq!(pending.take("w2").as_deref(), Some("b.md"));
    }

    #[test]
    fn a_window_owns_only_the_file_it_has_open() {
        let dir = std::env::temp_dir().join(format!("washi-owns-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (a, b) = (dir.join("a.md"), dir.join("b.md"));
        std::fs::write(&a, "a").unwrap();
        std::fs::write(&b, "b").unwrap();
        let docs = Documents::default();
        docs.open("main", a.to_str().unwrap());
        assert!(docs.owns("main", a.to_str().unwrap()));
        assert!(!docs.owns("main", b.to_str().unwrap()), "別のファイルには書かせない");
        assert!(!docs.owns("other", a.to_str().unwrap()), "開いていないウィンドウには書かせない");
        assert!(!docs.owns("main", "/etc/hosts"));
        let dotted = format!("{}/./a.md", dir.display());
        assert!(docs.owns("main", &dotted));
    }

    #[test]
    fn documents_track_which_window_shows_which_file() {
        let docs = Documents::default();
        assert!(!docs.has("main"));
        docs.open("main", "a.md");
        docs.open("w2", "b.md");
        assert_eq!(docs.label_of("b.md").as_deref(), Some("w2"));
        assert_eq!(docs.label_of("c.md"), None);
        docs.open("w2", "c.md");
        assert_eq!(docs.label_of("b.md"), None);
        docs.close("w2");
        assert!(!docs.has("w2"));
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("washi-state-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn documents_know_the_folder_they_are_in() {
        let documents = Documents::default();
        documents.open("main", "/notes/a.md");
        assert_eq!(documents.path_of("main").as_deref(), Some("/notes/a.md"));
        assert_eq!(documents.root_of("main"), Some(PathBuf::from("/notes")));
        assert_eq!(documents.root_of("other"), None);
    }

    #[test]
    fn a_file_waiting_to_open_is_found_by_path() {
        let pending = PendingFiles::default();
        pending.set("tab-1", "/notes/a.md".into());
        assert_eq!(pending.label_of("/notes/a.md").as_deref(), Some("tab-1"));
        assert_eq!(pending.label_of("/notes/b.md"), None);
        pending.take("tab-1");
        assert_eq!(pending.label_of("/notes/a.md"), None);
    }

    #[test]
    fn the_same_file_is_found_under_another_spelling() {
        let dir = scratch("spelling");
        let file = dir.join("a.md");
        std::fs::write(&file, "x").unwrap();
        let documents = Documents::default();
        documents.open("main", file.to_str().unwrap());
        let dotted = dir.join(".").join("a.md");
        assert_eq!(documents.label_of(dotted.to_str().unwrap()).as_deref(), Some("main"));
        assert_eq!(documents.label_of(dir.join("b.md").to_str().unwrap()), None);
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_to_an_open_file_finds_its_window() {
        let dir = scratch("link");
        let file = dir.join("a.md");
        std::fs::write(&file, "x").unwrap();
        let link = dir.join("link.md");
        std::os::unix::fs::symlink(&file, &link).unwrap();
        let documents = Documents::default();
        documents.open("w1", file.to_str().unwrap());
        assert_eq!(documents.label_of(link.to_str().unwrap()).as_deref(), Some("w1"));
        assert!(documents.owns("w1", link.to_str().unwrap()));
    }

    #[test]
    fn a_window_keeps_its_root_while_documents_stay_inside_it() {
        let dir = scratch("roots");
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("a.md"), "x").unwrap();
        std::fs::write(dir.join("sub/b.md"), "x").unwrap();
        let roots = TreeRoots::default();
        roots.pin("w", &dir.join("a.md"));
        assert_eq!(roots.get("w"), Some(dir.clone()));
        roots.pin("w", &dir.join("sub/b.md"));
        assert_eq!(roots.get("w"), Some(dir));
    }

    #[test]
    fn a_window_moves_its_root_when_a_document_is_outside_it() {
        let dir = scratch("moves");
        std::fs::create_dir_all(dir.join("one")).unwrap();
        std::fs::create_dir_all(dir.join("two")).unwrap();
        std::fs::write(dir.join("one/a.md"), "x").unwrap();
        std::fs::write(dir.join("two/b.md"), "x").unwrap();
        let roots = TreeRoots::default();
        roots.pin("w", &dir.join("one/a.md"));
        roots.pin("w", &dir.join("two/b.md"));
        assert_eq!(roots.get("w"), Some(dir.join("two")));
    }

    #[test]
    fn a_root_that_was_set_ahead_of_time_is_kept() {
        let dir = scratch("inherit");
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("sub/b.md"), "x").unwrap();
        let roots = TreeRoots::default();
        roots.set("child", dir.clone());
        roots.pin("child", &dir.join("sub/b.md"));
        assert_eq!(roots.get("child"), Some(dir));
        roots.clear("child");
        assert_eq!(roots.get("child"), None);
    }
}
