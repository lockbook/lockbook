//! The platform's text size setting, as a multiple of its default, where the
//! window's scale doesn't already carry it: Windows' Text size, KDE's font
//! size, and GNOME's Large Text under Wayland. macOS has none; GNOME's and
//! XFCE's reach X11 windows through `Xft.dpi`, which winit applies.

use std::sync::atomic::{AtomicU32, Ordering};
use winit::window::Window;

/// The last scale read, as `f32` bits; 0 until one is.
static SCALE: AtomicU32 = AtomicU32::new(0);

pub fn scale() -> Option<f32> {
    let bits = SCALE.load(Ordering::Relaxed);
    (bits != 0).then(|| f32::from_bits(bits))
}

/// Reads the setting again off the UI thread, repainting `ctx` once it's
/// in: at launch, and as the window takes focus, since it's changed in the
/// system's settings.
pub fn refresh(ctx: &egui::Context, window: &Window) {
    #[cfg(any(windows, target_os = "linux", target_os = "freebsd"))]
    {
        #[cfg(windows)]
        let read = {
            let _ = window;
            windows::read
        };
        #[cfg(not(windows))]
        let read = {
            let wayland = unix::is_wayland(window);
            move || unix::read(wayland)
        };
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            SCALE.store(read().to_bits(), Ordering::Relaxed);
            ctx.request_repaint();
        });
    }
    #[cfg(not(any(windows, target_os = "linux", target_os = "freebsd")))]
    let _ = (ctx, window);
}

#[cfg(windows)]
mod windows {
    /// Settings > Accessibility > Text size, stored as a percentage; absent
    /// until it's first changed.
    pub fn read() -> f32 {
        let hkcu = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
        let percent: u32 = hkcu
            .open_subkey("Software\\Microsoft\\Accessibility")
            .and_then(|key| key.get_value("TextScaleFactor"))
            .unwrap_or(100);
        percent as f32 / 100.0
    }
}

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
mod unix {
    use std::collections::HashMap;
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use winit::window::Window;
    use zbus::zvariant::OwnedValue;

    /// KDE's default General font size, in points.
    const KDE_DEFAULT_POINTS: f32 = 10.0;

    pub fn is_wayland(window: &Window) -> bool {
        window
            .window_handle()
            .is_ok_and(|h| matches!(h.as_raw(), RawWindowHandle::Wayland(_)))
    }

    /// Asks the desktop's settings portal, which answers inside Flatpak and
    /// Snap too. A desktop without the portal, or a setting left at its
    /// default, reads as 1.
    pub fn read(wayland: bool) -> f32 {
        let settings = read_all().unwrap_or_default();
        let kde = settings
            .get("org.kde.kdeglobals.General")
            .and_then(|g| g.get("font"))
            .and_then(|v| String::try_from(v.try_clone().ok()?).ok())
            .and_then(|font| kde_font_points(&font))
            .map_or(1.0, |points| points / KDE_DEFAULT_POINTS);
        // under X11, GNOME's settings daemon folds this into `Xft.dpi`
        let gnome = settings
            .get("org.gnome.desktop.interface")
            .filter(|_| wayland)
            .and_then(|g| g.get("text-scaling-factor"))
            .and_then(|v| f64::try_from(v).ok())
            .map_or(1.0, |f| f as f32);
        kde * gnome
    }

    type Namespaces = HashMap<String, HashMap<String, OwnedValue>>;

    fn read_all() -> zbus::Result<Namespaces> {
        let conn = zbus::blocking::Connection::session()?;
        let reply = conn.call_method(
            Some("org.freedesktop.portal.Desktop"),
            "/org/freedesktop/portal/desktop",
            Some("org.freedesktop.portal.Settings"),
            "ReadAll",
            // a slice, which goes as `as`; an array would go as a struct
            &(&["org.kde.kdeglobals.General", "org.gnome.desktop.interface"][..],),
        )?;
        reply.body().deserialize()
    }

    /// A Qt font description, `family,points,pixels,…`; `points` is -1 for a
    /// font sized in pixels.
    pub(super) fn kde_font_points(font: &str) -> Option<f32> {
        let points: f32 = font.split(',').nth(1)?.trim().parse().ok()?;
        (points > 0.0).then_some(points)
    }

    #[cfg(test)]
    mod tests {
        use super::kde_font_points;

        #[test]
        fn kde_font_points_reads_qt_descriptions() {
            assert_eq!(kde_font_points("Noto Sans,15,-1,5,400,0,0,0,0,0,0,0,0,0,0,1"), Some(15.0));
            assert_eq!(kde_font_points("Noto Sans,10.5,-1,5,50,0,0,0,0,0"), Some(10.5));
            assert_eq!(kde_font_points("Noto Sans,-1,14,5,50,0,0,0,0,0"), None);
            assert_eq!(kde_font_points("Noto Sans"), None);
        }
    }
}
