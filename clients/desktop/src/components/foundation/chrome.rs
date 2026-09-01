//! Shared chrome metrics + keyboard badges.
//!
//! ## Height ladder
//! | Name | ~pts | Role |
//! |------|------|------|
//! | [`control_height`] | ~28 | buttons, field, picker, menu rows |
//! | segmented | ≈ control | exclusive strip (see `segmented`) |
//! | form row | control | labeled settings rows (group pad is Spacers) |
//! | toggle | ~22 | switch thumb track (intentionally smaller) |

use std::sync::Arc;

use egui::{
    Color32, CornerRadius, FontFamily, FontId, Frame, Margin, Rect, Response, Shadow, Stroke,
    StrokeKind, Ui,
};

use super::color::Theme;
use super::space::Space;
use super::space::control as control_space;
use super::typography::TypeRole;

/// Control transition duration (also `Style::animation_time`).
pub const HOVER_ANIM_SECS: f32 = 0.20;
/// Toggle thumb travel — snappier than general hover.
pub const TOGGLE_ANIM_SECS: f32 = 0.14;
/// After leaving a tip host, stay “hot” this long so the next host can chain
/// without a second dwell (see [`super::tip`]).
pub const TIP_CHAIN_GRACE_SECS: f32 = 0.40;

/// Corner radius steps.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Radius {
    /// 4 pt — swatches, tight chips, dense chrome.
    Sm,
    /// 8 pt — buttons, fields, rows.
    Control,
    /// 12 pt — sheets, large panels.
    Surface,
}

impl Radius {
    pub const fn pts(self) -> u8 {
        match self {
            Radius::Sm => 4,
            Radius::Control => 8,
            Radius::Surface => 12,
        }
    }

    pub fn corner(self) -> egui::CornerRadius {
        egui::CornerRadius::same(self.pts())
    }
}

/// Resting outline width (wireframe buttons, fields, frames).
pub const STROKE_HAIRLINE: f32 = 1.0;

/// Inset a fill rect so a [`StrokeKind::Outside`] hairline of [`STROKE_HAIRLINE`]
/// is fully visible inside `clip` (panel edge, window top, sibling panels).
///
/// **Outside stroke is free only when there is free space outside `fill`.** At a
/// hard edge (screen top, SidePanel join, parent clip), the fill must be pulled
/// in by one hairline or the stroke is clipped / covered by the neighbor.
/// Open edges (e.g. active tab bottom → workspace) can skip that side by
/// expanding `clip` past the intentional bleed.
///
/// Prefer this over ad-hoc ±1 hacks when painting edge chrome (tabs, flush plates).
pub fn fit_outside_stroke_fill(fill: egui::Rect, clip: egui::Rect) -> egui::Rect {
    let m = STROKE_HAIRLINE;
    let mut min = fill.min;
    let mut max = fill.max;
    // If Outside would land left of clip, pull fill right.
    if fill.left() - m < clip.left() {
        min.x = clip.left() + m;
    }
    if fill.right() + m > clip.right() {
        max.x = clip.right() - m;
    }
    if fill.top() - m < clip.top() {
        min.y = clip.top() + m;
    }
    if fill.bottom() + m > clip.bottom() {
        max.y = clip.bottom() - m;
    }
    if max.x < min.x + 1.0 {
        max.x = min.x + 1.0;
    }
    if max.y < min.y + 1.0 {
        max.y = min.y + 1.0;
    }
    egui::Rect::from_min_max(min, max)
}

/// Body line box for fields / control chrome.
pub fn control_line_height() -> f32 {
    TypeRole::Body.line_height()
}

/// Control height: vertical pad + line box + vertical pad.
pub fn control_height() -> f32 {
    control_space::PAD_Y.pts() * 2.0 + control_line_height()
}

/// Uniform inset for row hover/select washes (all four sides).
///
/// File rows, menu rows, nav: **1 px** air so adjacent washes read as separate
/// plates. Overlay scrollbars do not change content width (floating); when the
/// thumb is visible it may cover the right edge of the wash — that is paint
/// order, not layout.
pub fn row_wash_inset() -> f32 {
    1.0
}

/// Soft float under menus, pickers, floating plates.
pub fn overlay_shadow() -> Shadow {
    Shadow { offset: [0, 4], blur: 12, spread: 0, color: Color32::from_black_alpha(36) }
}

