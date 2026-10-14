use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::{ipc::Response, AppHandle, State, WebviewWindow};
use washi_core::{
    editor,
    files::{self, DiskText, SaveResult},
    render,
    tree::{self, Limits},
    update,
};

use crate::{
    launch,
    state::{DirtyWindows, Documents, PendingFiles, TreeRoots},
    watch::FileWatcher,
};

#[tauri::command]
pub fn supported_extensions() -> Vec<&'static str> {
    render::supported_extensions()
}

#[tauri::command]
pub fn initial_file(
    window: WebviewWindow,
    pending: State<PendingFiles>,
    documents: State<Documents>,
) -> Option<String> {
    let path = pending.take(window.label())?;
    documents.open(window.label(), &path);
    Some(path)
}

#[tauri::command]
pub async fn latest_release() -> Result<update::Release, String> {
    tauri::async_runtime::spawn_blocking(update::fetch_latest)
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn print(window: WebviewWindow) -> Result<(), String> {
    window.print().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn render(
    window: WebviewWindow,
    watcher: State<'_, FileWatcher>,
    path: String,
) -> Result<Response, String> {
    let (output, dirs) = tauri::async_runtime::spawn_blocking(move || {
        let path = PathBuf::from(path);
        let output = render::render(&path);
        (output, render::dependency_dirs(&path))
    })
    .await
    .map_err(|e| e.to_string())?;
    watcher.set_dependencies(window.label(), &dirs);
    Ok(Response::new(output?.into_wire()))
}

#[tauri::command]
pub async fn render_buffer(
    window: WebviewWindow,
    watcher: State<'_, FileWatcher>,
    path: String,
    text: String,
) -> Result<Response, String> {
    let (rendered, dirs) = tauri::async_runtime::spawn_blocking(move || {
        let path = PathBuf::from(path);
        let rendered = render::render_buffer(&path, &text);
        (rendered, render::buffer_dependency_dirs(&path, &text))
    })
    .await
    .map_err(|e| e.to_string())?;
    watcher.set_dependencies(window.label(), &dirs);
    Ok(Response::new(rendered.into_wire()))
}

#[tauri::command]
pub async fn autocomplete(path: String, text: String, offset: usize, explicit: bool) -> Result<render::Completions, String> {
    tauri::async_runtime::spawn_blocking(move || render::complete(&PathBuf::from(path), &text, offset, explicit))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn read_text(path: String) -> Result<DiskText, String> {
    tauri::async_runtime::spawn_blocking(move || files::read_text(&PathBuf::from(path)))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn write_file(
    window: WebviewWindow,
    documents: State<'_, Documents>,
    path: String,
    text: String,
    base_hash: Option<String>,
    force: Option<bool>,
) -> Result<SaveResult, String> {
    if !documents.owns(window.label(), &path) {
        return Err("Can only save the file this window has open".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        files::write_file(&PathBuf::from(path), &text, base_hash.as_deref(), force.unwrap_or(false))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn set_dirty(window: WebviewWindow, dirty_windows: State<DirtyWindows>, dirty: bool) {
    dirty_windows.set(window.label(), dirty);
}

#[tauri::command]
pub async fn render_text(text: String) -> Result<Response, String> {
    let output = tauri::async_runtime::spawn_blocking(move || render::render_text(&text))
        .await
        .map_err(|e| e.to_string())??;
    Ok(Response::new(output.into_wire()))
}

#[tauri::command]
pub async fn open_source(
    window: WebviewWindow,
    documents: State<'_, Documents>,
    roots: State<'_, TreeRoots>,
    file: String,
    line: usize,
    column: usize,
) -> Result<String, String> {
    let root = roots
        .get(window.label())
        .or_else(|| documents.root_of(window.label()))
        .ok_or_else(|| "no document is open in this window".to_owned())?;
    let file = PathBuf::from(file);
    if !tree::contains(&root, &file) {
        return Err(format!("{} is outside {}", file.display(), root.display()));
    }
    tauri::async_runtime::spawn_blocking(move || {
        let name = file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        editor::open(&render::SourceLocation { file, line, column })?;
        Ok(format!("{name}:{line}"))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn watch(
    app: AppHandle,
    window: WebviewWindow,
    watcher: State<FileWatcher>,
    documents: State<Documents>,
    roots: State<TreeRoots>,
    path: String,
) -> Result<(), String> {
    documents.open(window.label(), &path);
    roots.pin(window.label(), Path::new(&path));
    watcher.watch(app, window.label(), &PathBuf::from(path))
}

#[derive(Serialize)]
pub struct DocumentList {
    root: String,
    files: Vec<String>,
    truncated: bool,
}

#[tauri::command]
pub async fn list_documents(
    window: WebviewWindow,
    documents: State<'_, Documents>,
    roots: State<'_, TreeRoots>,
) -> Result<DocumentList, String> {
    let Some(root) = roots.get(window.label()).or_else(|| documents.root_of(window.label())) else {
        return Ok(DocumentList { root: String::new(), files: Vec::new(), truncated: false });
    };
    tauri::async_runtime::spawn_blocking(move || {
        let scan = tree::scan(&root, Limits::default())
            .map_err(|e| format!("{}: {e}", root.display()))?;
        Ok(DocumentList {
            root: root.to_string_lossy().into_owned(),
            files: scan.files,
            truncated: scan.truncated,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn open_in_tab(
    app: AppHandle,
    window: WebviewWindow,
    documents: State<'_, Documents>,
    roots: State<'_, TreeRoots>,
    path: String,
) -> Result<(), String> {
    let root = roots
        .get(window.label())
        .or_else(|| documents.root_of(window.label()))
        .ok_or_else(|| "no document is open in this window".to_owned())?;
    let target = PathBuf::from(&path);
    if !tree::contains(&root, &target) {
        return Err(format!("{path} is outside {}", root.display()));
    }
    let path = launch::openable(&target).ok_or_else(|| format!("unsupported format: {path}"))?;
    launch::open_document_in_tab(&app, &window, path, root);
    Ok(())
}
