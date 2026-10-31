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
    Align2, Area, Context, Frame, Id, LayerId, Margin, Order, Pos2, Rect, Response, Ui, Vec2, vec2,
};

use super::chrome::{Radius, TIP_CHAIN_DWELL_SECS, TIP_CHAIN_GRACE_SECS};
use super::color::ThemeExt;
use super::space::Space;
use super::typography::TypeRole;

/// Compact one-line tips (toolbar / footer).
const MAX_W: f32 = 280.0;

/// Last wall time any design tip was painted (chaining / grace).
const LAST_TIP_TIME: &str = "design_tip_last_shown";
/// Host `Response::id` currently showing a tip — stay open without delay.
const OPEN_TIP_HOST: &str = "design_tip_open_host";
/// Where the pointer last was over a reachable card's host.
const REACH_EXIT: &str = "design_tip_reach_exit";
/// The host whose reachable card holds the pointer, and the frame it did.
const REACH_HELD: &str = "design_tip_reach_held";
/// The layer of the last reachable card shown.
const REACH_LAYER: &str = "design_tip_reach_layer";

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
/// `max_w` caps **content** width (frame pad, `margin`, is outside that).
pub fn tip_card_placed(
    ctx: &Context, hit: &Response, place: egui::Rect, max_w: f32, margin: Margin,
    add_contents: impl FnOnce(&mut Ui),
) {
    if !should_show_tip(ctx, hit) {
        return;
    }
    note_tip_shown(ctx, hit.id);
    show_anchored_at(ctx, hit.id, place, max_w.max(1.0), margin, add_contents);
}

/// A card tip the pointer can move onto to use what's on it. It stays while
/// the pointer is on the card, inside the triangle from where it left the
/// host to the card's near edge, or in a menu the card opened.
pub fn tip_card_reachable(
    ctx: &Context, hit: &Response, place: egui::Rect, max_w: f32, margin: Margin,
    add_contents: impl FnOnce(&mut Ui),
) {
    let id = hit.id.with("design_tip");
    let pointer = ctx.input(|i| i.pointer.latest_pos());
    let exit_id = Id::unique(REACH_EXIT);
    if hit.contains_pointer() {
        ctx.data_mut(|d| d.insert_temp(exit_id, pointer));
    }
    let open = ctx.data(|d| d.get_temp::<Id>(Id::unique(OPEN_TIP_HOST))) == Some(hit.id);
    let card = ctx.memory(|m| m.area_rect(id));
    let exit = ctx.data(|d| d.get_temp::<Option<Pos2>>(exit_id)).flatten();
    // a scroll that takes the host from under the pointer puts the card away
    let scrolling = ctx.input(|i| i.smooth_scroll_delta != Vec2::ZERO);
    let held = open
        && !hit.contains_pointer()
        && (crate::style::context_menu::is_any_open(ctx)
            || !scrolling
                && matches!((pointer, card), (Some(p), Some(c)) if c.contains(p)
                    || exit.is_some_and(|e| in_reach(p, e, c))));
    if held {
        let frame = ctx.cumulative_frame_nr();
        ctx.data_mut(|d| d.insert_temp(Id::unique(REACH_HELD), (hit.id, frame)));
    } else if !should_show_tip(ctx, hit) {
        return;
    }
    note_tip_shown(ctx, hit.id);
    let layer = LayerId::new(Order::Foreground, id);
    ctx.data_mut(|d| d.insert_temp(Id::unique(REACH_LAYER), layer));
    show_area(ctx, hit.id, place, max_w, margin, true, add_contents);
}

/// The pointer is in `rect` with a reachable card on top: the wheel scrolls
/// what's under the card.
pub fn scrolls_through_card(ctx: &Context, rect: egui::Rect) -> bool {
    let card = ctx.data(|d| d.get_temp::<LayerId>(Id::unique(REACH_LAYER)));
    ctx.input(|i| i.pointer.latest_pos())
        .is_some_and(|p| rect.contains(p) && card.is_some() && ctx.layer_id_at(p) == card)
}

/// Puts away whatever tip is open, as a click does.
pub fn dismiss_tip(ctx: &Context) {
    chill_tip_session(ctx);
}

