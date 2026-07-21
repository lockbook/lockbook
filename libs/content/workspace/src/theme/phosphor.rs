//! Phosphor UI icons — shared by the markdown toolbar, context menu, and any
//! workspace chrome that needs the same glyphs as the desktop shell.
//!
//! Font bytes: `lb_fonts::PHOSPHOR`, registered as family `"phosphor"` in
//! [`crate::tab::markdown_editor::register_fonts`] (desktop + mobile).

use std::sync::Arc;

use egui::{FontFamily, FontId};

/// Family name installed by `register_fonts`.
pub const FAMILY: &str = "phosphor";

pub fn font_id(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(Arc::from(FAMILY)))
}

// ── Markdown / toolbar (matches editor context menu) ─────────────────────────

pub const TEXT_B: &str = "\u{E5BE}";
pub const TEXT_ITALIC: &str = "\u{E5C0}";
pub const TEXT_UNDERLINE: &str = "\u{E5C4}";
pub const TEXT_STRIKETHROUGH: &str = "\u{E5C2}";
pub const CODE: &str = "\u{E1BC}";
pub const HIGHLIGHTER_CIRCLE: &str = "\u{E632}";
pub const EYE_SLASH: &str = "\u{E224}";
pub const TEXT_SUBSCRIPT: &str = "\u{EC98}";
pub const TEXT_SUPERSCRIPT: &str = "\u{EC9A}";

pub const TEXT_AA: &str = "\u{E6EE}";
pub const PARAGRAPH: &str = "\u{E960}";
pub const TEXT_H: &str = "\u{E6BA}";
pub const TEXT_H_ONE: &str = "\u{E6BC}";
pub const TEXT_H_TWO: &str = "\u{E6BE}";
pub const TEXT_H_THREE: &str = "\u{E6C0}";
pub const TEXT_H_FOUR: &str = "\u{E6C2}";
pub const TEXT_H_FIVE: &str = "\u{E6C4}";
pub const TEXT_H_SIX: &str = "\u{E6C6}";
pub const QUOTES: &str = "\u{E660}";
pub const CODE_BLOCK: &str = "\u{EAFE}";

pub const LIST_BULLETS: &str = "\u{E2F2}";
pub const LIST_NUMBERS: &str = "\u{E2F6}";
pub const LIST_CHECKS: &str = "\u{EADC}";
pub const TEXT_INDENT: &str = "\u{EA1E}";
pub const TEXT_OUTDENT: &str = "\u{EA1C}";

pub const LINK: &str = "\u{E2E2}";
pub const ARROW_SQUARE_OUT: &str = "\u{E5DE}";
pub const APP_WINDOW: &str = "\u{E5DA}";
pub const ARROWS_CLOCKWISE: &str = "\u{E094}";
pub const PENCIL_SIMPLE: &str = "\u{E3B4}";
pub const IMAGE: &str = "\u{E2CA}";
pub const CAMERA: &str = "\u{E10E}";

pub const SCISSORS: &str = "\u{EAE0}";
pub const COPY: &str = "\u{E1CA}";
pub const CLIPBOARD: &str = "\u{E196}";
pub const SELECTION_ALL: &str = "\u{E746}";
pub const ARROW_U_UP_LEFT: &str = "\u{E08A}";
pub const ARROW_U_UP_RIGHT: &str = "\u{E08C}";
pub const MAGNIFYING_GLASS: &str = "\u{E30C}";

pub const CARET_UP: &str = "\u{E13C}";
pub const CARET_DOWN: &str = "\u{E136}";
pub const CARET_DOUBLE_UP: &str = "\u{E12C}";
pub const CARET_DOUBLE_DOWN: &str = "\u{E126}";
