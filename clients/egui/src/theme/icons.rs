//! Phosphor icons — codepoints + helpers for the desktop shell.
//!
//! Font **bytes** live in `lb-fonts` and are registered by
//! `workspace_rs::register_fonts` (desktop, iOS, Android). This module only
//! names the family and ships the glyph constants used by egui chrome.
//!
//! Codepoints are from egui-phosphor 0.12 (regular variant).

use egui::{FontData, FontDefinitions, FontFamily, FontId};
use std::sync::Arc;

/// The family workspace / this module register under.
const FAMILY: &str = "phosphor";

pub const FOLDER: &str = "\u{E24A}";
pub const FOLDER_OPEN: &str = "\u{E256}";
pub const FILE: &str = "\u{E230}";
/// Doc-type icons — mapped from `DocType` (same extension rules as master tree).
pub const FILE_TEXT: &str = "\u{E23A}";
pub const FILE_PDF: &str = "\u{E702}";
pub const CODE: &str = "\u{E1BC}"; // code (not file-code)
pub const IMAGE_SQUARE: &str = "\u{E2CC}"; // image-square
pub const PAINT_BRUSH: &str = "\u{E6F0}"; // drawings / SVG
pub const MARKDOWN_LOGO: &str = "\u{E508}";
pub const CHAT: &str = "\u{E15C}";
#[allow(dead_code)] // kept for optional single-sidebar affordances
pub const SIDEBAR: &str = "\u{EC24}"; // sidebar-simple
pub const GEAR: &str = "\u{E270}";
pub const NOTE_PENCIL: &str = "\u{E34C}"; // square-and-pencil-ish / new note
pub const DOWNLOAD_SIMPLE: &str = "\u{E20C}"; // import (Apple: square.and.arrow.down)
pub const SEARCH: &str = "\u{E30C}"; // magnifying-glass
pub const ARROWS_CLOCKWISE: &str = "\u{E094}"; // sync
/// Sidebar view toggles (Apple: folder / clock / person.2; Zed-style always on).
pub const CLOCK: &str = "\u{E19A}"; // clock (recents)
pub const USERS: &str = "\u{E4D6}"; // users (shared with me)
pub const USER: &str = "\u{E4C2}"; // person (collaborator chip)
pub const USER_PLUS: &str = "\u{E4D0}"; // add person
pub const CHECK_CIRCLE: &str = "\u{E184}"; // lookup success
pub const X_CIRCLE: &str = "\u{E4F8}"; // lookup missing user
pub const WARNING_CIRCLE: &str = "\u{E4E2}"; // offline / caution
pub const SPINNER_GAP: &str = "\u{E66C}"; // lookup in progress
/// Quiet pin mark on tree/recents rows (not a filled orange badge).
pub const PUSH_PIN: &str = "\u{E3E2}";
/// Context-menu unpin / pin.slash.
pub const PUSH_PIN_SLASH: &str = "\u{E3E4}";
/// View-only for you (shared with Read, including via ancestors).
pub const PENCIL_SIMPLE_SLASH: &str = "\u{ECF6}";
/// Local changes not yet on the server (Finder-style status, not idle check).
pub const CLOUD_ARROW_UP: &str = "\u{E1AE}";
/// Pulling metadata/content from the server.
pub const CLOUD_ARROW_DOWN: &str = "\u{E1AC}";
/// Add a pending share into your files tree (organize).
pub const FOLDER_PLUS: &str = "\u{E258}";
/// Remove an organized share link from your files (returns to Shared with me).
pub const FOLDER_MINUS: &str = "\u{E254}";
// Context-menu action icons (Phosphor regular).
pub const ARROW_SQUARE_OUT: &str = "\u{E5DE}"; // open
pub const APP_WINDOW: &str = "\u{E5DA}"; // open in new tab
pub const FILE_PLUS: &str = "\u{E236}"; // new document
pub const PENCIL_SIMPLE: &str = "\u{E3B4}"; // rename
pub const TRASH: &str = "\u{E4A6}"; // delete own files
// Decline pending share: reuse X_CIRCLE (lookup / forsake).
pub const SHARE_NETWORK: &str = "\u{E408}"; // share with person
pub const LINK: &str = "\u{E2E2}"; // copy share link
pub const EXPORT: &str = "\u{EAF0}"; // export / share externally
pub const COPY: &str = "\u{E1CA}"; // copy (to clipboard)
pub const SCISSORS: &str = "\u{EAE0}"; // cut
pub const CLIPBOARD: &str = "\u{E196}"; // paste
pub const FILES: &str = "\u{E710}"; // duplicate (sibling copy now)
pub const FOLDERS: &str = "\u{E260}"; // move to folder
pub const CARET_DOWN: &str = "\u{E136}"; // expand folder / subtree
pub const CARET_RIGHT: &str = "\u{E13A}"; // collapse folder / subtree
pub const CARET_DOUBLE_DOWN: &str = "\u{E126}"; // expand all
pub const CARET_DOUBLE_UP: &str = "\u{E12C}"; // collapse all

/// Phosphor glyph for a document type — extension selection lives in
/// `DocType::from_name` (workspace); this only maps type → icon, parallel to
/// master's `DocType::to_icon` (Material) for the Phosphor set.
pub fn for_doc_type(dt: workspace_rs::show::DocType) -> &'static str {
    use workspace_rs::show::DocType;
    match dt {
        DocType::Markdown => MARKDOWN_LOGO,
        DocType::PlainText => FILE_TEXT,
        DocType::SVG => PAINT_BRUSH,
        DocType::Image | DocType::ImageUnsupported => IMAGE_SQUARE,
        DocType::Code => CODE,
        DocType::PDF => FILE_PDF,
        DocType::Chat => CHAT,
        DocType::Unknown => FILE,
    }
}
/// Plain × — modal dismiss / window close (not the circled error mark).
pub const X: &str = "\u{E4F6}";
// Window controls — Windows/Linux only (macOS keeps native chrome).
#[cfg(not(target_os = "macos"))]
pub const MINUS: &str = "\u{E32A}"; // minimize
#[cfg(not(target_os = "macos"))]
pub const SQUARE: &str = "\u{E45E}"; // maximize

/// Ensure the `phosphor` family is present. Prefer
/// [`workspace_rs::register_fonts`] (loads bytes from `lb-fonts`); this is a
/// fallback when shell fonts are registered without the workspace path.
pub fn register(fonts: &mut FontDefinitions) {
    if fonts.font_data.contains_key(FAMILY) {
        // Already installed by workspace_rs::register_fonts.
        fonts
            .families
            .entry(FontFamily::Name(FAMILY.into()))
            .or_insert_with(|| vec![FAMILY.into()]);
        return;
    }
    // Fallback: vendored copy next to this module (legacy / shell-only demos).
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
    FontId::new(size, FontFamily::Name(Arc::from(FAMILY)))
}
