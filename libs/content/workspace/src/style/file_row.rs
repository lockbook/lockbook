//! Selectable file list row — shared paint for tree, Recents, Shared, sheets.
//!
//! ## Layout
//! Uses [`tree_metrics`]: uniform [`ROW_H`], [`INDENT_BASE`] + depth ×
//! [`INDENT_STEP`], [`ICON_SLOT`] for the type glyph. Virtualized tree paints
//! into a fixed rect via [`FileRow::paint_at_hit`].
//!
//! Optional **subtitle** (path crumbs) lives **inside** the same interact rect
//! so hover/selection wash covers name + path as one item. Paths use
//! middle-ellipsis when they don't fit (`a / … / leaf`).
//!
//! ## Looks
//! Canvas (or elevated sticky) ground; idle transparent; hover/selection ink
//! wash. Label body `fg` (muted / hanging continuations use secondary). Folder
//! icons accent. Optional sync + pin.

use egui::{Color32, Id, Rect, Response, Sense, Ui, pos2, vec2};

use super::file_name;
use crate::style::chrome::{
    HOVER_ANIM_SECS, Radius, display_file_name, phosphor, phosphor_ui_font_id, row_wash_inset,
};
use crate::style::color::{FG_HOVER, FG_PRESS, Theme};
use crate::style::interact::sense_click;
use crate::style::space::Space;
use crate::style::tree_metrics::{ICON_SLOT, INDENT_BASE, INDENT_STEP, ROW_H};
use crate::style::typography::TypeRole;
use crate::widgets::glyphon_label::{GlyphonLabel, TextOverflow};

/// Secondary path line under the name (Recents crumbs).
const SUB_SIZE: f32 = 12.0;
const SUB_LINE_H: f32 = SUB_SIZE * 1.4;
/// Air between name and subtitle inside a two-line row.
const SUB_GAP: f32 = 1.0;
/// Extra icon→text gap when a path line is present — the two-line stack
/// needs more horizontal air than a single tree name (tree keeps [`ICON_SLOT`]).
const SUB_ICON_EXTRA: f32 = Space::Sm.pts();

/// Full-width selectable file row.
pub struct FileRow<'a> {
    tokens: &'a Theme,
    label: String,
    /// Optional second line (path crumbs). Joined into the same hover/select plate.
    subtitle: Option<String>,
    /// Optional second line that is **not** a path (search snippets). Bold spans
    /// mark the query; end-ellipsis when it doesn't fit. Takes precedence over
    /// [`Self::subtitle`].
    caption: Option<Vec<(String, bool)>>,
    selected: bool,
    icon: &'static str,
    depth: usize,
    pinned: bool,
    /// Trailing people mark (Files tree: this file has share metadata).
    shared: bool,
    /// Optional sync status dot (caller debounces).
    sync_dot: Option<Color32>,
    elevated: bool,
    /// When elevated and > 0: round NW/NE of the wash to match a sheet plate pin.
    elevated_top_radius: u8,
    /// Horizontal inset for icons / name / trail only (not wash / elevated plate).
    /// Sticky sidebar pins: full-bleed plate + wash; content still uses list pad.
    content_inset: f32,
    /// Extra right reserve inside the content band (e.g. nested Save control).
    /// Name/trail chrome stop before this; wash still full-bleed.
    trail_reserve: f32,
    sense: Sense,
    /// When false: no hover/press wash (static display — e.g. non-folder delete rows).
    interactive: bool,
    /// Hang under a two-line file's caption column: no type icon, caption-size
    /// type. Content-search extra hits / “Show N more matches”.
    continuation: bool,
    /// Label (and continuation mark) in secondary ink.
    muted: bool,
    /// Keyboard/cursor plate at hover wash (not press). Files-tree hover.
    highlighted: bool,
    /// Center the hanging mark + label (content-search “Show more”).
    centered: bool,
}

