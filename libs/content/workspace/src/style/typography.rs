//! Type scale: 22 title, 16 heading, 14 body, 12 mono.

use egui::{FontFamily, FontId, RichText, TextStyle};

/// Named type roles.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TypeRole {
    /// Document / page / article title — 22. Web-page energy, not UI chrome.
    Title,
    /// Layout region name — 17. Sheets, settings panels, section chrome.
    Heading,
    /// Prose, buttons, captions, labels — 14. De-emphasize with muted / weight.
    Body,
    /// Code, token names, kbd badges — 12 mono.
    Mono,
}

impl TypeRole {
    pub const ALL: [TypeRole; 4] =
        [TypeRole::Title, TypeRole::Heading, TypeRole::Body, TypeRole::Mono];

    /// Page / document title size.
    pub const TITLE_SIZE: f32 = 22.0;
    /// UI section / sheet heading size.
    pub const HEADING_SIZE: f32 = 16.0;
    /// Default UI size.
    pub const UI_SIZE: f32 = 14.0;
    /// Monospace size.
    pub const MONO_SIZE: f32 = 12.0;

    pub const fn size(self) -> f32 {
        match self {
            TypeRole::Title => Self::TITLE_SIZE,
            TypeRole::Heading => Self::HEADING_SIZE,
            TypeRole::Body => Self::UI_SIZE,
            TypeRole::Mono => Self::MONO_SIZE,
        }
    }

    /// Line box (em × 1.4) — matches Glyphon / control metrics.
    pub const fn line_height(self) -> f32 {
        self.size() * 1.4
    }

    pub const fn family(self) -> FontFamily {
        match self {
            TypeRole::Mono => FontFamily::Monospace,
            _ => FontFamily::Proportional,
        }
    }

    pub fn font_id(self) -> FontId {
        FontId::new(self.size(), self.family())
    }

    pub fn text_style(self) -> Option<TextStyle> {
        match self {
            // egui only has one Heading slot — map our document Title there.
            TypeRole::Title => Some(TextStyle::Heading),
            TypeRole::Heading => None, // use font_id / rich only
            TypeRole::Body => Some(TextStyle::Body),
            TypeRole::Mono => Some(TextStyle::Monospace),
        }
    }

    pub fn rich(self, text: impl Into<String>) -> RichText {
        RichText::new(text).font(self.font_id())
    }
}

/// Sizes applied by [`super::install`] into `Style::text_styles`.
pub fn install_text_styles(style: &mut egui::Style) {
    for role in TypeRole::ALL {
        if let Some(ts) = role.text_style() {
            style.text_styles.insert(ts, role.font_id());
        }
    }
    // No separate Small role — keep egui's Small style at body size for stock widgets.
    style
        .text_styles
        .insert(TextStyle::Small, TypeRole::Body.font_id());
    style
        .text_styles
        .insert(TextStyle::Button, TypeRole::Body.font_id());
}
