//! The type system: three roles, two faces. Sans → `heading` + `text`; mono →
//! `mono`. No variants — color or weight is adjusted at the callsite (chain
//! `.color(..)`) when a specific spot needs it; a recurring adjustment graduates
//! into a named helper here rather than being guessed at each use.

use egui::{FontFamily, FontId, RichText};

use super::tokens::Tokens;

const HEADING: f32 = 18.0;
const TEXT: f32 = 14.0;
const MONO: f32 = 14.0;

/// SF Pro / Noto bold, registered by `workspace_rs::register_fonts`.
fn bold() -> FontFamily {
    FontFamily::Name("Bold".into())
}

/// The largest text in a contained surface — auth card, Settings, a dialog.
pub fn heading(t: &Tokens, s: impl Into<String>) -> RichText {
    RichText::new(s)
        .font(FontId::new(HEADING, bold()))
        .color(t.fg())
}

/// The body of everything — sidebar rows, buttons, prose, labels, values.
pub fn text(t: &Tokens, s: impl Into<String>) -> RichText {
    RichText::new(s)
        .font(FontId::proportional(TEXT))
        .color(t.fg())
}

/// Data and code — the account key, hex, paths, keyboard shortcuts.
pub fn mono(t: &Tokens, s: impl Into<String>) -> RichText {
    RichText::new(s).font(FontId::monospace(MONO)).color(t.fg())
}
