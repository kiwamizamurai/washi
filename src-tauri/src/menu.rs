use tauri::{
    image::Image,
    menu::{AboutMetadata, Menu, MenuItem, MenuItemBuilder, SubmenuBuilder},
    AppHandle, Emitter, Manager, Wry,
};

pub const MENU_EVENT: &str = "washi://menu";

pub mod id {
    pub const OPEN: &str = "open";
    pub const RELOAD: &str = "reload";
    pub const PRINT: &str = "print";
    pub const PASTE: &str = "paste";
    pub const FIND: &str = "find";
    pub const OUTLINE: &str = "outline";
    pub const FILES: &str = "files";
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
    pub const CHECK_UPDATES: &str = "check-updates";
    pub const AUTO_UPDATES: &str = "auto-updates";
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

pub fn build(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let menu = Menu::new(app)?;

    #[cfg(target_os = "macos")]
    {
        let application = SubmenuBuilder::new(app, "Washi")
            .about(Some(about_metadata()))
            .item(&item(app, id::CHECK_UPDATES, "Check for Updates…", None)?)
            .item(&item(app, id::AUTO_UPDATES, "Toggle Automatic Update Checks", None)?)
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

    let file = SubmenuBuilder::new(app, "File")
        .item(&item(app, id::OPEN, "Open…", Some("CmdOrCtrl+O"))?)
        .item(&item(app, id::RELOAD, "Reload", Some("CmdOrCtrl+R"))?)
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
        .item(&item(app, id::FILES, "Show/Hide Files", Some("CmdOrCtrl+B"))?)
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

