//! Settings-style form primitives: toggle, group, labeled rows.
//!
//! [`form_group`] is a **canvas plate** (canvas fill + hairline) — the form
//! itself, not a surface card nested on another plate.
//! Stack: [`section_label`] → [`form_group`] of [`form_row`]s → optional
//! group [`footnote`].
//!
//! Spacing hierarchy (less space ⇒ more connected; one token per gap, never
//! stacked smaller tokens to fake a larger one):
//!
//! - label ↔ detail: [`metrics::DETAIL_GAP`] (`Xs`)
//! - row ↔ row: `Space::Xs` (only *between* rows — not after the last)
//! - group plate edge: [`metrics::PAD_GROUP_Y`] (`Sm`)
//! - section ↔ section: `Space::Lg` at call sites
//!
//! Some rows need a **detail** line under the label (privacy, restart); most
//! don’t — pass detail only when the control can’t stand alone.

use egui::text::{LayoutJob, TextWrapping};
use egui::{Align, FontId, Id, Layout, Rect, Response, Stroke, TextFormat, Ui, pos2, vec2};

use crate::components::foundation::chrome::{
    Radius, STROKE_HAIRLINE, TOGGLE_ANIM_SECS, control_height, phosphor, phosphor_ui_font_id,
};
use crate::components::foundation::color::{FG_HOVER, Theme};
use crate::components::foundation::interact::sense_click;
use crate::components::foundation::layout::{claim, origin, place_at, ui_width};
use crate::components::foundation::space::Space;
use crate::components::foundation::spacer::Spacer;
use crate::components::foundation::typography::TypeRole;

/// Form row metrics — derived from control height + space tokens.
///
/// Vertical rhythm is **only** Spacers (F2-visible).
pub mod metrics {
    use super::*;

    /// Minimum band for a form label / detail line (`size × 1`).
    ///
    /// Vertical stack uses **shaped galley height** (see [`form_row_inner`]),
    /// not this alone. `1.0` is fine on macOS/SF; Linux/Noto galleys often run
    /// taller than `size`, so centering in a `size`-tall box used to collide
    /// title/subtitle. Prefer galley metrics over bumping this mult.
    pub fn line_box(role: TypeRole) -> f32 {
        role.size()
    }

    /// Leading inset inside a form group (row L).
    pub const PAD_L: Space = Space::Md;
    /// Trailing inset inside a form group (row R).
    pub const PAD_R: Space = Space::Sm;
    /// Top/bottom inset of the form-group plate (around the row stack).
    pub const PAD_GROUP_Y: Space = Space::Sm;
    /// Gap between label and detail line (same field).
    pub const DETAIL_GAP: Space = Space::Xs;

    /// Switch track height (intentionally under control height).
    pub const TOGGLE_H: f32 = 22.0;
    /// Switch width (≈ track height × golden-ish ratio, stable hit target).
    pub const TOGGLE_W: f32 = 38.0;
}

fn form_row_seq_id(ui: &Ui) -> Id {
    // Shared with [`form_group`] so the counter resets per plate.
    ui.id().with("form_row_seq")
}

/// One [`Space::Xs`] between rows (F2-visible). Skipped for the first row so
/// it doesn’t stack with [`metrics::PAD_GROUP_Y`].
fn form_row_before(ui: &mut Ui) {
    let id = form_row_seq_id(ui);
    let n = ui.ctx().data_mut(|d| {
        let n = d.get_temp::<u32>(id).unwrap_or(0);
        d.insert_temp(id, n + 1);
        n
    });
    if n > 0 {
        ui.add(Spacer::new(Space::Xs));
    }
}

// ── Toggle ──────────────────────────────────────────────────────────────────

/// Soft switch. Writes `*on` on click; returns the response (`.changed()` if toggled).
pub fn toggle(ui: &mut Ui, t: &Theme, on: &mut bool) -> Response {
    let (rect, mut resp) =
        ui.allocate_exact_size(vec2(metrics::TOGGLE_W, metrics::TOGGLE_H), sense_click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }

    let anim = ui.ctx().animate_bool_with_time_and_easing(
        resp.id.with("on"),
        *on,
        TOGGLE_ANIM_SECS,
        egui::emath::easing::cubic_out,
    );

    let off_track = t.neutral();
    let on_track = t.accent();
    let track = off_track.lerp_to_gamma(on_track, anim);

    let radius = rect.height() / 2.0;
    ui.painter().rect_filled(rect, radius, track);

    let kn = metrics::TOGGLE_H - Space::Xxs.pts() * 2.0;
    let inset = Space::Xxs.pts();
    let x = rect.left() + inset + (rect.width() - kn - inset * 2.0) * anim;
    let knob = Rect::from_min_size(pos2(x, rect.top() + inset), vec2(kn, kn));
    ui.painter()
        .circle_filled(knob.center(), kn / 2.0, t.neutral_bg());
    ui.painter().circle_stroke(
        knob.center(),
        kn / 2.0,
        Stroke::new(STROKE_HAIRLINE * 0.5, t.neutral()),
    );

    resp
}

