use std::sync::Mutex;

use tauri::{
    image::Image,
    menu::{
        AboutMetadata, Menu, MenuItem, MenuItemBuilder, PredefinedMenuItem, Submenu, SubmenuBuilder,
    },
    AppHandle, Emitter, Manager, State, Wry,
};

pub const MENU_EVENT: &str = "washi://menu";

pub mod id {
    pub const OPEN: &str = "open";
    pub const RECENT_CLEAR: &str = "recent-clear";
    pub const RELOAD: &str = "reload";
    pub const PRINT: &str = "print";
    pub const PASTE: &str = "paste";
    pub const FIND: &str = "find";
    pub const OUTLINE: &str = "outline";
    pub const THEME_SYSTEM: &str = "theme-system";
    pub const THEME_LIGHT: &str = "theme-light";
    pub const THEME_DARK: &str = "theme-dark";
    pub const WIDTH_NARROW: &str = "width-narrow";
    pub const WIDTH_WIDE: &str = "width-wide";
    pub const WIDTH_FULL: &str = "width-full";
    pub const ZOOM_IN: &str = "zoom-in";
    pub const ZOOM_OUT: &str = "zoom-out";
    pub const ZOOM_RESET: &str = "zoom-reset";
    pub const SAVE: &str = "save";
    pub const EDIT: &str = "edit";
    pub const UNDO: &str = "undo";
    pub const REDO: &str = "redo";
    pub const AUTOSAVE: &str = "autosave";
    pub const SYNC_CURSOR: &str = "sync-cursor";
    pub const PALETTE: &str = "palette";
}

fn item(app: &AppHandle, id: &str, label: &str, accelerator: Option<&str>) -> tauri::Result<MenuItem<Wry>> {
    let builder = MenuItemBuilder::with_id(id, label);
    match accelerator {
        Some(accelerator) => builder.accelerator(accelerator).build(app),
        None => builder.build(app),
    }
}

fn about_metadata() -> AboutMetadata<'static> {
    AboutMetadata {
        name: Some("Washi".into()),
        version: Some(env!("CARGO_PKG_VERSION").into()),
        icon: Image::from_bytes(include_bytes!("../icons/128x128@2x.png")).ok(),
        ..Default::default()
    }
}

/// Handle to the "Open Recent" submenu so the frontend can rebuild it
/// whenever the recent-files list changes.
#[derive(Default)]
pub struct RecentMenu(Mutex<Option<Submenu<Wry>>>);

/// Rebuild the "Open Recent" submenu from `paths` (newest first). Runs on the
/// main thread: `set_recent_files` hops over from the webview thread.
fn rebuild_recent(
    app: &AppHandle,
    submenu: &Submenu<Wry>,
    paths: &[String],
) -> tauri::Result<()> {
    for kind in submenu.items()? {
        submenu.remove(&kind)?;
    }

    if paths.is_empty() {
        let empty = MenuItemBuilder::with_id("recent-empty", "No Recent Files")
            .enabled(false)
            .build(app)?;
        submenu.append(&empty)?;
    } else {
        for (i, path) in paths.iter().enumerate() {
            let item = MenuItem::with_id(app, format!("recent:{i}"), path, true, None::<&str>)?;
            submenu.append(&item)?;
        }
    }

    submenu.append(&PredefinedMenuItem::separator(app)?)?;
    submenu.append(&item(app, id::RECENT_CLEAR, "Clear Menu", None)?)?;
    Ok(())
}

/// Called from the frontend after the recent-files list changes.
pub fn set_recent_files(
    app: &AppHandle,
    recent: State<'_, RecentMenu>,
    paths: Vec<String>,
) -> Result<(), String> {
    let app = app.clone();
    let submenu = recent.0.lock().unwrap().clone();
    app.clone().run_on_main_thread(move || {
        if let Some(submenu) = submenu.as_ref() {
            let _ = rebuild_recent(&app, submenu, &paths);
        }
    })
    .map_err(|e| e.to_string())
}

