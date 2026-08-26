//! macOS NSWindow tweaks for custom chrome under a transparent titlebar.

use raw_window_handle::{HasWindowHandle, RawWindowHandle};

use objc2::rc::Retained;
use objc2_app_kit::{NSView, NSWindow};

fn ns_window(window: &impl HasWindowHandle) -> Option<Retained<NSWindow>> {
    let handle = window.window_handle().ok()?;
    let RawWindowHandle::AppKit(appkit) = handle.as_raw() else {
        return None;
    };
    let ns_view_ptr = appkit.ns_view.as_ptr();
    if ns_view_ptr.is_null() {
        return None;
    }
    // SAFETY: host's AppKit handle is a live NSView on the main thread.
    unsafe {
        let view = Retained::retain(ns_view_ptr.cast::<NSView>())?;
        view.window()
    }
}

/// Turn off AppKit's automatic "drag anywhere in the titlebar band" behavior.
///
/// With `fullSizeContentView` + transparent titlebar, macOS still treats the
/// top strip as a window-move region even when tabs/toolbar paint there, which
/// races with tab reorder DnD. `isMovable = false` disables that path; free
/// chrome still moves the window via `performWindowDragWithEvent` (`StartDrag`).
///
/// Call once after the window exists (desktop host or eframe).
pub fn disable_automatic_titlebar_drag(window: &impl HasWindowHandle) {
    let Some(ns_window) = ns_window(window) else {
        return;
    };
    ns_window.setMovable(false);
    ns_window.setMovableByWindowBackground(false);
}

/// AppKit resets the titlebar container each layout — re-pin every frame.
pub fn pin_traffic_lights(window: &impl HasWindowHandle) {
    center_traffic_lights(window, crate::shell::titlebar::HEADER_H as f64);
}

/// Vertically center close / miniaturize / zoom in a `header_h`-tall band.
pub fn center_traffic_lights(window: &impl HasWindowHandle, header_h: f64) {
    use objc2_app_kit::{NSTitlebarSeparatorStyle, NSWindowButton, NSWindowStyleMask};
    use objc2_foundation::NSPoint;

    let Some(ns_window) = ns_window(window) else {
        return;
    };

    unsafe {
        if ns_window
            .styleMask()
            .contains(NSWindowStyleMask::FullScreen)
        {
            return;
        }
        ns_window.setTitlebarSeparatorStyle(NSTitlebarSeparatorStyle::None);

        let Some(close) = ns_window.standardWindowButton(NSWindowButton::NSWindowCloseButton)
        else {
            return;
        };
        let Some(titlebar_view) = close.superview() else {
            return;
        };
        // NSTitlebarContainerView — sibling of the content view, top of the theme frame.
        let Some(container) = titlebar_view.superview() else {
            return;
        };

        let win_h = ns_window.frame().size.height;
        if win_h <= 0.0 || header_h <= 0.0 {
            return;
        }

        container.setClipsToBounds(false);
        titlebar_view.setClipsToBounds(false);

        let mut container_f = container.frame();
        container_f.size.height = header_h;
        container_f.origin.y = win_h - header_h;
        if !rect_near(container.frame(), container_f) {
            log::debug!("traffic lights: container {:?} → {:?}", container.frame(), container_f);
            container.setFrame(container_f);
        }

        // Grow height so the lights are not clipped; keep AppKit's x/width so
        // the container does not eat hits across the whole titleband.
        let mut title_f = titlebar_view.frame();
        title_f.origin.y = 0.0;
        title_f.size.height = header_h;
        if !rect_near(titlebar_view.frame(), title_f) {
            titlebar_view.setFrame(title_f);
        }

        let close_f = close.frame();
        let close_h = close_f.size.height;
        if close_h <= 0.0 {
            return;
        }
        // Absolute origin (Electron): never add a window-coord delta onto
        // possibly-reset frames — that one-frame mismatch is a visible jump.
        let inset = (header_h - close_h) / 2.0;
        let base_x = close_f.origin.x;

        let buttons = [
            NSWindowButton::NSWindowCloseButton,
            NSWindowButton::NSWindowMiniaturizeButton,
            NSWindowButton::NSWindowZoomButton,
        ];
        for b in buttons {
            let Some(btn) = ns_window.standardWindowButton(b) else {
                continue;
            };
            let frame = btn.frame();
            let h = frame.size.height;
            if h <= 0.0 {
                continue;
            }
            // Unflipped: y=0 is the bottom of the (now HEADER_H-tall) titlebar
            // view. Centering pad from bottom == pad from top.
            let y = (header_h - h) / 2.0;
            let x = inset + (frame.origin.x - base_x);
            if (frame.origin.x - x).abs() < 0.25 && (frame.origin.y - y).abs() < 0.25 {
                continue;
            }
            log::debug!(
                "traffic lights: button {} ({:.1}, {:.1}) → ({:.1}, {:.1})",
                b.0,
                frame.origin.x,
                frame.origin.y,
                x,
                y
            );
            btn.setFrameOrigin(NSPoint::new(x, y));
        }
    }
}

