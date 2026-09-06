//! Overlay (floating) scrollbars — Files, search, folder pickers, markdown,
//! chat, PDF.
//!
//! Idle hidden; reveal on content scroll or thumb hover/drag; fade after leave.
//! A host may set [`SIDEBAR_RESIZING_LATCH`] to force-hide while a split is dragged.
//!
//! egui [`ScrollArea`]: wrap with [`with_overlay_scroll`].
//! Custom bars (affine markdown): [`tick`] + [`paint`].

use std::time::Duration;

use egui::{Context, Id, Rect, Ui};

use super::color::ThemeExt;

const FADE_SECS: f64 = 0.85;

/// Host sets this while a sidebar splitter is mid-drag.
pub const SIDEBAR_RESIZING_LATCH: &str = "lb_sidebar_resizing";

/// Overlay thumb width. Matches Files / search floating bars.
pub const BAR_WIDTH: f32 = 6.0;

/// Corner radius for a 6px pill.
const BAR_RADIUS: f32 = 3.0;

#[derive(Clone, Copy)]
struct State {
    last_offset_y: f32,
    last_active: f64,
    thumb_held: bool,
}

impl Default for State {
    fn default() -> Self {
        Self { last_offset_y: f32::NAN, last_active: f64::NEG_INFINITY, thumb_held: false }
    }
}

fn separator_dragging(ui: &Ui) -> bool {
    ui.ctx().data(|d| {
        d.get_temp::<bool>(Id::new(SIDEBAR_RESIZING_LATCH))
            .unwrap_or(false)
    })
}

pub fn bar_held(ctx: &Context, scroll_area_id: Id) -> bool {
    (0..2).any(|d| {
        let bar_id = scroll_area_id.with(d);
        ctx.is_being_dragged(bar_id)
            || ctx
                .read_response(bar_id)
                .is_some_and(|r| r.hovered() || r.dragged())
    })
}

fn is_shown(ui: &Ui, id: Id) -> bool {
    if separator_dragging(ui) {
        return false;
    }
    let now = ui.input(|i| i.time);
    let st = ui
        .ctx()
        .data(|d| d.get_temp::<State>(id))
        .unwrap_or_default();
    st.thumb_held || now - st.last_active < FADE_SECS
}

fn request_hide_repaint(ui: &Ui, id: Id) {
    let now = ui.input(|i| i.time);
    let st = ui
        .ctx()
        .data(|d| d.get_temp::<State>(id))
        .unwrap_or_default();
    if is_shown(ui, id) && !st.thumb_held {
        let remaining = (FADE_SECS - (now - st.last_active)).max(0.0);
        ui.ctx()
            .request_repaint_after(Duration::from_secs_f64(remaining.max(1.0 / 120.0)));
    }
}

fn apply_egui_style(ui: &mut Ui, show: bool) {
    {
        let style = ui.style_mut();
        let ink = style.visuals.widgets.inactive.fg_stroke.color;
        let radius = style.visuals.widgets.inactive.corner_radius;
        for w in [
            &mut style.visuals.widgets.inactive,
            &mut style.visuals.widgets.hovered,
            &mut style.visuals.widgets.active,
        ] {
            w.corner_radius = radius;
            w.expansion = 0.0;
            w.fg_stroke.color = ink;
            w.bg_fill = ink;
            w.weak_bg_fill = ink;
        }
    }

    let scroll = &mut ui.style_mut().spacing.scroll;
    scroll.floating = true;
    scroll.floating_width = BAR_WIDTH;
    scroll.bar_width = BAR_WIDTH;
    scroll.bar_outer_margin = 0.0;
    scroll.dormant_handle_opacity = 0.0;
    scroll.dormant_background_opacity = 0.0;
    scroll.foreground_color = true;

    if show {
        scroll.active_handle_opacity = 0.5;
        scroll.active_background_opacity = 0.2;
        scroll.interact_handle_opacity = 0.85;
        scroll.interact_background_opacity = 0.35;
    } else {
        scroll.active_handle_opacity = 0.0;
        scroll.active_background_opacity = 0.0;
        scroll.interact_handle_opacity = 0.0;
        scroll.interact_background_opacity = 0.0;
    }
}

/// Call inside a `ui.scope` so widget visuals stay local. Prefer [`with_overlay_scroll`]
/// for egui `ScrollArea`. Custom bars should use [`tick`] + [`paint`] instead.
pub fn prepare(ui: &mut Ui, id: Id) {
    request_hide_repaint(ui, id);
    apply_egui_style(ui, is_shown(ui, id));
}

pub fn note_offset(ui: &Ui, id: Id, offset_y: f32) {
    let now = ui.input(|i| i.time);
    ui.ctx().data_mut(|d| {
        let st = d.get_temp_mut_or_default::<State>(id);
        if st.last_offset_y.is_nan() {
            st.last_offset_y = offset_y;
            return;
        }
        if (offset_y - st.last_offset_y).abs() > 0.25 {
            st.last_active = now;
            st.last_offset_y = offset_y;
        }
    });
}

pub fn note_bar_interaction(ui: &Ui, id: Id, bar_held: bool) {
    let now = ui.input(|i| i.time);
    let left = ui.ctx().data_mut(|d| {
        let st = d.get_temp_mut_or_default::<State>(id);
        let was = st.thumb_held;
        st.thumb_held = bar_held;
        if bar_held || was {
            st.last_active = now;
        }
        was && !bar_held
    });
    if left {
        ui.ctx()
            .request_repaint_after(Duration::from_secs_f64(FADE_SECS));
    }
}

/// Note scroll + bar hover, return whether the overlay should paint this frame.
///
/// For custom (non-egui) bars. Call after reading this frame's offset / hover,
/// then [`paint`] when the result is true.
pub fn tick(ui: &mut Ui, id: Id, offset_y: f32, bar_held: bool) -> bool {
    note_offset(ui, id, offset_y);
    note_bar_interaction(ui, id, bar_held);
    request_hide_repaint(ui, id);
    is_shown(ui, id)
}

/// Overlay track + thumb. Opacities match egui floating bars (idle / interact).
pub fn paint(ui: &Ui, track: Rect, thumb: Rect, interact: bool) {
    let t = ui.ctx().get_lb_theme();
    let ink = t.neutral_fg();
    let (track_a, thumb_a) = if interact { (0.35, 0.85) } else { (0.20, 0.50) };
    ui.painter()
        .rect_filled(track, BAR_RADIUS, ink.gamma_multiply(track_a));
    ui.painter()
        .rect_filled(thumb, BAR_RADIUS, ink.gamma_multiply(thumb_a));
}

/// Scope + prepare style, run `f`, then note offset and bar hold.
///
/// `f` returns `(result, offset_y, scroll_area_id)`.
pub fn with_overlay_scroll<R>(ui: &mut Ui, id: Id, f: impl FnOnce(&mut Ui) -> (R, f32, Id)) -> R {
    ui.scope(|ui| {
        prepare(ui, id);
        let (result, offset_y, scroll_area_id) = f(ui);
        note_offset(ui, id, offset_y);
        note_bar_interaction(ui, id, bar_held(ui.ctx(), scroll_area_id));
        result
    })
    .inner
}