/// Canvas plate frame for floating menus / pickers (fill + hairline + shadow).
///
/// Uses real `inner_margin` so kids are not flush to the edge; Frame’s hardcoded
/// Inside stroke is acceptable here. For **flush** plates prefer
/// [`paint_plate`] / [`plate_content`] (Outside stroke).
pub fn canvas_overlay_frame(t: &Theme, inner_pad: Space) -> Frame {
    let p = inner_pad.pts() as i8;
    Frame::new()
        .fill(t.neutral_bg())
        .stroke(Stroke::new(STROKE_HAIRLINE, t.neutral()))
        .corner_radius(Radius::Control.corner())
        .inner_margin(Margin::same(p))
        .shadow(overlay_shadow())
}

/// Fill + hairline on a known rect. **`StrokeKind::Outside`** so later child
/// fills flush to `rect` cannot cover the border.
///
/// egui [`Frame`] hardcodes Inside stroke — do not use Frame for flush plates.
pub fn paint_plate(
    ui: &Ui, rect: Rect, radius: impl Into<CornerRadius>, fill: Color32, hairline: Color32,
) {
    ui.painter().rect(
        rect,
        radius,
        fill,
        Stroke::new(STROKE_HAIRLINE, hairline),
        StrokeKind::Outside,
    );
}

/// Outside hairline only (transparent fill — e.g. list/tree chrome over parent).
pub fn paint_plate_stroke(ui: &Ui, rect: Rect, radius: impl Into<CornerRadius>, hairline: Color32) {
    ui.painter().rect_stroke(
        rect,
        radius,
        Stroke::new(STROKE_HAIRLINE, hairline),
        StrokeKind::Outside,
    );
}

/// Content-sized plate: Frame for **fill + layout only**, Outside hairline after.
///
/// Prefer this over `Frame::stroke` whenever children may paint flush to the edge.
pub fn plate_content(
    ui: &mut Ui, fill: Color32, hairline: Color32, radius: impl Into<CornerRadius>,
    add: impl FnOnce(&mut Ui),
) -> Response {
    let radius = radius.into();
    let out = Frame::new()
        .fill(fill)
        .corner_radius(radius)
        .inner_margin(0.0)
        .show(ui, add);
    paint_plate_stroke(ui, out.response.rect, radius, hairline);
    out.response
}

