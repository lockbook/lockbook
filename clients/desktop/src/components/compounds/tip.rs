//! Hover tips — design-styled cards instead of stock `on_hover_text`.
//!
//! Dwell / chain timing for hover tips:
//! - **First tip:** wait `tooltip_delay` after the pointer is still.
//! - **Chain:** brief hop between hosts (`tooltip_grace_time` /
//!   [`TIP_CHAIN_GRACE_SECS`]) — not the full dwell again.
//! - **Leave host:** clear “open host” so re-entry after the chain window
//!   waits a full dwell (do not keep stay-open across seconds away).
//! - **Click:** dismisses; clears open-host + last-shown so a wiggle on the
//!   same control does **not** reopen until a full still dwell again.
//!
//! Anchored to the host control (not the cursor). Canvas plate + hairline.

use egui::{
    Align2, Area, Context, Frame, Id, Margin, Order, Pos2, Response, Sense, Ui, Vec2, vec2,
};

use crate::components::foundation::chrome::{Radius, STROKE_HAIRLINE, TIP_CHAIN_GRACE_SECS};
use crate::components::foundation::color::ThemeExt;
use crate::components::foundation::space::Space;
use crate::components::foundation::typography::TypeRole;

/// Compact one-line tips (toolbar / footer).
const MAX_W: f32 = 280.0;

/// Last wall time any design tip was painted (chaining / grace).
const LAST_TIP_TIME: &str = "design_tip_last_shown";
/// Host `Response::id` currently showing a tip — stay open without delay.
const OPEN_TIP_HOST: &str = "design_tip_open_host";

/// One-line chrome tip (toolbar, footer).
pub fn tip_text(ctx: &Context, resp: &Response, text: impl Into<String>) {
    let text = text.into();
    if text.is_empty() {
        return;
    }
    if !should_show_tip(ctx, resp) {
        return;
    }
    note_tip_shown(ctx, resp.id);
    show_anchored(ctx, resp, MAX_W, |ui| {
        let t = ui.ctx().get_lb_theme();
        ui.set_max_width(MAX_W);
        ui.label(TypeRole::Body.rich(text).color(t.neutral_fg()));
    });
}

/// Multi-line tip — hover/delay use `hit`, placement uses `place`
/// (e.g. footer: tip only over status text, centered on full strip).
/// `max_w` caps **content** width (frame pad is outside that).
pub fn tip_card_placed(
    ctx: &Context, hit: &Response, place: egui::Rect, max_w: f32,
    add_contents: impl FnOnce(&mut Ui),
) {
    if !should_show_tip(ctx, hit) {
        return;
    }
    note_tip_shown(ctx, hit.id);
    show_anchored_at(ctx, hit.id, place, max_w.max(1.0), add_contents);
}

fn note_tip_shown(ctx: &Context, host: Id) {
    let now = ctx.input(|i| i.time);
    ctx.data_mut(|d| {
        // Mut access keeps temps alive across frames (gaps between hosts).
        d.insert_temp(Id::new(LAST_TIP_TIME), now);
        d.insert_temp(Id::new(OPEN_TIP_HOST), host);
    });
}

/// Kill chain so click-dismiss doesn’t reopen on the next micro-move.
fn chill_tip_session(ctx: &Context) {
    ctx.data_mut(|d| {
        d.insert_temp(Id::new(LAST_TIP_TIME), f64::NEG_INFINITY);
        d.insert_temp(Id::new(OPEN_TIP_HOST), Id::NULL);
    });
}

