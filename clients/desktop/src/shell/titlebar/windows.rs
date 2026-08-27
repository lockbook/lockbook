//! Win11 caption cells (min · max/restore · close).

use egui::{Align, Area, Id, Layout, Order, Rect, Sense, Ui, pos2, vec2};

use crate::components::{FG_HOVER, STROKE_HAIRLINE, Theme, claim, phosphor, place_at};

use super::block_window_drag;
use super::metrics::{CAPTION_W, HEADER_H, caption_cluster_w};

pub fn window_controls(ctx: &egui::Context, t: &Theme) {
    use egui::{Align2, ViewportCommand};
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
            place_caption_row(ui, |ui, i, slot| {
                let _ = place_at(ui, slot, Layout::top_down(Align::Min), |ui| match i {
                    0 => {
                        if window_button(ui, t, phosphor::MINUS, false, ground).clicked() {
                            ui.ctx().send_viewport_cmd(ViewportCommand::Minimized(true));
                        }
                    }
                    1 => {
                        let maximized = ui.input(|i| i.viewport().maximized.unwrap_or(false));
                        let max_icon = if maximized { phosphor::COPY } else { phosphor::SQUARE };
                        if window_button(ui, t, max_icon, false, ground).clicked() {
                            ui.ctx()
                                .send_viewport_cmd(ViewportCommand::Maximized(!maximized));
                        }
                    }
                    _ => {
                        if window_button(ui, t, phosphor::X, true, ground).clicked() {
                            ui.ctx().send_viewport_cmd(ViewportCommand::Close);
                        }
                    }
                });
            });
        });
}

fn place_caption_row(ui: &mut Ui, mut paint_at: impl FnMut(&mut Ui, usize, Rect)) {
    let origin = ui.cursor().min;
    let mut x = origin.x;
    for i in 0..3 {
        let slot = Rect::from_min_size(pos2(x, origin.y), vec2(CAPTION_W, HEADER_H));
        paint_at(ui, i, slot);
        x += CAPTION_W;
    }
    claim(ui, Rect::from_min_size(origin, vec2(caption_cluster_w(), HEADER_H)));
}

/// Full [`HEADER_H`] hit, square hover. Close goes red. Fill stops one
/// hairline short of the bottom so the titleband→workspace edge stays visible.
fn window_button(
    ui: &mut Ui, t: &Theme, icon: &'static str, danger: bool, ground: egui::Color32,
) -> egui::Response {
    use crate::components::foundation::chrome::HOVER_ANIM_SECS;
    use crate::components::phosphor_ui_font_id;
    let (rect, resp) = ui.allocate_exact_size(vec2(CAPTION_W, HEADER_H), Sense::click());
    let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    let over = resp.hovered() || ui.ctx().rect_contains_pointer(ui.layer_id(), rect);
    let hover = ui
        .ctx()
        .animate_bool_with_time(resp.id.with("win_hov"), over, HOVER_ANIM_SECS);
    if hover > 0.0 {
        let wash = if danger {
            ground.lerp_to_gamma(t.danger(), hover)
        } else {
            t.wash_toward_neutral_fg(ground, FG_HOVER * hover)
        };
        let fill =
            Rect::from_min_max(rect.min, pos2(rect.right(), rect.bottom() - STROKE_HAIRLINE));
        ui.painter().rect_filled(fill, 0.0, wash);
    }
    let color = if danger {
        t.neutral_fg_secondary()
            .lerp_to_gamma(egui::Color32::WHITE, hover)
    } else {
        t.neutral_fg_secondary()
            .lerp_to_gamma(t.neutral_fg(), hover)
    };
    let g = ui
        .painter()
        .layout_no_wrap(icon.into(), phosphor_ui_font_id(), color);
    ui.painter()
        .galley(rect.center() - g.size() / 2.0, g, color);
    resp
}
