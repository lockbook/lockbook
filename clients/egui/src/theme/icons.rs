//! Phosphor icons — a vendored subset. The regular Phosphor TTF is embedded and
//! registered under its own font family, so icons render crisply at any size
//! independent of the text fonts. Codepoints are lifted from egui-phosphor 0.12
//! (regular variant); we vendor the font rather than depend on the crate to
//! avoid egui version lockstep — the file is just glyph bytes and never breaks.

use egui::{FontData, FontDefinitions, FontFamily, FontId};

/// The family the embedded Phosphor font registers under.
const FAMILY: &str = "phosphor";

pub const FOLDER: &str = "\u{E24A}";
pub const FOLDER_OPEN: &str = "\u{E256}";
pub const FILE: &str = "\u{E230}";
pub const SIDEBAR: &str = "\u{EC24}"; // sidebar-simple
pub const GEAR: &str = "\u{E270}";
pub const NOTE_PENCIL: &str = "\u{E34C}";
pub const SEARCH: &str = "\u{E30C}"; // magnifying-glass
pub const CLOUD_CHECK: &str = "\u{E1B0}";

/// Register the embedded Phosphor font. It gets its own family for deliberate
/// icon rendering, and is appended as a fallback to the text families so an icon
/// char inlined in a label still resolves.
pub fn register(fonts: &mut FontDefinitions) {
    fonts
        .font_data
        .insert(FAMILY.into(), FontData::from_static(include_bytes!("Phosphor.ttf")).into());
    fonts
        .families
        .insert(FontFamily::Name(FAMILY.into()), vec![FAMILY.into()]);
    for family in [FontFamily::Proportional, FontFamily::Monospace] {
        if let Some(keys) = fonts.families.get_mut(&family) {
            keys.push(FAMILY.into());
        }
    }
}

/// A `FontId` for rendering a Phosphor glyph at `size`.
pub fn font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(FAMILY.into()))
}
