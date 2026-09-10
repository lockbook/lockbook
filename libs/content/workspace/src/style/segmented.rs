//! Exclusive multi-option control — theme mode, open-in-tab policy, etc.
//!
//! Quiet language: soft **recessed track** + sliding **canvas pill** under the
//! active label. Metrics follow control height and space tokens.

use egui::{Response, Stroke, Ui, pos2, vec2};

use crate::style::chrome::{HOVER_ANIM_SECS, Radius, STROKE_HAIRLINE, control_height};
use crate::style::color::{FG_HOVER, Theme};
use crate::style::interact::sense_click;
use crate::style::space::Space;
use crate::style::typography::TypeRole;

/// Outer track height — same ladder as buttons / fields / form trailers.
///
/// Pill inset is **inside** this height (`Xxs` each edge). Do not add outer
/// height here: form rows allocate only [`control_height`] for the trailing
/// column, and a taller track paints past the row bottom.
pub fn segmented_h() -> f32 {
    control_height()
}

/// Equalize segment widths so the strip doesn’t look lopsided.
const EQUALIZE: bool = true;

/// Natural width of a [`segmented`] for `options` (equalized cells).
pub fn segmented_width(ui: &Ui, t: &Theme, options: &[&str]) -> f32 {
    segmented_cell_widths(ui, t, options).iter().sum()
}

fn segmented_cell_widths(ui: &Ui, t: &Theme, options: &[&str]) -> Vec<f32> {
    let n = options.len().max(1);
    let h = segmented_h();
    let pad_x = Space::Md.pts();
    let font = TypeRole::Body.font_id();
    let mut natural = Vec::with_capacity(n);
    let mut max_w = 0.0_f32;
    for &label in options {
        let g = ui
            .painter()
            .layout_no_wrap(label.to_owned(), font.clone(), t.neutral_fg());
        let w = (g.size().x + pad_x * 2.0).max(h * 1.5);
        natural.push(w);
        max_w = max_w.max(w);
    }
    if EQUALIZE { vec![max_w; n] } else { natural }
}

/// Exclusive segmented control. `options` should be 2–5 short labels.
/// Marks the response `.changed()` when `selected` updates.
pub fn segmented(ui: &mut Ui, t: &Theme, options: &[&str], selected: &mut usize) -> Response {
    let n = options.len().max(1);
    *selected = (*selected).min(n - 1);

    let h = segmented_h();
    let pill_inset = Space::Xxs.pts();
    let font = TypeRole::Body.font_id();
    let widths = segmented_cell_widths(ui, t, options);
    let total_w: f32 = widths.iter().sum();

    let (track, mut resp) = ui.allocate_exact_size(vec2(total_w, h), sense_click());
    let track_r = Radius::Control.corner();
    let track_fill = t.neutral_bg_secondary();
    ui.painter().rect_filled(track, track_r, track_fill);

    let mut lefts = Vec::with_capacity(n);
    {
        let mut x = track.left();
        for &w in &widths {
            lefts.push(x);
            x += w;
        }
    }

    let segs: Vec<egui::Rect> = (0..n)
        .map(|i| egui::Rect::from_min_size(pos2(lefts[i], track.top()), vec2(widths[i], h)))
        .collect();

    let layer = ui.layer_id();
    let mut pointer_seg: Option<usize> = None;
    if ui.ctx().rect_contains_pointer(layer, track) {
        for (i, seg) in segs.iter().enumerate() {
            if ui.ctx().rect_contains_pointer(layer, *seg) {
                pointer_seg = Some(i);
                break;
            }
        }
    }

    let mut changed = false;
    if resp.clicked() {
        if let Some(i) = pointer_seg {
            if i != *selected {
                *selected = i;
                changed = true;
            }
        }
    }

    // Animate in **track-local** space. Absolute screen x would tween from the
    // first layout rect (often left/default) into the centered sheet — looks
    // like the pill slides in from the window edge on open.
    let target_rel_x = (lefts[*selected] - track.left()) + pill_inset;
    let target_w = (widths[*selected] - pill_inset * 2.0).max(0.0);
    let pill_top = track.top() + pill_inset;
    let pill_h = (h - pill_inset * 2.0).max(0.0);

    let anim_id = resp.id.with("pill");
    let pill_rel_x =
        ui.ctx()
            .animate_value_with_time(anim_id.with("x"), target_rel_x, HOVER_ANIM_SECS);
    let pill_w = ui
        .ctx()
        .animate_value_with_time(anim_id.with("w"), target_w, HOVER_ANIM_SECS);
    let pill_left = track.left() + pill_rel_x;
    let pill = egui::Rect::from_min_size(pos2(pill_left, pill_top), vec2(pill_w, pill_h));
    let pill_r =
        ((pill_h / 2.0).min(Radius::Control.pts() as f32 - 1.0)).max(Radius::Sm.pts() as f32);

    ui.painter().rect_filled(pill, pill_r, t.neutral_bg());
    ui.painter().rect_stroke(
        pill,
        pill_r,
        Stroke::new(STROKE_HAIRLINE, t.neutral()),
        egui::StrokeKind::Inside,
    );

    let mut hover_settling = false;
    for (i, &label) in options.iter().enumerate() {
        let seg = segs[i];
        let over = pointer_seg == Some(i);
        let active = i == *selected;
        let hover_t = ui.ctx().animate_bool(resp.id.with("hov").with(i), over);
        if over && hover_t < 0.999 || !over && hover_t > 0.001 {
            hover_settling = true;
        }

        if !active && hover_t > 0.0 {
            let wash = seg.shrink2(vec2(pill_inset, pill_inset));
            ui.painter().rect_filled(
                wash,
                pill_r,
                t.wash_toward_neutral_fg(track_fill, FG_HOVER * hover_t),
            );
        }

        let ink = if active {
            t.neutral_fg()
        } else {
            t.neutral_fg_secondary()
                .lerp_to_gamma(t.neutral_fg(), hover_t)
        };
        let g = ui
            .painter()
            .layout_no_wrap(label.to_owned(), font.clone(), ink);
        ui.painter().galley(
            pos2(seg.center().x - g.size().x / 2.0, seg.center().y - g.size().y / 2.0),
            g,
            ink,
        );
    }

    let pill_settling = (pill_rel_x - target_rel_x).abs() > 0.5 || (pill_w - target_w).abs() > 0.5;
    if pill_settling || hover_settling {
        ui.ctx().request_repaint();
    }

    if changed {
        resp.mark_changed();
    }
    resp
}