/// Within the triangle from `exit` to `card`'s nearer horizontal edge.
fn in_reach(p: Pos2, exit: Pos2, card: egui::Rect) -> bool {
    let y = if exit.y <= card.top() { card.top() } else { card.bottom() };
    let (a, b) = (Pos2::new(card.left(), y), Pos2::new(card.right(), y));
    let side = |u: Pos2, v: Pos2| (v - u).x * (p - u).y - (v - u).y * (p - u).x;
    let (d1, d2, d3) = (side(exit, a), side(a, b), side(b, exit));
    !((d1 < 0.0 || d2 < 0.0 || d3 < 0.0) && (d1 > 0.0 || d2 > 0.0 || d3 > 0.0))
}

/// `host`'s reachable card held the pointer this frame or last.
fn reach_held(ctx: &Context, host: Id) -> bool {
    let now = ctx.cumulative_frame_nr();
    ctx.data(|d| d.get_temp::<(Id, u64)>(Id::unique(REACH_HELD)))
        .is_some_and(|(held, frame)| held == host && frame + 1 >= now)
}

fn note_tip_shown(ctx: &Context, host: Id) {
    let now = ctx.input(|i| i.time);
    ctx.data_mut(|d| {
        d.insert_temp(Id::unique(LAST_TIP_TIME), now);
        d.insert_temp(Id::unique(OPEN_TIP_HOST), host);
    });
}

/// Kill chain so click-dismiss doesn’t reopen on the next micro-move.
fn chill_tip_session(ctx: &Context) {
    ctx.data_mut(|d| {
        d.insert_temp(Id::unique(LAST_TIP_TIME), f64::NEG_INFINITY);
        d.insert_temp(Id::unique(OPEN_TIP_HOST), Id::NULL);
    });
}

