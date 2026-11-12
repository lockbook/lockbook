//! The platform's text size setting, as a multiple of its default, where the
//! window's scale doesn't already carry it: Windows' Text size. macOS has
//! none; GNOME's reaches X11 apps through `Xft.dpi`.

use std::sync::atomic::{AtomicU32, Ordering};

/// The last scale read, as `f32` bits; 0 until one is.
static SCALE: AtomicU32 = AtomicU32::new(0);

pub fn scale() -> Option<f32> {
    let bits = SCALE.load(Ordering::Relaxed);
    (bits != 0).then(|| f32::from_bits(bits))
}

/// Reads the setting again off the UI thread, repainting `ctx` once it's
/// in: at launch, and as the window takes focus, since it's changed in the
/// system's settings.
pub fn refresh(ctx: &egui::Context) {
    #[cfg(windows)]
    {
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            SCALE.store(read().to_bits(), Ordering::Relaxed);
            ctx.request_repaint();
        });
    }
    #[cfg(not(windows))]
    let _ = ctx;
}

/// Settings > Accessibility > Text size, stored as a percentage; absent
/// until it's first changed.
#[cfg(windows)]
fn read() -> f32 {
    let hkcu = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
    let percent: u32 = hkcu
        .open_subkey("Software\\Microsoft\\Accessibility")
        .and_then(|key| key.get_value("TextScaleFactor"))
        .unwrap_or(100);
    percent as f32 / 100.0
}