/// One chip in a shortcut badge.
#[derive(Clone, Copy, Debug)]
pub enum KbdPart {
    /// Phosphor PUA glyph (command, key-return, …) — body size, mid-aligned.
    Icon(&'static str),
    /// Body-size mono key letter (`N`, `I`, `Ctrl+`) — matches icon scale.
    Mono(&'static str),
    /// Small mono caption (`esc` only) — sits with the button label baseline.
    MonoSm(&'static str),
}

/// Ordered shortcut badge (icons and/or mono text).
#[derive(Clone, Copy, Debug)]
pub struct Shortcut {
    pub parts: &'static [KbdPart],
}

/// Phosphor codepoints (https://phosphoricons.com). Regular variant.
/// Full set is the design surface — unused variants are expected.
#[allow(dead_code)]
pub mod phosphor {
    pub const COMMAND: &str = "\u{e1c4}";
    /// Return corner (`ph-arrow-elbow-down-left`).
    pub const KEY_RETURN: &str = "\u{e044}";
    /// Create-sheet “Alongside …” (`ph-arrow-bend-down-right`).
    pub const ARROW_BEND_DOWN_RIGHT: &str = "\u{e01a}";
    pub const FOLDER: &str = "\u{e24a}";
    /// Open folder.
    pub const FOLDER_OPEN: &str = "\u{e256}";
    pub const FILE: &str = "\u{e230}";
    /// Document type glyphs.
    pub const FILE_TEXT: &str = "\u{e23a}";
    pub const FILE_PDF: &str = "\u{e702}";
    pub const CODE: &str = "\u{e1bc}";
    pub const IMAGE_SQUARE: &str = "\u{e2cc}";
    pub const PAINT_BRUSH: &str = "\u{e6f0}";
    /// SVG / drawing docs.
    pub const PEN_NIB: &str = "\u{e3ac}";
    /// Settings appearance (theme / colors).
    pub const PALETTE: &str = "\u{e6c8}";

    pub const MARKDOWN_LOGO: &str = "\u{e508}";
    pub const CHAT: &str = "\u{e15c}";
    pub const SEARCH: &str = "\u{e30c}";
    pub const GEAR: &str = "\u{e270}";
    pub const TRASH: &str = "\u{e4a6}";
    /// Phosphor pencil (drawing docs).
    pub const PENCIL: &str = "\u{e3ae}";
    /// Phosphor `pencil-circle` — Can-edit access on share roster.
    pub const PENCIL_CIRCLE: &str = "\u{e3b0}";
    /// Can-view access (share roster). Distinct from slash-eye.
    pub const EYE: &str = "\u{e220}";
    /// Sidebar view toggles (Files / Recents / Shared).
    pub const CLOCK: &str = "\u{e19a}";
    pub const USERS: &str = "\u{e4d6}";
    /// Settings rail categories.
    pub const USER: &str = "\u{e4c2}";
    pub const USER_PLUS: &str = "\u{e4d0}";
    /// Found username (share field leading) — no `user-x` in the set for the fail case.
    pub const USER_CHECK: &str = "\u{eafa}";
    pub const CLOUD_ARROW_UP: &str = "\u{e1ae}";
    pub const WARNING_CIRCLE: &str = "\u{e4e2}";
    pub const CHECK_CIRCLE: &str = "\u{e184}";
    /// Bare check (confirm checkbox fill).
    pub const CHECK: &str = "\u{e182}";
    /// Not-found / clear fail (share field) — improvise; phosphor has no `user-x`.
    pub const X_CIRCLE: &str = "\u{e4f8}";
    pub const SPINNER_GAP: &str = "\u{e66c}";

    // Context menu / list actions.
    /// Open document.
    pub const ARROW_SQUARE_OUT: &str = "\u{e5de}";
    /// Open in new tab.
    pub const APP_WINDOW: &str = "\u{e5da}";
    /// Multiple browser-style tabs (strip / “close all”).
    pub const TABS: &str = "\u{e778}";
    /// Copy share link.
    pub const LINK: &str = "\u{e2e2}";
    pub const FILE_PLUS: &str = "\u{e236}";
    /// New note.
    pub const NOTE_PENCIL: &str = "\u{e34c}";
    pub const FOLDER_PLUS: &str = "\u{e258}";
    /// Phosphor 2.1 IcoMoon PUA (`folder-minus` / `folder-notch-minus`).
    pub const FOLDER_MINUS: &str = "\u{e254}";
    pub const CARET_DOWN: &str = "\u{e136}";
    pub const CARET_LEFT: &str = "\u{e138}";
    pub const CARET_RIGHT: &str = "\u{e13a}";
    /// Titleband back / forward.
    pub const ARROW_LEFT: &str = "\u{e058}";
    pub const ARROW_RIGHT: &str = "\u{e06c}";
    pub const FOLDERS: &str = "\u{e260}";
    pub const PUSH_PIN: &str = "\u{e3e2}";
    pub const SCISSORS: &str = "\u{eae0}";
    pub const COPY: &str = "\u{e1ca}";
    /// Paste (`ph-clipboard-text`).
    pub const CLIPBOARD_TEXT: &str = "\u{e198}";
    /// Select all (`ph-selection-all`).
    pub const SELECTION_ALL: &str = "\u{e746}";
    /// Sync / refresh (sidebar footer).
    pub const ARROWS_CLOCKWISE: &str = "\u{e094}";
    /// Zen / hide sidebar.
    pub const SIDEBAR_SIMPLE: &str = "\u{e9d0}";
    /// Dismiss / close sheet.
    pub const X: &str = "\u{e4f6}";
    /// Help / shortcuts.
    pub const QUESTION: &str = "\u{e3e8}";
    /// Import / download.
    pub const DOWNLOAD_SIMPLE: &str = "\u{e20c}";
    /// Window chrome (borderless title bar).
    pub const MINUS: &str = "\u{e32a}";
    pub const SQUARE: &str = "\u{e45e}";
    /// Linux maximize / restore (two diagonal arrows).
    pub const ARROWS_OUT_SIMPLE: &str = "\u{e0a6}";
    pub const ARROWS_IN_SIMPLE: &str = "\u{e09e}";
    /// Mind map tab (connected nodes).
    pub const GRAPH: &str = "\u{eb58}";
    /// Space inspector tab (share of disk).
    pub const CHART_PIE_SLICE: &str = "\u{e15a}";
}

/// Phosphor glyph for a workspace [`DocType`].
pub fn phosphor_for_doc_type(dt: workspace_rs::show::DocType) -> &'static str {
    use workspace_rs::show::DocType;
    match dt {
        DocType::Markdown => phosphor::MARKDOWN_LOGO,
        DocType::PlainText => phosphor::FILE_TEXT,
        DocType::SVG => phosphor::PENCIL,
        DocType::Image | DocType::ImageUnsupported => phosphor::IMAGE_SQUARE,
        DocType::Code => phosphor::CODE,
        DocType::PDF => phosphor::FILE_PDF,
        DocType::Chat => phosphor::CHAT,
        DocType::Unknown => phosphor::FILE,
    }
}

/// Row leading icon: folder, or doc-type from the file name extension.
pub fn file_row_icon(name: &str, is_folder: bool) -> &'static str {
    if is_folder {
        phosphor::FOLDER
    } else {
        phosphor_for_doc_type(workspace_rs::show::DocType::from_name(name))
    }
}

/// Visible file name in chrome: strip the extension when the doc type hides it
/// (Markdown, drawing, PDF, chat). Do not pass paths.
pub fn display_file_name(name: &str) -> &str {
    workspace_rs::show::DocType::from_name(name).display_name(name)
}

/// Tab-strip glyph for a workspace [`Destination`].
///
/// Search / mind map / space inspector are not files — do not go through
/// [`file_row_icon`]. Files still use the name’s [`DocType`].
pub fn tab_icon(dest: &workspace_rs::tab::Destination, name: &str) -> &'static str {
    use workspace_rs::tab::Destination;
    match dest {
        Destination::Search => phosphor::SEARCH,
        Destination::MindMap(_) => phosphor::GRAPH,
        Destination::SpaceInspector(_) => phosphor::CHART_PIE_SLICE,
        Destination::File(_) => file_row_icon(name, false),
    }
}

