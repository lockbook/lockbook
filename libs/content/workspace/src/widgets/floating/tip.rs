//! Hover tips — custom floating cards instead of stock `on_hover_text`.
//!
//! Tips are **anchored to the hovered control** (not the cursor), so they stay
//! put while the pointer moves within the host rect.
//!
//! Timing matches stock egui tooltips:
//! - First tip: wait `tooltip_delay` after the pointer is still
//! - Once a tip is up, moving to another tip host shows the next tip **immediately**
//!   (grace + “already open” host), same as `Response::on_hover_text`

use egui::{Align2, Area, Id, Order, Pos2, Rect, Response, RichText, Sense, Ui, Vec2, vec2};

use super::FloatingChrome;
use crate::theme::palette_v2::ThemeExt;

/// Max content width for compact one-line tips.
const MAX_W: f32 = 280.0;
/// Rich multi-line tips (file tree, etc.).
const RICH_MAX_W: f32 = 360.0;
/// Gap between the host control and the tip.
const GAP: f32 = 6.0;

/// Last wall time any lb tip was painted (chaining / grace).
const LAST_TIP_TIME: &str = "lb_float_tip_last_shown";
/// Host `Response::id` currently (or last) showing a tip — keep open without delay.
const OPEN_TIP_HOST: &str = "lb_float_tip_open_host";

/// Where the tip pins relative to the host rect.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TipPlacement {
    /// Center under/above the host (default for chrome / buttons).
    #[default]
    Center,
    /// Leading edge under/above the host (lists / file tree — not center-anchored).
    Leading,
}

/// Show a short text tip while `resp` is hovered (replaces `on_hover_text`).
pub fn tip_text(ctx: &egui::Context, resp: &Response, text: impl Into<String>) {
    let text = text.into();
    if text.is_empty() {
        return;
    }
    tip_ui(ctx, resp, |ui| {
        ui.set_max_width(MAX_W);
        let ink = ui.ctx().get_lb_theme().neutral_fg();
        ui.label(RichText::new(text).size(12.5).color(ink));
    });
}

/// Show custom tip content while `resp` is hovered (replaces `on_hover_ui`).
pub fn tip_ui(ctx: &egui::Context, resp: &Response, add_contents: impl FnOnce(&mut Ui)) {
    tip_ui_ex(ctx, resp, TipPlacement::Center, MAX_W, add_contents);
}

/// Rich tip: wider card, leading-aligned to the host (for tree / list rows).
pub fn tip_ui_rich(ctx: &egui::Context, resp: &Response, add_contents: impl FnOnce(&mut Ui)) {
    tip_ui_ex(ctx, resp, TipPlacement::Leading, RICH_MAX_W, add_contents);
}

fn tip_ui_ex(
    ctx: &egui::Context, resp: &Response, placement: TipPlacement, max_w: f32,
    add_contents: impl FnOnce(&mut Ui),
) {
    if !should_show_tip(ctx, resp) {
        // Leaving a host clears the open-host marker only when nothing else
        // claims it this frame; see `note_tip_shown`.
        return;
    }
    note_tip_shown(ctx, resp.id);
    show_anchored(
        ctx,
        resp.id.with("lb_float_tip"),
        resp.rect,
        placement,
        max_w,
        add_contents,
    );
}

fn note_tip_shown(ctx: &egui::Context, host: Id) {
    let now = ctx.input(|i| i.time);
    ctx.data_mut(|d| {
        // Mut access keeps temp entries alive across frames (unlike a pure read
        // gap when the pointer crosses dead space between rows).
        d.insert_temp(Id::new(LAST_TIP_TIME), now);
        d.insert_temp(Id::new(OPEN_TIP_HOST), host);
    });
}

