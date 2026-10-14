use tauri::WebviewWindow;

#[cfg(target_os = "macos")]
pub fn attach(parent: &WebviewWindow, child: &WebviewWindow) -> bool {
    use objc2_app_kit::{NSWindow, NSWindowOrderingMode};
    let (Ok(parent), Ok(child)) = (parent.ns_window(), child.ns_window()) else {
        return false;
    };
    let (parent, child) = unsafe { (&*parent.cast::<NSWindow>(), &*child.cast::<NSWindow>()) };
    parent.addTabbedWindow_ordered(child, NSWindowOrderingMode::Above);
    true
}

#[cfg(not(target_os = "macos"))]
pub fn attach(_parent: &WebviewWindow, _child: &WebviewWindow) -> bool {
    false
}
