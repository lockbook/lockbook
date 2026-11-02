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
//! Anchored to the host control; a card the pointer can reach opens beside
//! where the pointer rested instead. Canvas plate + hairline.

use egui::{Align2, Area, Context, Frame, Id, Margin, Order, Pos2, Rect, Response, Ui, Vec2, vec2};

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

/// A card tip the pointer can move onto to use what's on it, opened beside
/// where the pointer rested so it takes the wheel without standing where
/// the page scrolls. It stays while the pointer is on the card, inside the
/// triangle from where it left the host to the card's near edge, or in a
/// menu the card opened.
pub fn tip_card_reachable(
    ctx: &Context, hit: &Response, place: egui::Rect, max_w: f32, margin: Margin,
    add_contents: impl FnOnce(&mut Ui, f32),
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
    // a scroll on the way to the card puts it away; one on the card scrolls it
    let scrolling = ctx.input(|i| i.smooth_scroll_delta != Vec2::ZERO);
    let held = open
        && !hit.contains_pointer()
        && (crate::style::context_menu::is_any_open(ctx)
            || matches!((pointer, card), (Some(p), Some(c)) if c.contains(p)
                || !scrolling && exit.is_some_and(|e| in_reach(p, e, c))));
    if held {
        let frame = ctx.cumulative_frame_nr();
        ctx.data_mut(|d| d.insert_temp(Id::unique(REACH_HELD), (hit.id, frame)));
    } else if !should_show_tip(ctx, hit) {
        return;
    }
    // where the pointer rested when the card opened, kept while it's open
    let anchor_id = id.with("anchor");
    if !open {
        ctx.data_mut(|d| d.insert_temp(anchor_id, pointer.unwrap_or(place.center())));
    }
    let anchor = ctx
        .data(|d| d.get_temp::<Pos2>(anchor_id))
        .unwrap_or(place.center());
    note_tip_shown(ctx, hit.id);
    // narrower than `max_w` where neither side of the pointer holds it
    let screen = ctx.content_rect();
    let side = (screen.right() - anchor.x).max(anchor.x - screen.left());
    let width = max_w.min(side - Space::Xs.pts() - margin.sum().x).max(0.0);
    show_area(ctx, hit.id, place, width, margin, Some(anchor), |ui| add_contents(ui, width));
}

/// Puts away whatever tip is open, as a click does.
pub fn dismiss_tip(ctx: &Context) {
    chill_tip_session(ctx);
}

/// Within the triangle from `exit` to the edge of `card` that faces it.
fn in_reach(p: Pos2, exit: Pos2, card: egui::Rect) -> bool {
    let (a, b) = if exit.x <= card.left() {
        (card.left_top(), card.left_bottom())
    } else if exit.x >= card.right() {
        (card.right_top(), card.right_bottom())
    } else if exit.y <= card.top() {
        (card.left_top(), card.right_top())
    } else {
        (card.left_bottom(), card.right_bottom())
    };
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
    show_area(ctx, host_id, host, max_w, margin, None, add_contents);
}

