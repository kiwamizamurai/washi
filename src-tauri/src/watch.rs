use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::Mutex,
};

use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tauri::{AppHandle, Emitter};

pub const CHANGED_EVENT: &str = "washi://changed";

struct Watching {
    watcher: RecommendedWatcher,
    base: PathBuf,
    dirs: HashSet<PathBuf>,
}

impl Watching {
    fn new(base: &Path, on_change: impl Fn() + Send + 'static) -> Result<Self, String> {
        let watcher = start(base, on_change)?;
        Ok(Self { watcher, base: base.to_path_buf(), dirs: HashSet::from([base.to_path_buf()]) })
    }

    fn sync(&mut self, extra: &[PathBuf]) {
        let wanted: HashSet<PathBuf> = std::iter::once(self.base.clone()).chain(extra.iter().cloned()).collect();
        for gone in self.dirs.difference(&wanted).cloned().collect::<Vec<_>>() {
            let _ = self.watcher.unwatch(&gone);
            self.dirs.remove(&gone);
        }
        for added in wanted.difference(&self.dirs.clone()) {
            if self.watcher.watch(added, RecursiveMode::NonRecursive).is_ok() {
                self.dirs.insert(added.clone());
            }
        }
    }
}

#[derive(Default)]
pub struct FileWatcher(Mutex<HashMap<String, Watching>>);

impl FileWatcher {
    pub fn watch(&self, app: AppHandle, label: &str, path: &Path) -> Result<(), String> {
        let dir = path.parent().ok_or("cannot determine the folder")?;
        let target = label.to_owned();
        let watching = Watching::new(dir, move || {
            let _ = app.emit_to(target.as_str(), CHANGED_EVENT, ());
        })?;
        self.0.lock().unwrap().insert(label.to_owned(), watching);
        Ok(())
    }

    pub fn set_dependencies(&self, label: &str, dirs: &[PathBuf]) {
        if let Some(watching) = self.0.lock().unwrap().get_mut(label) {
            watching.sync(dirs);
        }
    }

    pub fn forget(&self, label: &str) {
        self.0.lock().unwrap().remove(label);
    }
}

fn start(dir: &Path, on_change: impl Fn() + Send + 'static) -> Result<RecommendedWatcher, String> {
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if let Ok(event) = res {
            if is_relevant(&event) {
                on_change();
            }
        }
    })
    .map_err(|e| e.to_string())?;
    watcher
        .watch(dir, RecursiveMode::NonRecursive)
        .map_err(|e| e.to_string())?;
    Ok(watcher)
}

fn is_relevant(event: &notify::Event) -> bool {
    matches!(event.kind, EventKind::Create(_) | EventKind::Modify(_))
        && event.paths.iter().any(|p| !is_noise(p))
}

