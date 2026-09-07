//! Shared chrome metrics + keyboard badges.
//!
//! ## Height ladder
//! | Name | ~pts | Role |
//! |------|------|------|
//! | [`control_height`] | ~28 | buttons, field, picker, menu rows |
//! | [`CHROME_BAND_H`] | 40 | desktop titleband, markdown toolbar |
//! | segmented | ≈ control | exclusive strip (see `segmented`) |
//! | form row | control | labeled settings rows (group pad is Spacers) |
//! | toggle | ~22 | switch thumb track (intentionally smaller) |

use std::sync::Arc;

use egui::{
    Align, Align2, Color32, CornerRadius, FontFamily, FontId, Frame, Layout, Margin, Rect,
    Response, Sense, Shadow, Stroke, StrokeKind, Ui, pos2, vec2,
};

use super::color::{Theme, ThemeExt};
use super::space::Space;
use super::space::control as control_space;
use super::typography::TypeRole;

/// Control transition duration (also `Style::animation_time`).
pub const HOVER_ANIM_SECS: f32 = 0.20;
/// Hard cap for chrome motion (sidebar, surface reveal, overlay fades).
/// Hover/toggle may be shorter; nothing should run longer.
pub const ANIM_MAX_SECS: f32 = 0.22;
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

/// Desktop titleband and markdown toolbar — same strip.
pub const CHROME_BAND_H: f32 = 40.0;
/// Phosphor on [`CHROME_BAND_H`] (titleband pane/nav, markdown toolbar).
pub const CHROME_BAND_GLYPH: f32 = 18.0;

/// Square hit for a Phosphor mark inside a control (field clear, chip dismiss).
///
/// Glyph is body-size Phosphor ([`phosphor_ui_font_id`] via
/// [`super::button::icon_button_hit`]); this is only the hover/click square —
/// the inner band after vertical pads, same height as a field’s leading icon slot.
pub fn control_icon_hit() -> f32 {
    control_line_height()
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

/// Compact floating toolbar — image / canvas viewport islands.
///
/// Raised capsule (secondary fill, no hairline, overlay shadow). Height matches
/// the titleband ([`CHROME_BAND_H`]). End-cap icon squares are concentric with
/// the stadium (`pad + hit/2 == height/2`); hover wash is a circle. Glyph is
/// [`CHROME_BAND_GLYPH`]. Not [`canvas_overlay_frame`] (menus / pickers).
pub mod island {
    use egui::{Frame, Margin, Stroke};

    use super::{CHROME_BAND_H, overlay_shadow};
    use crate::style::color::Theme;
    use crate::style::space::Space;

    /// Inset that puts the first/last icon center on the cap center.
    pub const PAD_X: f32 = Space::Xs.pts();
    pub const PAD_Y: f32 = Space::Xs.pts();
    /// Large enough to fully round [`height`].
    pub const RADIUS: u8 = 30;

    /// Square hit, fills the inner band so end washes share the cap center.
    pub fn icon_hit() -> f32 {
        CHROME_BAND_H - PAD_Y * 2.0
    }

    pub fn height() -> f32 {
        CHROME_BAND_H
    }

    /// Opaque fill under island controls — pass as `icon_button` `ground`.
    pub fn ground(t: &Theme) -> egui::Color32 {
        t.neutral_bg_secondary()
    }

    pub fn frame(t: &Theme) -> Frame {
        Frame::new()
            .fill(ground(t))
            .stroke(Stroke::NONE)
            .corner_radius(egui::CornerRadius::same(RADIUS))
            .inner_margin(Margin::symmetric(PAD_X as i8, PAD_Y as i8))
            .shadow(overlay_shadow())
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn end_icon_concentric_with_cap() {
            let cx = PAD_X + icon_hit() / 2.0;
            let cy = PAD_Y + icon_hit() / 2.0;
            let cap = height() / 2.0;
            assert!((cx - cap).abs() < 0.01);
            assert!((cy - cap).abs() < 0.01);
        }
    }
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
    /// Zoom out (`ph-magnifying-glass-minus`).
    pub const MAGNIFYING_GLASS_MINUS: &str = "\u{e30e}";
    /// Zoom in (`ph-magnifying-glass-plus`).
    pub const MAGNIFYING_GLASS_PLUS: &str = "\u{e310}";
    /// Filter / funnel.
    pub const FUNNEL: &str = "\u{e268}";
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
    /// Horizontal ellipsis (`ph-dots-three`).
    pub const DOTS_THREE: &str = "\u{e1fe}";
    /// GFM alert / status.
    pub const INFO: &str = "\u{e2ce}";
    pub const LIGHTBULB: &str = "\u{e2dc}";
    pub const MEGAPHONE: &str = "\u{e324}";
    pub const WARNING: &str = "\u{e4e0}";
    pub const WARNING_OCTAGON: &str = "\u{e4e4}";
    pub const IMAGE: &str = "\u{e2ca}";
    pub const IMAGE_BROKEN: &str = "\u{e7a8}";
    pub const FLOPPY_DISK: &str = "\u{e248}";
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
    pub const CARET_UP: &str = "\u{e13c}";
    /// Undo (`ph-arrow-counter-clockwise`).
    pub const ARROW_COUNTER_CLOCKWISE: &str = "\u{e038}";
    /// Redo (`ph-arrow-clockwise`).
    pub const ARROW_CLOCKWISE: &str = "\u{e036}";
    /// Markdown toolbar.
    pub const TEXT_B: &str = "\u{e5be}";
    pub const TEXT_ITALIC: &str = "\u{e5c0}";
    pub const TEXT_H_ONE: &str = "\u{e6bc}";
    pub const TEXT_STRIKETHROUGH: &str = "\u{e5c2}";
    pub const TEXT_UNDERLINE: &str = "\u{e5c4}";
    pub const TEXT_SUBSCRIPT: &str = "\u{ec98}";
    pub const TEXT_SUPERSCRIPT: &str = "\u{ec9a}";
    pub const TEXT_INDENT: &str = "\u{ea1e}";
    pub const TEXT_OUTDENT: &str = "\u{ea1c}";
    pub const LIST_BULLETS: &str = "\u{e2f2}";
    pub const LIST_NUMBERS: &str = "\u{e2f6}";
    pub const CHECK_SQUARE: &str = "\u{e186}";
    pub const HIGHLIGHTER: &str = "\u{ec76}";
    pub const EYE_SLASH: &str = "\u{e224}";
    pub const CAMERA: &str = "\u{e10e}";
    /// Find bar: match case / whole word / regex / replace.
    pub const TEXT_AA: &str = "\u{e6ee}";
    pub const TEXT_T: &str = "\u{e48a}";
    pub const FUNCTION: &str = "\u{ebe4}";
    pub const SWAP: &str = "\u{e83c}";
    pub const REPEAT: &str = "\u{e3f6}";
    /// Content-search “Show N matches” (`ph-arrows-vertical`).
    pub const ARROWS_VERTICAL: &str = "\u{eb04}";
    /// Fit width (`ph-arrows-horizontal`).
    pub const ARROWS_HORIZONTAL: &str = "\u{eb06}";
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
    /// Panel with a leading strip (`ph-sidebar-simple`).
    pub const SIDEBAR_SIMPLE: &str = "\u{ec24}";
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
    /// Canvas tools.
    pub const HAND: &str = "\u{e298}";
    /// Select tool (`ph-cursor`).
    pub const CURSOR: &str = "\u{e1dc}";
    pub const ERASER: &str = "\u{e21e}";
    pub const POLYGON: &str = "\u{e6d0}";
    pub const LOCK_SIMPLE: &str = "\u{e308}";
    pub const LOCK_SIMPLE_OPEN: &str = "\u{e30a}";
    /// Shape tools (`ph-rectangle` / `ph-circle` / `ph-line-segment`).
    pub const RECTANGLE: &str = "\u{e3f0}";
    pub const CIRCLE: &str = "\u{e18a}";
    pub const LINE_SEGMENT: &str = "\u{e6d2}";
    /// Layer order (`ph-arrow-line-up` / `ph-arrow-line-down`).
    pub const ARROW_LINE_UP: &str = "\u{e066}";
    pub const ARROW_LINE_DOWN: &str = "\u{e05c}";
    /// Mind map tab (connected nodes).
    pub const GRAPH: &str = "\u{eb58}";
    /// Space inspector tab (share of disk).
    pub const CHART_PIE_SLICE: &str = "\u{e15a}";
}

/// Phosphor glyph for a workspace [`DocType`].
pub fn phosphor_for_doc_type(dt: crate::show::DocType) -> &'static str {
    use crate::show::DocType;
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
        phosphor_for_doc_type(crate::show::DocType::from_name(name))
    }
}

