use std::path::{Path, PathBuf};

use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use washi_core::render;

use crate::{
    state::{Documents, PendingFiles, TreeRoots},
    tabs,
};

pub const OPEN_EVENT: &str = "washi://open";

pub const TAB_GROUP: &str = "washi";

const DOCUMENT_LABEL_PREFIX: &str = "doc-";
const TAB_LABEL_PREFIX: &str = "tab-";

const MAIN_WINDOW: &str = "main";
const WINDOW_SIZE: (f64, f64) = (900.0, 1000.0);

pub fn remembers_geometry(label: &str) -> bool {
    !label.starts_with(TAB_LABEL_PREFIX)
}

pub fn openable(path: &Path) -> Option<String> {
    render::renderer_for(path).map(|_| path.to_string_lossy().into_owned())
}

pub fn openable_paths<I, P>(paths: I) -> Vec<String>
where
    I: IntoIterator<Item = P>,
    P: AsRef<Path>,
{
    paths.into_iter().filter_map(|p| openable(p.as_ref())).collect()
}

pub fn open_documents(app: &AppHandle, paths: Vec<String>) {
    for path in paths {
        open_document(app, path);
    }
}

fn already_open(app: &AppHandle, path: &str) -> Option<String> {
    app.state::<Documents>()
        .label_of(path)
        .or_else(|| app.state::<PendingFiles>().label_of(path))
}

fn open_document(app: &AppHandle, path: String) {
    if let Some(label) = already_open(app, &path) {
        focus(app, &label);
        return;
    }
    let documents = app.state::<Documents>();
    let pending = app.state::<PendingFiles>();
    let Some(label) = idle_window(app, &documents, &pending).or_else(|| create_window(app, DOCUMENT_LABEL_PREFIX, true)) else {
        return;
    };
    start(app, &label, path);
}

pub fn open_document_in_tab(app: &AppHandle, parent: &WebviewWindow, path: String, root: PathBuf) {
    if let Some(label) = already_open(app, &path) {
        focus(app, &label);
        return;
    }
    let Some(label) = create_window(app, TAB_LABEL_PREFIX, false) else { return };
    app.state::<TreeRoots>().set(&label, root);
    app.state::<PendingFiles>().set(&label, path);
    let Some(child) = app.get_webview_window(&label) else { return };
    let parent = parent.clone();
    let task_app = app.clone();
    let scheduled = app.run_on_main_thread(move || {
        tabs::attach(&parent, &child);
        let _ = child.show();
        announce(&task_app, child.label());
    });
    if scheduled.is_err() {
        if let Some(child) = app.get_webview_window(&label) {
            let _ = child.show();
        }
        announce(app, &label);
    }
}

fn start(app: &AppHandle, label: &str, path: String) {
    app.state::<PendingFiles>().set(label, path);
    announce(app, label);
}

fn announce(app: &AppHandle, label: &str) {
    let _ = app.emit_to(label, OPEN_EVENT, ());
    focus(app, label);
}

fn idle_window(app: &AppHandle, documents: &Documents, pending: &PendingFiles) -> Option<String> {
    let mut labels: Vec<String> = app.webview_windows().into_keys().collect();
    labels.sort_by_key(|l| l != MAIN_WINDOW);
    labels
        .into_iter()
        .find(|l| !documents.has(l) && !pending.has(l))
}

fn create_window(app: &AppHandle, prefix: &str, visible: bool) -> Option<String> {
    let label = format!("{prefix}{}", next_serial());
    let builder = WebviewWindowBuilder::new(app, &label, WebviewUrl::default())
        .title("Washi")
        .inner_size(WINDOW_SIZE.0, WINDOW_SIZE.1)
        .min_inner_size(480.0, 400.0)
        .visible(visible);
    #[cfg(target_os = "macos")]
    let builder = builder.tabbing_identifier(TAB_GROUP);
    builder.build().ok().map(|_| label)
}

fn next_serial() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SERIAL: AtomicU64 = AtomicU64::new(1);
    SERIAL.fetch_add(1, Ordering::Relaxed)
}

fn focus(app: &AppHandle, label: &str) {
    if let Some(window) = app.get_webview_window(label) {
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_only_supported_paths_in_order() {
        assert_eq!(
            openable_paths(["--flag", "notes.txt", "a.md", "b.typ", "c.docx"]),
            vec!["a.md", "b.typ"]
        );
        assert!(openable_paths(["x.docx"]).is_empty());
    }

    #[test]
    fn tabs_do_not_restore_a_saved_window_size() {
        assert!(remembers_geometry(MAIN_WINDOW));
        assert!(remembers_geometry(&format!("{DOCUMENT_LABEL_PREFIX}1")));
        assert!(!remembers_geometry(&format!("{TAB_LABEL_PREFIX}1")));
    }
}