/// egui / workspace-aligned delay + chaining + click-dismiss.
fn should_show_tip(ctx: &Context, resp: &Response) -> bool {
    // Widget hit, not `layer_id_at` — canvas islands live on a custom overlay
    // layer that is not an Area, so `rect_contains_pointer(layer, …)` is always
    // false there even when the control is hovered.
    let pointer_over = resp.contains_pointer();
    if !pointer_over || ctx.dragged_id().is_some() {
        end_open_if_host(ctx, resp.id);
        return false;
    }
    if !ctx.input(|i| i.pointer.has_pointer()) {
        end_open_if_host(ctx, resp.id);
        return false;
    }

    let style = ctx.global_style();
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

    if any_click || resp.clicked() || resp.secondary_clicked() || resp.middle_clicked() {
        chill_tip_session(ctx);
        return false;
    }

    let clicked_more_recently_than_moved =
        time_since_last_click < time_since_last_pointer_movement + 0.1;
    if clicked_more_recently_than_moved {
        chill_tip_session(ctx);
        return false;
    }

    let (last_shown, open_host) = ctx.data_mut(|d| {
        let last = *d.get_temp_mut_or_insert_with(Id::unique(LAST_TIP_TIME), || f64::NEG_INFINITY);
        let host = d.get_temp::<Id>(Id::unique(OPEN_TIP_HOST));
        (last, host)
    });

    // an open tip rides out a scroll that leaves its host under the pointer
    if open_host == Some(resp.id) {
        return true;
    }
    // the pointer is on its way to a card it can reach
    if open_host.is_some_and(|host| reach_held(ctx, host)) {
        return false;
    }

    if time_since_last_scroll < tooltip_delay {
        ctx.request_repaint_after_secs(tooltip_delay - time_since_last_scroll);
        return false;
    }

    let seconds_since_last_tip = now - last_shown;
    let chain_window = f64::from(tooltip_grace_time).max(f64::from(TIP_CHAIN_GRACE_SECS))
        + f64::from(TIP_CHAIN_DWELL_SECS);
    let in_tip_chain = seconds_since_last_tip < chain_window;

    if in_tip_chain {
        let since_still = time_since_last_pointer_movement.min(time_since_last_scroll);
        let wait = TIP_CHAIN_DWELL_SECS - since_still;
        if wait > 0.0 {
            ctx.request_repaint_after_secs(wait);
            return false;
        }
        return true;
    }

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

fn end_open_if_host(ctx: &Context, host: Id) {
    ctx.data_mut(|d| {
        if d.get_temp::<Id>(Id::unique(OPEN_TIP_HOST)) == Some(host) {
            d.insert_temp(Id::unique(OPEN_TIP_HOST), Id::NULL);
        }
    });
}

/// A one- or few-line tip's frame pad.
pub const TIP_MARGIN: Margin = Margin::symmetric(Space::Sm.pts() as i8, Space::Xs.pts() as i8);

fn show_anchored(ctx: &Context, resp: &Response, max_w: f32, add_contents: impl FnOnce(&mut Ui)) {
    show_anchored_at(ctx, resp.id, resp.rect, max_w, TIP_MARGIN, add_contents);
}

fn show_anchored_at(
    ctx: &Context, host_id: Id, host: egui::Rect, max_w: f32, margin: Margin,
    add_contents: impl FnOnce(&mut Ui),
) {
    show_area(ctx, host_id, host, max_w, margin, false, add_contents);
}

/// `reachable`: the card takes the pointer, above the page but under menus.
fn show_area(
    ctx: &Context, host_id: Id, host: egui::Rect, max_w: f32, margin: Margin, reachable: bool,
    add_contents: impl FnOnce(&mut Ui),
) {
    let t = ctx.get_lb_theme();
    let screen = ctx.content_rect();
    let gap = Space::Xs.pts();
    let id = host_id.with("design_tip");
    // measured by the area's sizing pass before it's first seen
    let height = ctx.memory(|m| m.area_rect(id)).map_or(0.0, |r| r.height());
    let (below, above) = (screen.bottom() - host.bottom() - gap, host.top() - screen.top() - gap);
    // below if it fits, else above if it fits, else the roomier side
    let side = if height <= below || height > above && below >= above {
        Rect::from_min_max(Pos2::new(screen.left(), host.bottom() + gap), screen.max)
    } else {
        Rect::from_min_max(screen.min, Pos2::new(screen.right(), host.top() - gap))
    };
    let (pos, pivot) = if side.top() > host.top() {
        (Pos2::new(host.center().x, side.top()), Align2::CENTER_TOP)
    } else {
        (Pos2::new(host.center().x, side.bottom()), Align2::CENTER_BOTTOM)
    };

    Area::new(id)
        .order(if reachable { Order::Foreground } else { Order::Tooltip })
        .fixed_pos(pos)
        .pivot(pivot)
        // a card too tall for its side runs off the screen's edge, never over its host
        .constrain_to(side)
        .interactable(reachable)
        .default_size(vec2(0.0, 0.0))
        .fade_in(false)
        .show(ctx, |ui| {
            Frame::new()
                .fill(t.neutral_bg())
                .shadow(crate::style::overlay_shadow(&t))
                .corner_radius(Radius::Control.corner())
                .inner_margin(margin)
                .show(ui, |ui| {
                    ui.set_max_width(max_w);
                    add_contents(ui);
                });
        });
}

#[cfg(test)]
mod tests {
    use egui::{Event, RawInput, Sense, pos2};

    use super::*;
    use crate::theme::palette_v2::{Mode, Theme};

    /// A card taller than the room on either side of its host stays open on
    /// the roomier side, never over the host, while the pointer rests there.
    #[test]
    fn tall_card_stays_off_its_host() {
        let ctx = Context::default();
        ctx.set_lb_theme(Theme::default(Mode::Light));
        let screen = Rect::from_min_size(Pos2::ZERO, vec2(400.0, 400.0));
        let host = Rect::from_min_size(pos2(150.0, 280.0), vec2(100.0, 20.0));
        let card = Id::unique("host").with("design_tip");
        let mut shown = vec![];
        for frame in 0..40 {
            let events = vec![Event::PointerMoved(host.center())];
            let input = RawInput {
                screen_rect: Some(screen),
                time: Some(frame as f64 * 0.1),
                events: if frame == 0 { events } else { vec![] },
                ..Default::default()
            };
            let mut drawn = false;
            let mut out = ctx.run_ui(input, |ui| {
                let resp = ui.interact(host, Id::unique("host"), Sense::hover());
                tip_card_reachable(ui.ctx(), &resp, host, 200.0, TIP_MARGIN, |ui| {
                    ui.allocate_space(vec2(200.0, 330.0));
                    drawn = true;
                });
            });
            out.textures_delta.clear();
            shown.push(drawn.then(|| ctx.memory(|m| m.area_rect(card)).unwrap()));
        }
        let settled = &shown[20..];
        assert!(
            settled
                .iter()
                .all(|c| c.is_some_and(|c| !c.intersects(host))),
            "{shown:?}"
        );
    }
}