/// Visible file name in chrome: strip the extension when the doc type hides it
/// (Markdown, drawing, PDF, chat). Do not pass paths.
pub fn display_file_name(name: &str) -> &str {
    crate::show::DocType::from_name(name).display_name(name)
}

/// Tab-strip glyph for a workspace [`Destination`].
///
/// Search / mind map / space inspector are not files — do not go through
/// [`file_row_icon`]. Files still use the name’s [`DocType`].
pub fn tab_icon(dest: &crate::tab::Destination, name: &str) -> &'static str {
    use crate::tab::Destination;
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

/// Centered Phosphor spinner + muted caption. Call every frame while waiting.
pub fn loading_indicator(ui: &mut Ui) {
    let t = ui.ctx().get_lb_theme();
    ui.ctx().request_repaint();
    let angle = (ui.input(|i| i.time) * std::f64::consts::TAU) as f32;
    let g = ui.painter().layout_no_wrap(
        phosphor::SPINNER_GAP.into(),
        phosphor_ui_font_id(),
        t.accent(),
    );
    let gap = Space::Xs.pts();
    let cap_h = TypeRole::Body.line_height();
    let content_h = g.size().y + gap + cap_h;
    let rect = ui.available_rect_before_wrap();
    let y = (rect.center().y - content_h / 2.0).max(rect.top());
    let block = Rect::from_min_size(
        pos2(rect.left(), y),
        vec2(rect.width(), content_h.min(rect.height()).max(1.0)),
    );
    super::layout::place_at(ui, block, Layout::top_down(Align::Center), |ui| {
        let (icon_rect, _) = ui.allocate_exact_size(g.size(), Sense::hover());
        ui.painter().add(
            egui::epaint::TextShape::new(icon_rect.min, g, t.accent())
                .with_angle_and_anchor(angle, Align2::CENTER_CENTER),
        );
        ui.add_space(gap);
        ui.label(
            TypeRole::Body
                .rich("Loading…")
                .color(t.neutral_fg_secondary()),
        );
    });
    let _ = ui.allocate_rect(rect, Sense::hover());
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

/// ⌘O / Ctrl+O — search / open quickly.
pub fn shortcut_cmd_o() -> Shortcut {
    if cfg!(target_os = "macos") {
        Shortcut { parts: &[KbdPart::Icon(phosphor::COMMAND), KbdPart::Mono("O")] }
    } else {
        Shortcut { parts: &[KbdPart::Mono("Ctrl+"), KbdPart::Mono("O")] }
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