// ── Group / section ─────────────────────────────────────────────────────────

/// Canvas plate holding form rows — canvas fill + Outside hairline.
///
/// Stack (all Spacers, F2-visible):
/// ```text
/// PAD_GROUP_Y (Sm)
///   form_row
///   Xs                  ← only between rows
///   form_row
/// PAD_GROUP_Y (Sm)
/// ```
pub fn form_group(ui: &mut Ui, t: &Theme, add: impl FnOnce(&mut Ui)) -> Response {
    crate::components::foundation::chrome::plate_content(
        ui,
        t.neutral_bg(),
        t.neutral(),
        Radius::Control.corner(),
        |ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            ui.set_width(crate::components::ui_width(ui));
            ui.ctx()
                .data_mut(|d| d.insert_temp(form_row_seq_id(ui), 0u32));
            ui.add(Spacer::new(metrics::PAD_GROUP_Y));
            add(ui);
            ui.add(Spacer::new(metrics::PAD_GROUP_Y));
        },
    )
}

/// Small caps-ish section eyebrow above a group.
pub fn section_label(ui: &mut Ui, t: &Theme, title: &str) {
    ui.label(
        TypeRole::Mono
            .rich(title.to_uppercase())
            .strong()
            .color(t.neutral_fg_secondary()),
    );
    ui.add(Spacer::new(Space::Xs));
}

/// Muted footnote under a group (privacy copy, restart notes).
pub fn footnote(ui: &mut Ui, t: &Theme, text: &str) {
    ui.add(Spacer::new(Space::Xs));
    ui.label(TypeRole::Mono.rich(text).color(t.neutral_fg_secondary()));
}

// ── Rows ────────────────────────────────────────────────────────────────────

/// Label left, trailing control right — fixed form row height.
pub fn form_row(ui: &mut Ui, t: &Theme, label: &str, trailing: impl FnOnce(&mut Ui)) {
    form_row_inner(ui, t, label, None, trailing);
}

/// Like [`form_row`], with a muted detail line under the label.
pub fn form_row_detail(
    ui: &mut Ui, t: &Theme, label: &str, detail: &str, trailing: impl FnOnce(&mut Ui),
) {
    form_row_inner(ui, t, label, Some(detail), trailing);
}

/// Height of the trailing control column (buttons / toggle / picker).
fn measure_trail_col() -> f32 {
    control_height()
}

/// Wrap text to `max_w` (form label / detail). Width is the slot left of trailing
/// controls — never paint past that into the switch / buttons.
fn layout_form_text(
    ui: &Ui, text: &str, font_id: FontId, color: egui::Color32, max_w: f32,
) -> std::sync::Arc<egui::Galley> {
    let max_w = max_w.max(1.0);
    let mut job = LayoutJob {
        wrap: TextWrapping {
            max_width: max_w,
            max_rows: 8,
            break_anywhere: false,
            overflow_character: Some('…'),
        },
        ..Default::default()
    };
    job.append(text, 0.0, TextFormat { font_id, color, ..Default::default() });
    ui.fonts(|f| f.layout_job(job))
}

fn form_row_inner(
    ui: &mut Ui, t: &Theme, label: &str, detail: Option<&str>, trailing: impl FnOnce(&mut Ui),
) {
    form_row_before(ui);

    let pad_l = metrics::PAD_L;
    let pad_r = metrics::PAD_R;
    let row_w = crate::components::ui_width(ui).max(1.0);
    let top_left = origin(ui);
    let mid_left = top_left.x + pad_l.pts();
    let mid_w = (row_w - pad_l.pts() - pad_r.pts()).max(1.0);

    // Place trailing first (control-height band) so we know how much width the
    // label column may use. Then wrap left text to that width and claim the
    // taller of the two columns.
    let trail_band =
        Rect::from_min_size(pos2(mid_left, top_left.y), vec2(mid_w, measure_trail_col()));
    let (_, trail_used) = place_at(ui, trail_band, Layout::right_to_left(Align::Center), |ui| {
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        trailing(ui);
    });
    let trail_left = if trail_used.width() > 0.0 { trail_used.left() } else { mid_left + mid_w };
    let label_w = (trail_left - mid_left - Space::Xs.pts()).max(Space::Xl.pts() * 2.0);

    let label_g = layout_form_text(ui, label, TypeRole::Body.font_id(), t.neutral_fg(), label_w);
    let detail_g = detail.map(|d| {
        layout_form_text(ui, d, TypeRole::Mono.font_id(), t.neutral_fg_secondary(), label_w)
    });

    let mut left_h = label_g.size().y.max(metrics::line_box(TypeRole::Body));
    if let Some(ref dg) = detail_g {
        left_h += metrics::DETAIL_GAP.pts() + dg.size().y.max(metrics::line_box(TypeRole::Mono));
    }
    let h = left_h.max(measure_trail_col());

    let outer = Rect::from_min_size(top_left, vec2(row_w, h));
    Spacer::paint_at(ui, pad_l, Rect::from_min_size(outer.min, vec2(pad_l.pts(), h)));
    Spacer::paint_at(
        ui,
        pad_r,
        Rect::from_min_size(pos2(outer.right() - pad_r.pts(), outer.top()), vec2(pad_r.pts(), h)),
    );

    // Left column: vertically centered in the row. Trailing stays in the top
    // control band (already placed) — fine when detail wraps taller than the
    // switch; single-line rows keep both sides in control height.
    let x = mid_left;
    let mut y = top_left.y + (h - left_h) / 2.0;
    let label_h = label_g.size().y.max(metrics::line_box(TypeRole::Body));
    ui.painter().galley(pos2(x, y), label_g, t.neutral_fg());
    y += label_h;
    if let Some(dg) = detail_g {
        Spacer::paint_at(
            ui,
            metrics::DETAIL_GAP,
            Rect::from_min_size(pos2(x, y), vec2(label_w, metrics::DETAIL_GAP.pts())),
        );
        y += metrics::DETAIL_GAP.pts();
        ui.painter()
            .galley(pos2(x, y), dg, t.neutral_fg_secondary());
    }

    claim(ui, outer);
}

