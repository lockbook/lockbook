//! macOS NSWindow tweaks for custom chrome under a transparent titlebar.

use raw_window_handle::{HasWindowHandle, RawWindowHandle};

/// Turn off AppKit's automatic "drag anywhere in the titlebar band" behavior.
///
/// With `fullSizeContentView` + transparent titlebar, macOS still treats the
/// top strip as a window-move region even when tabs/toolbar paint there, which
/// races with tab reorder DnD. `isMovable = false` disables that path; free
/// chrome still moves the window via `performWindowDragWithEvent` (`StartDrag`).
///
/// Call once after the window exists (desktop host or eframe).
pub fn disable_automatic_titlebar_drag(window: &impl HasWindowHandle) {
    let Ok(handle) = window.window_handle() else {
        return;
    };
    let RawWindowHandle::AppKit(appkit) = handle.as_raw() else {
        return;
    };
    let ns_view_ptr = appkit.ns_view.as_ptr();
    if ns_view_ptr.is_null() {
        return;
    }

    // SAFETY: host's AppKit handle is a live NSView on the main thread.
    unsafe {
        use objc2::rc::Retained;
        use objc2_app_kit::NSView;

        let Some(view) = Retained::retain(ns_view_ptr.cast::<NSView>()) else {
            return;
        };
        let Some(ns_window) = view.window() else {
            return;
        };
        ns_window.setMovable(false);
        ns_window.setMovableByWindowBackground(false);
    }
}