/// Font family name registered by `workspace_rs::register_fonts`.
const PHOSPHOR_FAMILY: &str = "phosphor";

/// Phosphor at `size` pt.
pub fn phosphor_font_id(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(Arc::from(PHOSPHOR_FAMILY)))
}

/// Phosphor at body size (leading button icons / file rows).
pub fn phosphor_ui_font_id() -> FontId {
    phosphor_font_id(TypeRole::Body.size())
}

/// Commit shortcut badge (⌘⏎ on macOS, Ctrl+⏎ elsewhere).
pub fn shortcut_return() -> Shortcut {
    if cfg!(target_os = "macos") {
        Shortcut { parts: &[KbdPart::Icon(phosphor::COMMAND), KbdPart::Icon(phosphor::KEY_RETURN)] }
    } else {
        Shortcut { parts: &[KbdPart::Mono("Ctrl+"), KbdPart::Icon(phosphor::KEY_RETURN)] }
    }
}

/// Plain ⏎ — create sheets; host often also accepts ⌘⏎ via `consume_key`.
pub fn shortcut_enter() -> Shortcut {
    Shortcut { parts: &[KbdPart::Icon(phosphor::KEY_RETURN)] }
}

/// Dismiss shortcut badge — small `esc` (only mono caption in this slot).
pub fn shortcut_esc() -> Shortcut {
    Shortcut { parts: &[KbdPart::MonoSm("esc")] }
}

/// ⌘N / Ctrl+N — onboard Create account (and product Create when signed in).
pub fn shortcut_cmd_n() -> Shortcut {
    if cfg!(target_os = "macos") {
        Shortcut { parts: &[KbdPart::Icon(phosphor::COMMAND), KbdPart::Mono("N")] }
    } else {
        Shortcut { parts: &[KbdPart::Mono("Ctrl+"), KbdPart::Mono("N")] }
    }
}

/// ⌘I / Ctrl+I — onboard Import account.
pub fn shortcut_cmd_i() -> Shortcut {
    if cfg!(target_os = "macos") {
        Shortcut { parts: &[KbdPart::Icon(phosphor::COMMAND), KbdPart::Mono("I")] }
    } else {
        Shortcut { parts: &[KbdPart::Mono("Ctrl+"), KbdPart::Mono("I")] }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_outside_stroke_pulls_fill_off_clip_edges() {
        use egui::{Rect, pos2};
        // Fill flush to clip left/top — Outside would escape.
        let clip = Rect::from_min_max(pos2(100.0, 0.0), pos2(400.0, 32.0));
        let fill = Rect::from_min_max(pos2(100.0, 0.0), pos2(200.0, 32.0));
        let fitted = fit_outside_stroke_fill(fill, clip);
        assert!(
            (fitted.left() - (clip.left() + STROKE_HAIRLINE)).abs() < 0.01,
            "left must inset by hairline: {}",
            fitted.left()
        );
        assert!(
            (fitted.top() - (clip.top() + STROKE_HAIRLINE)).abs() < 0.01,
            "top must inset by hairline: {}",
            fitted.top()
        );
        // Right/bottom had room inside clip for Outside — unchanged.
        assert!((fitted.right() - fill.right()).abs() < 0.01);
    }
}