/// Read-only value on the trailing edge (debug / plan status).
pub fn form_value(ui: &mut Ui, t: &Theme, label: &str, value: &str) {
    form_row(ui, t, label, |ui| {
        ui.label(TypeRole::Body.rich(value).color(t.neutral_fg_secondary()));
    });
}

/// Label + [`toggle`] in a form row. Returns the toggle response.
pub fn form_toggle(ui: &mut Ui, t: &Theme, label: &str, on: &mut bool) -> Response {
    let mut out = None;
    form_row(ui, t, label, |ui| {
        out = Some(toggle(ui, t, on));
    });
    out.expect("form_row always runs trailing")
}

/// Toggle row with a muted detail under the label (privacy, restart, etc.).
pub fn form_toggle_detail(
    ui: &mut Ui, t: &Theme, label: &str, detail: &str, on: &mut bool,
) -> Response {
    let mut out = None;
    form_row_detail(ui, t, label, detail, |ui| {
        out = Some(toggle(ui, t, on));
    });
    out.expect("form_row_detail always runs trailing")
}

/// Label + exclusive [`super::segmented`] in the trailing slot.
pub fn form_segmented(
    ui: &mut Ui, t: &Theme, label: &str, options: &[&str], selected: &mut usize,
) -> Response {
    let mut out = None;
    form_row(ui, t, label, |ui| {
        out = Some(crate::components::atoms::segmented::segmented(ui, t, options, selected));
    });
    out.expect("form_row always runs trailing")
}

/// Label + [`super::picker`] in the trailing slot (single value, menu of options).
pub fn form_picker(
    ui: &mut Ui, t: &Theme, label: &str, options: &[&str], selected: &mut usize,
) -> Response {
    let mut out = None;
    form_row(ui, t, label, |ui| {
        out = Some(crate::components::atoms::picker::picker(ui, t, options, selected));
    });
    out.expect("form_row always runs trailing")
}

/// Checkbox + wrapping copy; the whole row toggles (logout / key-backup ack).
pub fn ack_row(ui: &mut Ui, t: &Theme, label: &str, on: &mut bool) {
    let box_s = TypeRole::Body.line_height().min(control_height() * 0.85);
    let gap = Space::Sm.pts();
    let max_w = ui_width(ui).max(1.0);
    let text_w = (max_w - box_s - gap).max(1.0);
    let galley =
        ui.painter()
            .layout(label.to_owned(), TypeRole::Body.font_id(), t.neutral_fg(), text_w);
    let row_h = galley.size().y.max(box_s);
    let (row, resp) = ui.allocate_exact_size(egui::vec2(max_w, row_h), sense_click());
    if resp.clicked() {
        *on = !*on;
    }
    let over = ui.ctx().rect_contains_pointer(ui.layer_id(), row);
    let box_rect = egui::Rect::from_min_size(
        egui::pos2(row.left(), row.center().y - box_s / 2.0),
        egui::vec2(box_s, box_s),
    );
    let ground = t.neutral_bg();
    let fill = if *on {
        t.accent()
    } else if over {
        t.wash_toward_neutral_fg(ground, FG_HOVER)
    } else {
        ground
    };
    ui.painter().rect(
        box_rect,
        Radius::Sm.corner(),
        fill,
        Stroke::new(STROKE_HAIRLINE, if *on { t.accent() } else { t.neutral() }),
        egui::StrokeKind::Inside,
    );
    if *on {
        let ig = ui.painter().layout_no_wrap(
            phosphor::CHECK.into(),
            phosphor_ui_font_id(),
            t.neutral_bg(),
        );
        ui.painter().galley(
            egui::pos2(
                box_rect.center().x - ig.size().x / 2.0,
                box_rect.center().y - ig.size().y / 2.0,
            ),
            ig,
            t.neutral_bg(),
        );
    }
    ui.painter()
        .galley(egui::pos2(row.left() + box_s + gap, row.top()), galley, t.neutral_fg());
}