fn is_noise(path: &Path) -> bool {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    name.starts_with('.') || name.starts_with('#') || name.ends_with('~') || name.ends_with(".swp")
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify::event::{CreateKind, ModifyKind, RemoveKind};

    fn event(kind: EventKind, path: &str) -> notify::Event {
        notify::Event::new(kind).add_path(path.into())
    }

    use std::{fs, sync::mpsc, time::Duration};

    fn settle(rx: &mpsc::Receiver<()>) {
        while rx.recv_timeout(Duration::from_millis(600)).is_ok() {}
    }

    fn watched_dir(name: &str) -> (std::path::PathBuf, RecommendedWatcher, mpsc::Receiver<()>) {
        let dir = std::env::temp_dir().join(format!("washi-watch-{name}-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("doc.md"), "v1").unwrap();
        let (tx, rx) = mpsc::channel();
        let watcher = start(&dir, move || {
            let _ = tx.send(());
        })
        .unwrap();
        std::thread::sleep(Duration::from_millis(300));
        settle(&rx);
        (dir, watcher, rx)
    }

    #[test]
    fn plain_writes_trigger_a_change() {
        let (dir, _watcher, rx) = watched_dir("plain");
        fs::write(dir.join("doc.md"), "v2").unwrap();
        assert!(rx.recv_timeout(Duration::from_secs(5)).is_ok());
    }

    #[test]
    fn atomic_saves_trigger_a_change() {
        let (dir, _watcher, rx) = watched_dir("atomic");
        let temp = dir.join("doc.md.tmp");
        fs::write(&temp, "v2").unwrap();
        fs::rename(&temp, dir.join("doc.md")).unwrap();
        assert!(rx.recv_timeout(Duration::from_secs(5)).is_ok());
    }

    #[test]
    fn finder_metadata_files_do_not_trigger_a_change() {
        let (dir, _watcher, rx) = watched_dir("noise");
        fs::write(dir.join(".DS_Store"), "x").unwrap();
        assert!(rx.recv_timeout(Duration::from_millis(800)).is_err());
    }

    #[test]
    fn ignores_editor_and_finder_noise() {
        for name in ["/d/.DS_Store", "/d/#a.md#", "/d/a.md~", "/d/.a.md.swp", "/d/a.swp"] {
            assert!(is_noise(Path::new(name)), "{name}");
        }
        assert!(!is_noise(Path::new("/d/a.md")));
    }

    #[test]
    fn reacts_to_writes_only() {
        assert!(is_relevant(&event(EventKind::Modify(ModifyKind::Any), "/d/a.typ")));
        assert!(is_relevant(&event(EventKind::Create(CreateKind::File), "/d/a.typ")));
        assert!(!is_relevant(&event(EventKind::Remove(RemoveKind::File), "/d/a.typ")));
        assert!(!is_relevant(&event(EventKind::Modify(ModifyKind::Any), "/d/.DS_Store")));
    }

    fn project(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("washi-watch-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("chapters")).unwrap();
        fs::write(dir.join("main.tex"), "x").unwrap();
        fs::write(dir.join("chapters/a.tex"), "v1").unwrap();
        dir
    }

    fn watching(dir: &Path) -> (Watching, mpsc::Receiver<()>) {
        let (tx, rx) = mpsc::channel();
        let watching = Watching::new(dir, move || {
            let _ = tx.send(());
        })
        .unwrap();
        std::thread::sleep(Duration::from_millis(300));
        settle(&rx);
        (watching, rx)
    }

    #[test]
    fn dependency_folders_trigger_changes_once_added() {
        let dir = project("sub-after");
        let (mut watching, rx) = watching(&dir);
        watching.sync(&[dir.join("chapters")]);
        std::thread::sleep(Duration::from_millis(300));
        fs::write(dir.join("chapters/a.tex"), "v2").unwrap();
        assert!(rx.recv_timeout(Duration::from_secs(5)).is_ok());
    }

    #[test]
    fn removed_dependency_folders_are_no_longer_watched() {
        let dir = project("sub-removed");
        let (mut watching, rx) = watching(&dir);
        watching.sync(&[dir.join("chapters")]);
        assert_eq!(watching.dirs, HashSet::from([dir.clone(), dir.join("chapters")]));
        watching.sync(&[]);
        assert_eq!(watching.dirs, HashSet::from([dir.clone()]));
        std::thread::sleep(Duration::from_millis(300));
        settle(&rx);
        fs::write(dir.join("main.tex"), "y").unwrap();
        assert!(rx.recv_timeout(Duration::from_secs(5)).is_ok());
    }

    #[test]
    fn missing_dependency_folders_are_skipped_without_losing_the_base() {
        let dir = project("sub-missing");
        let (mut watching, rx) = watching(&dir);
        watching.sync(&[dir.join("does-not-exist")]);
        assert_eq!(watching.dirs, HashSet::from([dir.clone()]));
        fs::write(dir.join("main.tex"), "y").unwrap();
        assert!(rx.recv_timeout(Duration::from_secs(5)).is_ok());
    }
}