impl<'a> FileRow<'a> {
    pub fn new(tokens: &'a Theme, label: impl Into<String>) -> Self {
        Self {
            tokens,
            label: label.into(),
            subtitle: None,
            caption: None,
            selected: false,
            icon: phosphor::FILE,
            depth: 0,
            pinned: false,
            shared: false,
            sync_dot: None,
            elevated: false,
            elevated_top_radius: 0,
            content_inset: 0.0,
            trail_reserve: 0.0,
            sense: sense_click(),
            interactive: true,
            continuation: false,
            muted: false,
            highlighted: false,
            centered: false,
        }
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// Hover-strength plate without a pointer (search keyboard cursor).
    /// Same wash as Files-tree hover; [`Self::selected`] is press.
    pub fn highlighted(mut self, on: bool) -> Self {
        self.highlighted = on;
        self
    }

    /// Leading Phosphor glyph (default [`phosphor::FILE`]).
    pub fn icon(mut self, glyph: &'static str) -> Self {
        self.icon = glyph;
        self
    }

    /// Second line under the name (e.g. parent path). Empty strings are ignored.
    pub fn subtitle(mut self, s: impl Into<String>) -> Self {
        let s = s.into();
        self.subtitle = if s.is_empty() { None } else { Some(s) };
        self
    }

    /// Rich second line (search snippets). `(text, bold)` spans; empty ignored.
    pub fn caption_spans(mut self, spans: Vec<(String, bool)>) -> Self {
        self.caption = if spans.is_empty() { None } else { Some(spans) };
        self
    }

    /// Tree indent depth (0 = flush list like Recents).
    pub fn depth(mut self, depth: usize) -> Self {
        self.depth = depth;
        self
    }

    pub fn pinned(mut self, on: bool) -> Self {
        self.pinned = on;
        self
    }

    /// Trailing muted people glyph — file participates in a share (Files tree).
    pub fn shared(mut self, on: bool) -> Self {
        self.shared = on;
        self
    }

    pub fn sync_dot(mut self, c: Option<Color32>) -> Self {
        self.sync_dot = c;
        self
    }

    /// Sticky elevated plate (surface ground under the wash).
    pub fn elevated(mut self, on: bool) -> Self {
        self.elevated = on;
        self
    }

    /// Top-only corner radius on elevated wash (sheet plate pin NW/NE).
    pub fn elevated_top_radius(mut self, r: u8) -> Self {
        self.elevated_top_radius = r;
        self
    }

    /// Inset for leading/trailing chrome (icons, name, pin). Wash still uses full
    /// `paint_rect` (sticky plates stay edge-to-edge).
    pub fn content_inset(mut self, pts: f32) -> Self {
        self.content_inset = pts.max(0.0);
        self
    }

    /// Reserve the right of the content band for a nested control (Shared Save).
    pub fn trail_reserve(mut self, pts: f32) -> Self {
        self.trail_reserve = pts.max(0.0);
        self
    }

    /// Interact sense (tree uses click+drag for DnD).
    pub fn sense(mut self, sense: Sense) -> Self {
        self.sense = sense;
        self
    }

    /// Hover wash + click affordance. Off for static list rows (delete sheet
    /// files — only folders expand). Selected fill still paints when `selected`.
    pub fn interactive(mut self, on: bool) -> Self {
        self.interactive = on;
        if !on {
            self.sense = Sense::hover();
        }
        self
    }

    /// Hang under a parent file's name/caption column (no type icon).
    pub fn continuation(mut self, on: bool) -> Self {
        self.continuation = on;
        self
    }

    /// Secondary ink for the label / continuation mark.
    pub fn muted(mut self, on: bool) -> Self {
        self.muted = on;
        self
    }

    /// Center hanging mark + label in the row (not under the parent caption).
    pub fn centered(mut self, on: bool) -> Self {
        self.centered = on;
        self
    }

    /// Pitch for virtualized lists (sticky tree, Shared) without building a row.
    pub const fn height_for(has_subtitle: bool) -> f32 {
        if has_subtitle { ROW_H + SUB_GAP + SUB_LINE_H } else { ROW_H }
    }

    /// Pitch for hanging continuation rows (caption size + pad).
    pub const fn continuation_height() -> f32 {
        SUB_LINE_H + Space::Xs.pts() * 2.0
    }

    /// Allocate a full-width row and paint it (landing, non-virtualized lists).
    pub fn show(self, ui: &mut Ui, id: Id) -> Response {
        let h = if self.continuation {
            Self::continuation_height()
        } else {
            Self::height_for(self.subtitle.is_some() || self.caption.is_some())
        };
        let w = ui.available_width().max(1.0);
        let (rect, _) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
        self.paint_at_hit(ui, rect, rect, id)
    }

    /// Paint into `paint_rect`; **hover / click** use `hit_rect`. Tree uses this
    /// so content can scroll under sticky pins (full paint) without sharing
    /// hover with the pin (hit clipped below pins).
    pub fn paint_at_hit(self, ui: &mut Ui, paint_rect: Rect, hit_rect: Rect, id: Id) -> Response {
        let t = self.tokens;
        let hit = hit_rect.intersect(ui.clip_rect());
        let resp = if hit.height() < 0.5 {
            ui.interact(hit, id, Sense::hover())
        } else {
            ui.interact(hit, id, self.sense)
        };

        // Dense lists: pure geometry only — `hovered()` includes interact_radius
        // and would light a pin and the underlapping file at once (geometry-only hit testing).
        let over = self.interactive
            && (self.highlighted || ui.ctx().rect_contains_pointer(ui.layer_id(), hit));
        let hover_t = ui
            .ctx()
            .animate_bool_with_time(resp.id.with("hov"), over, HOVER_ANIM_SECS);

        let ground = if self.elevated { t.neutral_bg_secondary() } else { t.neutral_bg() };
        let (h_amt, p_amt) = (FG_HOVER, FG_PRESS);

        let fill = if self.selected {
            t.wash_toward_neutral_fg(ground, p_amt)
        } else if self.interactive && hover_t > 0.0 {
            let hover_c = t.wash_toward_neutral_fg(ground, h_amt);
            ground.lerp_to_gamma(hover_c, hover_t)
        } else {
            Color32::TRANSPARENT
        };
        if fill.a() > 0 {
            // Full-width row geometry; wash is 1 px inset on all sides so adjacent
            // hovers read as separate plates (sidebar + sheet trees alike).
            let wash = paint_rect.shrink(row_wash_inset());
            let corners = if self.elevated && self.elevated_top_radius > 0 {
                // Match sheet plate pin (top only); radius slightly under the
                // frame so the 1 px inset still sits inside the rounded chrome.
                let r = self.elevated_top_radius.saturating_sub(1);
                egui::CornerRadius { nw: r, ne: r, sw: 0, se: 0 }
            } else if self.elevated {
                egui::CornerRadius::ZERO
            } else {
                Radius::Control.corner()
            };
            ui.painter()
                .with_clip_rect(ui.clip_rect())
                .rect_filled(wash, corners, fill);
        }

        let hanging = self.continuation;
        let ink = if self.muted { t.neutral_fg_secondary() } else { t.neutral_fg() };
        let is_folder = self.icon == phosphor::FOLDER || self.icon == phosphor::FOLDER_OPEN;
        let icon_ink = if is_folder { t.accent() } else { ink };
        let painter = ui
            .painter()
            .with_clip_rect(ui.clip_rect().intersect(paint_rect));

        // Content band may be inset from paint_rect (sticky full-bleed plate).
        let content_left = paint_rect.left() + self.content_inset;
        let content_right = paint_rect.right() - self.content_inset - self.trail_reserve;

        let icon_x = content_left + INDENT_BASE + self.depth as f32 * INDENT_STEP;
        let icon_cy = paint_rect.center().y;
        let two_line = !hanging && (self.subtitle.is_some() || self.caption.is_some());
        // Two-line recents rows (and hanging continuations): wider icon column so
        // name/path don't crowd the glyph. Continuations share that column so
        // extra hits line up under the parent's caption.
        let icon_col = if two_line || hanging { ICON_SLOT + SUB_ICON_EXTRA } else { ICON_SLOT };
        let text_x = icon_x + icon_col;

        if !hanging {
            let ig = painter.layout_no_wrap(self.icon.into(), phosphor_ui_font_id(), icon_ink);
            painter.galley(pos2(icon_x, icon_cy - ig.size().y / 2.0), ig, icon_ink);
        }

        // Trailing marks from the right: end pad · pin? · share?
        let trail_slot = Space::Md.pts();
        let mut trail_w = Space::Xs.pts();
        if self.pinned {
            trail_w += trail_slot;
        }
        if self.shared {
            trail_w += trail_slot;
        }
        const SYNC_R: f32 = 4.0;
        const SYNC_GAP: f32 = 6.0;
        let sync_reserve = if self.sync_dot.is_some() { SYNC_GAP + 2.0 * SYNC_R } else { 0.0 };
        let max_w = (content_right - trail_w - sync_reserve - text_x).max(Space::Sm.pts());

        if hanging {
            let cy = paint_rect.center().y;
            let mark = if self.icon != phosphor::FILE {
                Some(painter.layout_no_wrap(
                    self.icon.into(),
                    phosphor_ui_font_id(),
                    t.neutral_fg_secondary(),
                ))
            } else {
                None
            };
            let mark_w = mark.as_ref().map(|g| g.size().x).unwrap_or(0.0);
            let mark_gap = if mark.is_some() { Space::Xs.pts() } else { 0.0 };
            let label_w = if self.caption.is_none() && !self.label.is_empty() {
                file_name::measure_sized(ui, &self.label, SUB_SIZE, SUB_LINE_H, f32::MAX)
            } else {
                0.0
            };
            let cluster = mark_w + mark_gap + label_w;
            let mut body_x = if self.centered {
                (paint_rect.center().x - cluster / 2.0)
                    .clamp(content_left, (content_right - cluster).max(content_left))
            } else {
                text_x
            };
            if let Some(ig) = mark {
                let mark_sz = ig.size();
                painter.galley(pos2(body_x, cy - mark_sz.y / 2.0), ig, t.neutral_fg_secondary());
                body_x += mark_sz.x + mark_gap;
            }
            let hang_w = (content_right - body_x).max(Space::Sm.pts());
            if let Some(spans) = &self.caption {
                let slot = Rect::from_min_size(
                    pos2(body_x, cy - SUB_LINE_H / 2.0),
                    vec2(hang_w, SUB_LINE_H),
                );
                paint_caption(ui, spans, t.neutral_fg_secondary(), slot, SUB_SIZE, SUB_LINE_H);
            } else if !self.label.is_empty() {
                let slot = Rect::from_min_size(
                    pos2(body_x, cy - SUB_LINE_H / 2.0),
                    vec2(hang_w, SUB_LINE_H),
                );
                file_name::paint(ui, &self.label, ink, slot, SUB_SIZE, SUB_LINE_H);
            }
            return resp;
        }

        // Name sits in the upper band; subtitle under it when present.
        let name_lh = TypeRole::Body.line_height();
        let (name_cy, sub_cy) = if two_line {
            // Vertically stack within the rect with even-ish air.
            let block_h = name_lh + SUB_GAP + SUB_LINE_H;
            let block_top = paint_rect.center().y - block_h / 2.0;
            (block_top + name_lh / 2.0, block_top + name_lh + SUB_GAP + SUB_LINE_H / 2.0)
        } else {
            (paint_rect.center().y, paint_rect.center().y)
        };

        // File names (and path/from captions) via glyphon — emoji-safe.
        let name_slot =
            Rect::from_min_size(pos2(text_x, name_cy - name_lh / 2.0), vec2(max_w, name_lh));
        let name_w = file_name::paint_body(ui, display_file_name(&self.label), ink, name_slot);

        if let Some(c) = self.sync_dot {
            let cx = (text_x + name_w + SYNC_GAP + SYNC_R).min(content_right - trail_w - SYNC_R);
            painter.circle_filled(pos2(cx, name_cy), SYNC_R, c);
        }

        if let Some(spans) = &self.caption {
            let sub_font_size = SUB_SIZE;
            let sub_lh = SUB_LINE_H;
            let sub_slot =
                Rect::from_min_size(pos2(text_x, sub_cy - sub_lh / 2.0), vec2(max_w, sub_lh));
            paint_caption(ui, spans, t.neutral_fg_secondary(), sub_slot, sub_font_size, sub_lh);
        } else if let Some(sub) = &self.subtitle {
            // Middle-ellipsis for path segments, then glyphon paint (folder names may be emoji).
            let sub_font_size = SUB_SIZE;
            let sub_lh = SUB_LINE_H;
            let crumbs = path_crumbs(sub);
            let shown = middle_ellipsize_path_glyphon(ui, &crumbs, max_w, sub_font_size, sub_lh);
            let sub_slot =
                Rect::from_min_size(pos2(text_x, sub_cy - sub_lh / 2.0), vec2(max_w, sub_lh));
            file_name::paint(ui, &shown, t.neutral_fg_secondary(), sub_slot, sub_font_size, sub_lh);
        }

        // Trailing chrome (right → left) inside the content band.
        let trail_end = Space::Xs.pts();
        let mut tx = content_right - trail_end;
        if self.pinned {
            let pin_g = painter.layout_no_wrap(
                phosphor::PUSH_PIN.into(),
                phosphor_ui_font_id(),
                t.neutral_fg_secondary(),
            );
            tx -= pin_g.size().x;
            painter.galley(
                pos2(tx.max(text_x), name_cy - pin_g.size().y / 2.0),
                pin_g,
                t.neutral_fg_secondary(),
            );
            tx -= Space::Xs.pts();
        }
        if self.shared {
            let g = painter.layout_no_wrap(
                phosphor::USERS.into(),
                phosphor_ui_font_id(),
                t.neutral_fg_secondary(),
            );
            tx -= g.size().x;
            painter.galley(
                pos2(tx.max(text_x), name_cy - g.size().y / 2.0),
                g,
                t.neutral_fg_secondary(),
            );
        }

        resp
    }
}

fn paint_caption(
    ui: &mut Ui, spans: &[(String, bool)], color: Color32, slot: Rect, font_size: f32,
    line_height: f32,
) {
    let max_w = slot.width().max(0.0);
    if max_w < 0.5 || spans.is_empty() {
        return;
    }
    let refs: Vec<(&str, bool)> = spans.iter().map(|(s, b)| (s.as_str(), *b)).collect();
    let shaped = GlyphonLabel::new_rich(refs, color)
        .font_size(font_size)
        .line_height(line_height)
        .max_width(max_w)
        .text_overflow(TextOverflow::EndEllipsis)
        .build(ui.ctx());
    let drawn_w = shaped.size.x.min(max_w);
    let y = slot.center().y - line_height / 2.0;
    let text_rect = Rect::from_min_size(pos2(slot.left(), y), vec2(drawn_w, line_height));
    let clip = text_rect.intersect(ui.clip_rect()).intersect(slot);
    if clip.width() > 0.5 && clip.height() > 0.5 && ui.is_rect_visible(clip) {
        let area = shaped.text_area(text_rect, ui.ctx(), clip);
        ui.painter()
            .add(egui_wgpu_renderer::egui_wgpu::Callback::new_paint_callback(
                clip,
                crate::GlyphonRendererCallback::new(vec![area]),
            ));
    }
}

fn path_parts(path: &str) -> Vec<&str> {
    if path.contains(" / ") {
        path.split(" / ").filter(|p| !p.is_empty()).collect()
    } else {
        path.split('/').filter(|p| !p.is_empty()).collect()
    }
}

/// Display crumbs for a stored path (`/a/b/c` or `a / b / c`) → `a / b / c`.
pub fn path_crumbs(path: &str) -> String {
    let parts = path_parts(path);
    if parts.is_empty() { "Home".into() } else { parts.join(" / ") }
}

/// Parent crumbs of a stored path (`/a/b/c` → `a / b`). Root → `Home`.
pub fn parent_crumbs(path: &str) -> String {
    let parts = path_parts(path);
    match parts.split_last() {
        Some((_, parent)) if !parent.is_empty() => parent.join(" / "),
        _ => "Home".into(),
    }
}

/// Middle-ellipsis for ` / `-joined path segments: `a / b / c / d` → `a / … / d`.
///
/// Width measured with glyphon so emoji folder names size correctly.
pub fn ellipsize_path(ui: &Ui, path: &str, max_w: f32) -> String {
    middle_ellipsize_path_glyphon(ui, path, max_w, SUB_SIZE, SUB_LINE_H)
}

/// Middle-ellipsis for ` / `-joined path segments: `a / b / c / d` → `a / … / d`.
///
/// Width measured with glyphon so emoji folder names size correctly.
fn middle_ellipsize_path_glyphon(
    ui: &Ui, path: &str, max_w: f32, font_size: f32, line_height: f32,
) -> String {
    let measure =
        |s: &str| -> f32 { file_name::measure_sized(ui, s, font_size, line_height, f32::MAX) };
    if max_w <= 0.0 || path.is_empty() {
        return String::new();
    }
    if measure(path) <= max_w {
        return path.to_owned();
    }

    let parts: Vec<&str> = path.split(" / ").filter(|p| !p.is_empty()).collect();
    if parts.is_empty() {
        return String::new();
    }
    if parts.len() == 1 {
        return parts[0].to_owned();
    }

    let last = parts[parts.len() - 1];
    let ellipsis = "…";

    for right_keep in 1..parts.len() {
        let right = &parts[parts.len() - right_keep..];
        let right_s = right.join(" / ");
        for left_keep in (0..parts.len() - right_keep).rev() {
            let candidate = if left_keep == 0 {
                format!("{ellipsis} / {right_s}")
            } else {
                let left = parts[..left_keep].join(" / ");
                format!("{left} / {ellipsis} / {right_s}")
            };
            if measure(&candidate) <= max_w {
                return candidate;
            }
        }
    }

    let fallback = format!("{ellipsis} / {last}");
    if measure(&fallback) <= max_w { fallback } else { last.to_owned() }
}
