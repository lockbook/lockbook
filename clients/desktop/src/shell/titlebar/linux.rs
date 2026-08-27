//! Linux CSD captions: compact hit, circular hover.

use egui::{Align, Area, Id, Layout, Order, Rect, Sense, Ui, pos2, vec2};

use crate::components::{FG_HOVER, Theme, claim, phosphor, phosphor_font_id, place_at};

use super::block_window_drag;
use super::metrics::{
    HEADER_H, LINUX_GAP, LINUX_GLYPH, LINUX_HIT_W, LINUX_MIN_SHIFT, LINUX_WASH, caption_cluster_w,
};

pub fn window_controls(ctx: &egui::Context, t: &Theme) {
    use egui::Align2;
    let controls_w = caption_cluster_w();
    let screen = ctx.screen_rect();
    block_window_drag(
        ctx,
        Rect::from_min_size(
            pos2(screen.right() - controls_w, screen.top()),
            vec2(controls_w, HEADER_H),
        ),
    );
    Area::new(Id::new("shell_window_controls"))
        .order(Order::Foreground)
        .anchor(Align2::RIGHT_TOP, vec2(0.0, 0.0))
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            let ground = t.neutral_bg();
            let origin = ui.cursor().min;
            let mut x = origin.x;
            let maximized = ui.input(|i| i.viewport().maximized.unwrap_or(false));
            let icons = [
                phosphor::MINUS,
                if maximized { phosphor::ARROWS_IN_SIMPLE } else { phosphor::ARROWS_OUT_SIMPLE },
                phosphor::X,
            ];
            for (i, icon) in icons.into_iter().enumerate() {
                if i > 0 {
                    x += LINUX_GAP;
                }
                let slot = Rect::from_min_size(pos2(x, origin.y), vec2(LINUX_HIT_W, HEADER_H));
                let _ = place_at(ui, slot, Layout::top_down(Align::Min), |ui| {
                    linux_window_button(ui, t, icon, i == 0, i == 2, ground);
                });
                x += LINUX_HIT_W;
            }
            claim(ui, Rect::from_min_size(origin, vec2(caption_cluster_w(), HEADER_H)));
        });
}

fn linux_window_button(
    ui: &mut Ui, t: &Theme, icon: &'static str, minimize: bool, close: bool, ground: egui::Color32,
) -> egui::Response {
    use crate::components::foundation::chrome::HOVER_ANIM_SECS;
    let (rect, resp) = ui.allocate_exact_size(vec2(LINUX_HIT_W, HEADER_H), Sense::click());
    let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    if resp.clicked() {
        use egui::ViewportCommand;
        if close {
            ui.ctx().send_viewport_cmd(ViewportCommand::Close);
        } else if minimize {
            ui.ctx().send_viewport_cmd(ViewportCommand::Minimized(true));
        } else {
            let maximized = ui.input(|i| i.viewport().maximized.unwrap_or(false));
            ui.ctx()
                .send_viewport_cmd(ViewportCommand::Maximized(!maximized));
        }
    }
    let over = resp.hovered() || ui.ctx().rect_contains_pointer(ui.layer_id(), rect);
    let hover = ui
        .ctx()
        .animate_bool_with_time(resp.id.with("win_hov"), over, HOVER_ANIM_SECS);
    if hover > 0.0 {
        let wash = t.wash_toward_neutral_fg(ground, FG_HOVER * hover);
        ui.painter()
            .circle_filled(rect.center(), LINUX_WASH * 0.5, wash);
    }
    let color = if close {
        t.neutral_fg_secondary().lerp_to_gamma(t.danger(), hover)
    } else {
        t.neutral_fg_secondary()
            .lerp_to_gamma(t.neutral_fg(), hover)
    };
    let g = ui
        .painter()
        .layout_no_wrap(icon.into(), phosphor_font_id(LINUX_GLYPH), color);
    let mut pos = rect.center() - g.size() / 2.0;
    if minimize {
        pos.y += LINUX_MIN_SHIFT;
    }
    ui.painter().galley(pos, g, color);
    resp
}