/// `beside`: the card takes the pointer, above the page but under menus,
/// and opens to a side of that point.
fn show_area(
    ctx: &Context, host_id: Id, host: egui::Rect, max_w: f32, margin: Margin, beside: Option<Pos2>,
    add_contents: impl FnOnce(&mut Ui),
) {
    let t = ctx.get_lb_theme();
    let screen = ctx.content_rect();
    let gap = Space::Xs.pts();
    let id = host_id.with("design_tip");
    // measured by the area's sizing pass before it's first seen
    let size = ctx
        .memory(|m| m.area_rect(id))
        .map_or(vec2(max_w + margin.sum().x, 0.0), |r| r.size());
    let (side, pos, pivot) = match beside {
        Some(at) => {
            let (right, left) = (screen.right() - at.x - gap, at.x - screen.left() - gap);
            // right if it fits, else left if it fits, else the roomier side
            let y = (at.y - size.y / 2.0)
                .clamp(screen.top(), (screen.bottom() - size.y).max(screen.top()));
            if size.x <= right || size.x > left && right >= left {
                let side = Rect::from_min_max(Pos2::new(at.x + gap, screen.top()), screen.max);
                (side, Pos2::new(side.left(), y), Align2::LEFT_TOP)
            } else {
                let side = Rect::from_min_max(screen.min, Pos2::new(at.x - gap, screen.bottom()));
                (side, Pos2::new(side.right(), y), Align2::RIGHT_TOP)
            }
        }
        None => {
            let (below, above) =
                (screen.bottom() - host.bottom() - gap, host.top() - screen.top() - gap);
            // below if it fits, else above if it fits, else the roomier side
            if size.y <= below || size.y > above && below >= above {
                let side =
                    Rect::from_min_max(Pos2::new(screen.left(), host.bottom() + gap), screen.max);
                (side, Pos2::new(host.center().x, side.top()), Align2::CENTER_TOP)
            } else {
                let side =
                    Rect::from_min_max(screen.min, Pos2::new(screen.right(), host.top() - gap));
                (side, Pos2::new(host.center().x, side.bottom()), Align2::CENTER_BOTTOM)
            }
        }
    };

    Area::new(id)
        .order(if beside.is_some() { Order::Foreground } else { Order::Tooltip })
        .fixed_pos(pos)
        .pivot(pivot)
        // a card too big for its side runs off the screen's edge, never over its host or pointer
        .constrain_to(side)
        .interactable(beside.is_some())
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

    /// A card opens beside the pointer, never under it, and stays open
    /// while the wheel scrolls it with the pointer on it.
    #[test]
    fn card_opens_beside_the_pointer_and_takes_the_wheel() {
        let ctx = Context::default();
        ctx.set_lb_theme(Theme::default(Mode::Light));
        let screen = Rect::from_min_size(Pos2::ZERO, vec2(400.0, 400.0));
        let host = Rect::from_min_size(pos2(150.0, 280.0), vec2(100.0, 20.0));
        let card = Id::unique("host").with("design_tip");
        let mut shown = vec![];
        for frame in 0..60 {
            let mut events = vec![];
            if frame == 0 {
                events.push(Event::PointerMoved(host.center()));
            }
            // onto the card once it's open, then scroll there
            let open = shown.last().copied().flatten();
            if frame == 30 {
                let card: Rect = open.unwrap();
                events.push(Event::PointerMoved(card.center()));
            }
            if frame > 30 {
                events.push(Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: vec2(0.0, -20.0),
                    modifiers: Default::default(),
                    phase: egui::TouchPhase::Move,
                    source: egui::MouseWheelSource::Unknown,
                });
            }
            let input = RawInput {
                screen_rect: Some(screen),
                time: Some(frame as f64 * 0.1),
                events,
                ..Default::default()
            };
            let mut drawn = false;
            let mut out = ctx.run_ui(input, |ui| {
                let resp = ui.interact(host, Id::unique("host"), Sense::hover());
                tip_card_reachable(ui.ctx(), &resp, host, 200.0, TIP_MARGIN, |ui, _| {
                    ui.allocate_space(vec2(200.0, 330.0));
                    drawn = true;
                });
            });
            out.textures_delta.clear();
            shown.push(drawn.then(|| ctx.memory(|m| m.area_rect(card)).unwrap()));
        }
        assert!(
            shown[20..30]
                .iter()
                .all(|c| c.is_some_and(|c| !c.contains(host.center()))),
            "{shown:?}"
        );
        assert!(shown[31..].iter().all(Option::is_some), "{shown:?}");
    }
    /// A card wider than either side of the pointer narrows to stay on the
    /// screen beside it.
    #[test]
    fn card_narrows_to_fit_beside_the_pointer() {
        let ctx = Context::default();
        ctx.set_lb_theme(Theme::default(Mode::Light));
        let screen = Rect::from_min_size(Pos2::ZERO, vec2(300.0, 400.0));
        let host = Rect::from_min_size(pos2(100.0, 180.0), vec2(100.0, 20.0));
        let card = Id::unique("host").with("design_tip");
        let mut last = None;
        for frame in 0..30 {
            let events = if frame == 0 { vec![Event::PointerMoved(host.center())] } else { vec![] };
            let input = RawInput {
                screen_rect: Some(screen),
                time: Some(frame as f64 * 0.1),
                events,
                ..Default::default()
            };
            let mut out = ctx.run_ui(input, |ui| {
                let resp = ui.interact(host, Id::unique("host"), Sense::hover());
                tip_card_reachable(ui.ctx(), &resp, host, 200.0, TIP_MARGIN, |ui, width| {
                    ui.allocate_space(vec2(width, 100.0));
                });
            });
            out.textures_delta.clear();
            last = ctx.memory(|m| m.area_rect(card));
        }
        let card = last.unwrap();
        assert!(screen.contains_rect(card) && !card.contains(host.center()), "{card:?}");
    }
}