/// egui / workspace-aligned delay + chaining + click-dismiss.
fn should_show_tip(ctx: &Context, resp: &Response) -> bool {
    // Prefer layer hit over raw `hovered()` near interact_radius edges.
    let pointer_over = ctx.rect_contains_pointer(resp.layer_id, resp.interact_rect);
    if !pointer_over || ctx.dragged_id().is_some() {
        // Left this host: drop “stay open” so re-entry waits a full dwell again.
        // Keep LAST_TIP_TIME so a short hop to a neighbor can still chain.
        end_open_if_host(ctx, resp.id);
        return false;
    }
    if !ctx.input(|i| i.pointer.has_pointer()) {
        end_open_if_host(ctx, resp.id);
        return false;
    }

    let style = ctx.style();
    let tooltip_delay = style.interaction.tooltip_delay;
    let tooltip_grace_time = style.interaction.tooltip_grace_time;

    let (
        time_since_last_scroll,
        time_since_last_click,
        time_since_last_pointer_movement,
        pointer_still,
        smooth_scroll,
        now,
        any_click,
    ) = ctx.input(|i| {
        (
            i.time_since_last_scroll(),
            i.pointer.time_since_last_click(),
            i.pointer.time_since_last_movement(),
            i.pointer.is_still(),
            i.smooth_scroll_delta,
            i.time,
            i.pointer.any_click(),
        )
    });

    // Click on this host (or any click while over it): dismiss and chill chain.
    // Workspace: click-then-rest must not open; we also clear last-shown so a
    // post-click wiggle cannot chain-reopen.
    if any_click || resp.clicked() || resp.secondary_clicked() || resp.middle_clicked() {
        chill_tip_session(ctx);
        return false;
    }

    // Click more recent than move → stay dismissed until a later still dwell.
    let clicked_more_recently_than_moved =
        time_since_last_click < time_since_last_pointer_movement + 0.1;
    if clicked_more_recently_than_moved {
        chill_tip_session(ctx);
        return false;
    }

    // Don’t flash tips while scrolling.
    if time_since_last_scroll < tooltip_delay {
        ctx.request_repaint_after_secs(tooltip_delay - time_since_last_scroll);
        return false;
    }

    let (last_shown, open_host) = ctx.data_mut(|d| {
        let last = *d.get_temp_mut_or_insert_with(Id::new(LAST_TIP_TIME), || f64::NEG_INFINITY);
        let host = d.get_temp::<Id>(Id::new(OPEN_TIP_HOST));
        (last, host)
    });

    let is_our_tip_open = open_host == Some(resp.id);
    let seconds_since_last_tip = now - last_shown;
    // Chain only for a short hop between hosts — never `tooltip_delay` (that
    // made “leave for seconds and re-enter” feel like chain forever).
    let chain_window = f64::from(tooltip_grace_time).max(f64::from(TIP_CHAIN_GRACE_SECS));
    let in_tip_chain = seconds_since_last_tip < chain_window;

    if is_our_tip_open {
        // Same host still hovered — stay open without re-waiting.
        return true;
    }

    if in_tip_chain {
        // Neighbor host while chain is still warm — open without full dwell.
        return true;
    }

    // First tip (or chain expired): require still pointer + delay, like egui.
    if style.interaction.show_tooltips_only_when_still
        && !(pointer_still && smooth_scroll == Vec2::ZERO)
    {
        ctx.request_repaint();
        return false;
    }

    let time_since_last_interaction = time_since_last_scroll
        .min(time_since_last_pointer_movement)
        .min(time_since_last_click);
    let time_til_tooltip = tooltip_delay - time_since_last_interaction;
    if time_til_tooltip > 0.0 {
        ctx.request_repaint_after_secs(time_til_tooltip);
        return false;
    }

    true
}

/// Clear “this host’s tip is open” when the pointer leaves (keep last-shown time).
fn end_open_if_host(ctx: &Context, host: Id) {
    ctx.data_mut(|d| {
        if d.get_temp::<Id>(Id::new(OPEN_TIP_HOST)) == Some(host) {
            d.insert_temp(Id::new(OPEN_TIP_HOST), Id::NULL);
        }
    });
}

fn show_anchored(ctx: &Context, resp: &Response, max_w: f32, add_contents: impl FnOnce(&mut Ui)) {
    show_anchored_at(ctx, resp.id, resp.rect, max_w, add_contents);
}

fn show_anchored_at(
    ctx: &Context, host_id: Id, host: egui::Rect, max_w: f32, add_contents: impl FnOnce(&mut Ui),
) {
    let t = ctx.get_lb_theme();
    let screen = ctx.screen_rect();
    let prefer_below = screen.bottom() - host.bottom() >= 72.0;
    let gap = Space::Xs.pts();

    let (pos, pivot) = if prefer_below {
        (Pos2::new(host.center().x, host.bottom() + gap), Align2::CENTER_TOP)
    } else {
        (Pos2::new(host.center().x, host.top() - gap), Align2::CENTER_BOTTOM)
    };

    Area::new(host_id.with("design_tip"))
        .order(Order::Tooltip)
        .fixed_pos(pos)
        .pivot(pivot)
        .constrain(true)
        .sense(Sense::hover())
        .default_size(vec2(0.0, 0.0))
        .fade_in(false)
        .show(ctx, |ui| {
            Frame::new()
                .fill(t.neutral_bg())
                .stroke(egui::Stroke::new(STROKE_HAIRLINE, t.neutral()))
                .corner_radius(Radius::Control.corner())
                .inner_margin(Margin::symmetric(Space::Sm.pts() as i8, Space::Xs.pts() as i8))
                .show(ui, |ui| {
                    ui.set_max_width(max_w);
                    add_contents(ui);
                });
        });
}