pub fn build(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let menu = Menu::new(app)?;

    #[cfg(target_os = "macos")]
    {
        let application = SubmenuBuilder::new(app, "Washi")
            .about(Some(about_metadata()))
            .separator()
            .services()
            .separator()
            .hide()
            .hide_others()
            .show_all()
            .separator()
            .quit()
            .build()?;
        menu.append(&application)?;
    }

    let recent_files = SubmenuBuilder::new(app, "Open Recent").build()?;
    {
        let recent_state = app.state::<RecentMenu>();
        let mut guard = recent_state.0.lock().unwrap();
        *guard = Some(recent_files.clone());
    }
    rebuild_recent(app, &recent_files, &[])?;

    let file = SubmenuBuilder::new(app, "File")
        .item(&item(app, id::OPEN, "Open…", Some("CmdOrCtrl+O"))?)
        .item(&item(app, id::RELOAD, "Reload", Some("CmdOrCtrl+R"))?)
        .item(&recent_files)
        .separator()
        .item(&item(app, id::SAVE, "Save", Some("CmdOrCtrl+S"))?)
        .item(&item(app, id::AUTOSAVE, "Toggle Autosave", None)?)
        .separator()
        .item(&item(app, id::PRINT, "Print…", Some("CmdOrCtrl+P"))?)
        .separator()
        .close_window()
        .build()?;

    let edit = SubmenuBuilder::new(app, "Edit")
        .item(&item(app, id::UNDO, "Undo", Some("CmdOrCtrl+Z"))?)
        .item(&item(app, id::REDO, "Redo", Some("CmdOrCtrl+Shift+Z"))?)
        .separator()
        .cut()
        .copy()
        .select_all()
        .separator()
        .item(&item(app, id::PASTE, "Paste and Open", Some("CmdOrCtrl+V"))?)
        .separator()
        .item(&item(app, id::FIND, "Find…", Some("CmdOrCtrl+F"))?)
        .build()?;

    let theme = SubmenuBuilder::new(app, "Theme")
        .item(&item(app, id::THEME_SYSTEM, "Match System", None)?)
        .item(&item(app, id::THEME_LIGHT, "Light", None)?)
        .item(&item(app, id::THEME_DARK, "Dark", None)?)
        .build()?;

    let width = SubmenuBuilder::new(app, "Text Width")
        .item(&item(app, id::WIDTH_NARROW, "Standard", None)?)
        .item(&item(app, id::WIDTH_WIDE, "Wide", None)?)
        .item(&item(app, id::WIDTH_FULL, "Full Window", None)?)
        .build()?;

    let view = SubmenuBuilder::new(app, "View")
        .item(&item(app, id::PALETTE, "Command Palette…", Some("CmdOrCtrl+K"))?)
        .separator()
        .item(&item(app, id::EDIT, "Edit (Split View)", Some("CmdOrCtrl+E"))?)
        .item(&item(app, id::SYNC_CURSOR, "Show Cursor Line in Preview", None)?)
        .separator()
        .item(&item(app, id::OUTLINE, "Show/Hide Outline", Some("CmdOrCtrl+Shift+O"))?)
        .separator()
        .item(&item(app, id::ZOOM_IN, "Zoom In", Some("CmdOrCtrl+="))?)
        .item(&item(app, id::ZOOM_OUT, "Zoom Out", Some("CmdOrCtrl+-"))?)
        .item(&item(app, id::ZOOM_RESET, "Actual Size", Some("CmdOrCtrl+0"))?)
        .separator()
        .item(&theme)
        .item(&width)
        .separator()
        .fullscreen()
        .build()?;

    let window = SubmenuBuilder::new(app, "Window")
        .minimize()
        .maximize()
        .build()?;

    menu.append(&file)?;
    menu.append(&edit)?;
    menu.append(&view)?;
    menu.append(&window)?;
    Ok(menu)
}

pub fn dispatch(app: &AppHandle, menu_id: &str) {
    let focused = app
        .webview_windows()
        .into_values()
        .find(|w| w.is_focused().unwrap_or(false));
    if let Some(window) = focused {
        let _ = app.emit_to(window.label(), MENU_EVENT, menu_id);
    }
}

#[cfg(test)]
mod tests {
    use super::id::*;

    #[test]
    fn ids_are_unique() {
        let all = [
            OPEN, RECENT_CLEAR, RELOAD, PRINT, PASTE, FIND, OUTLINE, THEME_SYSTEM, THEME_LIGHT,
            THEME_DARK, WIDTH_NARROW, WIDTH_WIDE, WIDTH_FULL, ZOOM_IN, ZOOM_OUT, ZOOM_RESET, SAVE,
            EDIT, UNDO, REDO, AUTOSAVE, SYNC_CURSOR, PALETTE,
        ];
        let mut sorted = all.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), all.len());
    }
}
