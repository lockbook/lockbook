//! Wash amounts and chromatic plates on top of workspace [`Theme`].

use crate::theme::palette_v2::{Mode, Palette};
use egui::Color32;

pub use crate::theme::palette_v2::{Theme, ThemeExt};

/// Toward `neutral_fg` on hover (rows / quiet on primary bg).
pub const FG_HOVER: f32 = Theme::HOVER_WASH;
/// Toward `neutral_fg` while pressed.
pub const FG_PRESS: f32 = Theme::PRESS_WASH;
/// Quiet on secondary bg: hover wash.
pub const QUIET_PLATE_HOVER: f32 = 0.12;
/// Quiet on secondary bg: press wash.
pub const QUIET_PLATE_PRESS: f32 = 0.18;
/// Solid primary fill: toward `neutral_bg` on hover.
pub const BG_HOVER: f32 = 0.16;
/// Solid primary fill: toward `neutral_bg` while pressed.
pub const BG_PRESS: f32 = 0.24;
/// Chip dismiss × press.
pub const CHIP_DISMISS_PRESS: f32 = 0.30;
/// Field/picker edge: neutral → neutral_fg when open/focused (× anim t).
pub const STROKE_EMPHASIS: f32 = 0.40;

/// Soft chromatic plate (~mnemonic 100 light / 900 dark) from the mode-near pole.
pub fn hue_wash(theme: &Theme, p: Palette) -> Color32 {
    hue_wash_from_poles(theme.current, theme.bright.get_color(p), theme.dim.get_color(p))
}

// Soft plate: mode-near pole → OKLCH reshape toward soft stops (~mnemonic 100 / 900).
// Light parent = bright; dark parent = dim.
const WASH_LIGHT_L_TARGET: f32 = 0.92;
const WASH_LIGHT_L_BLEND: f32 = 0.55; // L' = L + (target − L) × blend
const WASH_LIGHT_C: f32 = 0.62; // fraction of parent C (~step 100 / 200)
const WASH_DARK_L: f32 = 0.55;
const WASH_DARK_C: f32 = 0.75;
const WASH_ACHROMATIC_C: f32 = 0.02;

fn hue_wash_from_poles(mode: Mode, bright: Color32, dim: Color32) -> Color32 {
    let parent = match mode {
        Mode::Light => bright,
        Mode::Dark => dim,
    };
    let (l, c, h) = srgb_to_oklch(parent);
    let c_src = if c < WASH_ACHROMATIC_C { 0.0 } else { c };
    let (l2, c2) = match mode {
        Mode::Light => (l + (WASH_LIGHT_L_TARGET - l) * WASH_LIGHT_L_BLEND, c_src * WASH_LIGHT_C),
        Mode::Dark => (l * WASH_DARK_L, c_src * WASH_DARK_C),
    };
    oklch_to_srgb(l2, c2, h)
}

// OKLCH ↔ sRGB (Björn Ottosson), gamut clamped.

fn srgb_channel_to_linear(c: f32) -> f32 {
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

fn linear_to_srgb_channel(c: f32) -> f32 {
    let c = c.clamp(0.0, 1.0);
    if c <= 0.0031308 { 12.92 * c } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 }
}

// OKLab matrix coefficients from the reference transform (more digits than f32).
#[allow(clippy::excessive_precision)]
fn srgb_to_oklch(color: Color32) -> (f32, f32, f32) {
    let r = srgb_channel_to_linear(color.r() as f32 / 255.0);
    let g = srgb_channel_to_linear(color.g() as f32 / 255.0);
    let b = srgb_channel_to_linear(color.b() as f32 / 255.0);

    let l_ = 0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b;
    let m_ = 0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b;
    let s_ = 0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b;

    let l_ = l_.cbrt();
    let m_ = m_.cbrt();
    let s_ = s_.cbrt();

    let l = 0.2104542553 * l_ + 0.7936177850 * m_ - 0.0040720468 * s_;
    let a = 1.9779984951 * l_ - 2.4285922050 * m_ + 0.4505937099 * s_;
    let b = 0.0259040371 * l_ + 0.7827717662 * m_ - 0.8086757660 * s_;

    let c = (a * a + b * b).sqrt();
    let h = b.atan2(a).to_degrees().rem_euclid(360.0);
    (l, c, h)
}

#[allow(clippy::excessive_precision)]
fn oklch_to_srgb(l: f32, c: f32, h_deg: f32) -> Color32 {
    let h = h_deg.to_radians();
    let a = c * h.cos();
    let b = c * h.sin();

    let l_ = l + 0.3963377774 * a + 0.2158037573 * b;
    let m_ = l - 0.1055613458 * a - 0.0638541728 * b;
    let s_ = l - 0.0894841775 * a - 1.2914855480 * b;

    let l_ = l_ * l_ * l_;
    let m_ = m_ * m_ * m_;
    let s_ = s_ * s_ * s_;

    let r = 4.0767416621 * l_ - 3.3077115913 * m_ + 0.2309699292 * s_;
    let g = -1.2684380046 * l_ + 2.6097574011 * m_ - 0.3413193965 * s_;
    let b = -0.0041960863 * l_ - 0.7034186147 * m_ + 1.7076147010 * s_;

    Color32::from_rgb(
        (linear_to_srgb_channel(r) * 255.0)
            .round()
            .clamp(0.0, 255.0) as u8,
        (linear_to_srgb_channel(g) * 255.0)
            .round()
            .clamp(0.0, 255.0) as u8,
        (linear_to_srgb_channel(b) * 255.0)
            .round()
            .clamp(0.0, 255.0) as u8,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::palette_v2::Mode;

    #[test]
    fn hue_wash_differs_from_poles() {
        let t = Theme::default(Mode::Light);
        let washed = hue_wash(&t, Palette::Blue);
        assert_ne!(washed, t.bright.get_color(Palette::Blue));
        assert_ne!(washed, t.dim.get_color(Palette::Blue));
    }
}