/// egui-aligned delay + chaining: first tip waits for still/delay; once any tip
/// is live, other hosts open immediately while you move between them.
fn should_show_tip(ctx: &egui::Context, resp: &Response) -> bool {
    if !resp.hovered() || ctx.dragged_id().is_some() {
        return false;
    }
    if !ctx.input(|i| i.pointer.has_pointer()) {
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
    ) = ctx.input(|i| {
        (
            i.time_since_last_scroll(),
            i.pointer.time_since_last_click(),
            i.pointer.time_since_last_movement(),
            i.pointer.is_still(),
            i.smooth_scroll_delta,
            i.time,
        )
    });

    // Don't flash tips while scrolling the list.
    if time_since_last_scroll < tooltip_delay {
        ctx.request_repaint_after_secs(tooltip_delay - time_since_last_scroll);
        return false;
    }

    // Click-then-rest shouldn't immediately open a tip.
    let clicked_more_recently_than_moved =
        time_since_last_click < time_since_last_pointer_movement + 0.1;
    if clicked_more_recently_than_moved {
        return false;
    }

    // Touch clock so temp data doesn't expire between adjacent hosts.
    let (last_shown, open_host) = ctx.data_mut(|d| {
        let last = *d
            .get_temp_mut_or_insert_with(Id::new(LAST_TIP_TIME), || f64::NEG_INFINITY);
        let host = d.get_temp::<Id>(Id::new(OPEN_TIP_HOST));
        (last, host)
    });

    let is_our_tip_open = open_host == Some(resp.id);
    let seconds_since_last_tip = now - last_shown;
    // egui: after any tip, a short grace lets the next host open immediately —
    // including while the pointer is still moving.
    let tip_was_recently_shown = seconds_since_last_tip < tooltip_grace_time as f64;
    // Also chain while actively moving across tip hosts shortly after a tip
    // (tree rows): keep “tooltip mode” for a bit longer than stock grace so a
    // gap frame between rows doesn’t force another still+delay wait.
    let in_tip_chain = seconds_since_last_tip < (tooltip_delay as f64).max(0.5);

    if is_our_tip_open {
        // Same host still hovered — stay open without re-waiting.
        return true;
    }

    if tip_was_recently_shown || in_tip_chain {
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

/// Place a floating tip relative to a host rect.
fn show_anchored(
    ctx: &egui::Context, id: Id, host: Rect, placement: TipPlacement, max_w: f32,
    add_contents: impl FnOnce(&mut Ui),
) {
    let chrome = FloatingChrome::from_ctx(ctx);
    let screen = ctx.screen_rect();
    let space_below = screen.bottom() - host.bottom();
    let prefer_below = space_below >= 72.0;

    let (pos, pivot) = match (placement, prefer_below) {
        (TipPlacement::Center, true) => {
            (Pos2::new(host.center().x, host.bottom() + GAP), Align2::CENTER_TOP)
        }
        (TipPlacement::Center, false) => {
            (Pos2::new(host.center().x, host.top() - GAP), Align2::CENTER_BOTTOM)
        }
        (TipPlacement::Leading, true) => {
            (Pos2::new(host.left(), host.bottom() + GAP), Align2::LEFT_TOP)
        }
        (TipPlacement::Leading, false) => {
            (Pos2::new(host.left(), host.top() - GAP), Align2::LEFT_BOTTOM)
        }
    };

    Area::new(id)
        .order(Order::Tooltip)
        .fixed_pos(pos)
        .pivot(pivot)
        .constrain(true)
        .sense(Sense::hover())
        .default_size(vec2(0.0, 0.0))
        .fade_in(false)
        .show(ctx, |ui| {
            chrome
                .frame_margin(egui::Margin::symmetric(14, 12))
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 4.0;
                    ui.set_max_width(max_w);
                    add_contents(ui);
                });
        });
}

/// Multi-line tip for status-style content (tab hover, etc.).
pub fn tip_lines(ctx: &egui::Context, resp: &Response, title: &str, detail: &str) {
    tip_ui(ctx, resp, |ui| {
        ui.set_max_width(MAX_W);
        let theme = ui.ctx().get_lb_theme();
        if !title.is_empty() {
            ui.label(RichText::new(title).size(13.0).strong().color(theme.neutral_fg()));
        }
        if !detail.is_empty() {
            ui.label(
                RichText::new(detail)
                    .size(12.0)
                    .color(theme.neutral_fg_secondary()),
            );
        }
    });
}