fn rect_near(a: objc2_foundation::NSRect, b: objc2_foundation::NSRect) -> bool {
    (a.origin.x - b.origin.x).abs() < 0.25
        && (a.origin.y - b.origin.y).abs() < 0.25
        && (a.size.width - b.size.width).abs() < 0.25
        && (a.size.height - b.size.height).abs() < 0.25
}

/// `AppleActionOnDoubleClick` from global defaults (`Maximize` / `Minimize` /
/// `None` / `Fill`). `None` if unset — caller treats that as zoom.
fn apple_action_on_double_click() -> Option<String> {
    use objc2_foundation::{NSString, NSUserDefaults};

    // SAFETY: AppKit main thread; `standardUserDefaults` is process-global.
    unsafe {
        let defaults = NSUserDefaults::standardUserDefaults();
        let key = NSString::from_str("AppleActionOnDoubleClick");
        defaults.stringForKey(&key).map(|s| s.to_string())
    }
}

/// User's double-click interval (System Settings → Mouse / Trackpad).
pub fn double_click_interval_secs() -> f64 {
    // SAFETY: class method; AppKit main thread.
    unsafe { objc2_app_kit::NSEvent::doubleClickInterval() }.max(0.2)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TitlebarDoubleClick {
    Zoom,
    Miniaturize,
    Ignore,
}

fn titlebar_double_click_action(pref: Option<&str>) -> TitlebarDoubleClick {
    match pref {
        Some("Minimize") => TitlebarDoubleClick::Miniaturize,
        Some("None") => TitlebarDoubleClick::Ignore,
        // `Maximize` is Settings “Zoom”; `Fill` (Sonoma+) ≈ zoom for 80/20.
        _ => TitlebarDoubleClick::Zoom,
    }
}

/// Run the system titlebar double-click action on this window.
pub fn perform_titlebar_double_click(window: &impl HasWindowHandle) {
    let Some(ns_window) = ns_window(window) else {
        return;
    };
    match titlebar_double_click_action(apple_action_on_double_click().as_deref()) {
        TitlebarDoubleClick::Miniaturize => unsafe { ns_window.performMiniaturize(None) },
        TitlebarDoubleClick::Ignore => {}
        TitlebarDoubleClick::Zoom => unsafe { ns_window.performZoom(None) },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn double_click_pref_maps() {
        assert_eq!(
            titlebar_double_click_action(Some("Minimize")),
            TitlebarDoubleClick::Miniaturize
        );
        assert_eq!(titlebar_double_click_action(Some("None")), TitlebarDoubleClick::Ignore);
        assert_eq!(titlebar_double_click_action(Some("Maximize")), TitlebarDoubleClick::Zoom);
        assert_eq!(titlebar_double_click_action(Some("Fill")), TitlebarDoubleClick::Zoom);
        assert_eq!(titlebar_double_click_action(None), TitlebarDoubleClick::Zoom);
    }
}
