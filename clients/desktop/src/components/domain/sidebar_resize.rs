//! SidePanel resize grab vs overlay scrollbar policy.
//!
//! egui store width from content; resize chrome and the floating bar fight over
//! the right edge. These helpers codify grab size, latch, and content-side half.

use crate::components::compounds::scroll::SIDEBAR_RESIZING_LATCH;
use crate::components::foundation::{STROKE_HAIRLINE, Theme};

/// Must match [`egui::SidePanel::left`] id in the shell.
pub const PANEL_ID: &str = "shell_sidebar";
/// Keep in sync with SidePanel `.width_range` in the shell.
pub const WIDTH_MIN: f32 = 268.0;
pub const WIDTH_MAX: f32 = 500.0;
/// Into each side of the edge (default egui is 5). Smaller = less fight with the bar.
pub const RESIZE_GRAB: f32 = 3.0;

fn resize_drag_ids() -> (egui::Id, egui::Id) {
    (
        egui::Id::new(PANEL_ID).with("__resize"),
        // Must not equal Area id (widget + Area same id → bad drag/hover).
        egui::Id::new("shell_sidebar_resize_content").with("drag"),
    )
}

fn resize_area_id() -> egui::Id {
    egui::Id::new("shell_sidebar_resize_content")
}

/// Soften resize chrome + shrink grab for the SidePanel paint window.
pub fn begin_resize_style(ctx: &egui::Context, t: &Theme) -> (egui::Stroke, egui::Stroke, f32) {
    let resize_stroke = egui::Stroke::new(STROKE_HAIRLINE, t.neutral());
    let saved = {
        let s = ctx.style();
        (
            s.visuals.widgets.hovered.fg_stroke,
            s.visuals.widgets.active.fg_stroke,
            s.interaction.resize_grab_radius_side,
        )
    };
    ctx.style_mut(|s| {
        s.visuals.widgets.hovered.fg_stroke = resize_stroke;
        s.visuals.widgets.active.fg_stroke = resize_stroke;
        s.interaction.resize_grab_radius_side = RESIZE_GRAB;
    });
    sync_resizing_latch(ctx);
    saved
}

pub fn end_resize_style(ctx: &egui::Context, saved: (egui::Stroke, egui::Stroke, f32)) {
    sync_resizing_latch(ctx);
    ctx.style_mut(|s| {
        s.visuals.widgets.hovered.fg_stroke = saved.0;
        s.visuals.widgets.active.fg_stroke = saved.1;
        s.interaction.resize_grab_radius_side = saved.2;
    });
}

/// Sticky "separator is resizing" until primary is released.
pub fn sync_resizing_latch(ctx: &egui::Context) {
    let latch = egui::Id::new(SIDEBAR_RESIZING_LATCH);
    if !ctx.input(|i| i.pointer.primary_down()) {
        ctx.data_mut(|d| d.insert_temp(latch, false));
        return;
    }
    let (panel_resize, content_resize) = resize_drag_ids();
    if ctx.is_being_dragged(panel_resize) || ctx.is_being_dragged(content_resize) {
        ctx.data_mut(|d| d.insert_temp(latch, true));
    }
}

/// Content-side half of the resize grab (workspace covers SidePanel’s right half).
///
/// `header_h` = shell tab strip height when tabs are open (`HEADER_H`), else **0**
/// so the hairline runs full height when workspace is flush to the top.
/// When `header_h > 0`, the line starts below the strip (sidebar head + tabs
/// read as one continuous top chrome).
pub fn resize_over_workspace(ctx: &egui::Context, t: &Theme, header_h: f32) {
    let panel_id = egui::Id::new(PANEL_ID);
    let Some(state) = egui::containers::panel::PanelState::load(ctx, panel_id) else {
        return;
    };

    let edge_x = state.rect.right();
    let h = state.rect.height().max(1.0);
    let grab_w = RESIZE_GRAB.max(1.0);
    // Grab still spans the full edge (resize near the top is fine).
    let rect =
        egui::Rect::from_min_size(egui::pos2(edge_x, state.rect.top()), egui::vec2(grab_w, h));
    // Painted separator: only under the tab/title strip.
    let line_top = (state.rect.top() + header_h.max(0.0)).min(state.rect.bottom());
    let line_y = egui::Rangef::new(line_top, state.rect.bottom());

    let drag_id = resize_drag_ids().1;
    let diag = std::env::var_os("LOCKBOOK_RESIZE_DIAG").is_some();

    egui::Area::new(resize_area_id())
        .order(egui::Order::Middle)
        .fixed_pos(rect.min)
        .default_size(rect.size())
        .sense(egui::Sense::hover())
        .show(ctx, |ui| {
            ui.set_min_size(rect.size());
            ui.set_max_size(rect.size());
            let (hit, _) = ui.allocate_exact_size(rect.size(), egui::Sense::hover());
            let resp = ui
                .interact(hit, drag_id, egui::Sense::drag())
                .on_hover_cursor(egui::CursorIcon::ResizeHorizontal);

            let line_x = edge_x + STROKE_HAIRLINE * 0.5;
            let stroke = egui::Stroke::new(STROKE_HAIRLINE, t.neutral());
            ui.painter().vline(line_x, line_y, stroke);
            // Hover uses the same segment (no second thicker line through the strip).

            let primary = ui.input(|i| i.pointer.primary_down());
            if resp.dragged() && primary {
                if let Some(p) = resp.interact_pointer_pos() {
                    let new_w = (p.x - state.rect.left()).clamp(WIDTH_MIN, WIDTH_MAX);
                    if diag {
                        eprintln!(
                            "[resize] drag edge={edge_x:.1} ptr.x={:.1} w {:.1}→{new_w:.1}",
                            p.x,
                            state.rect.width(),
                        );
                    }
                    let mut new_rect = state.rect;
                    new_rect.set_right(state.rect.left() + new_w);
                    ui.ctx().data_mut(|d| {
                        d.insert_persisted(
                            panel_id,
                            egui::containers::panel::PanelState { rect: new_rect },
                        );
                    });
                    ui.ctx().request_repaint();
                }
            } else if diag && (resp.hovered() || resp.dragged()) {
                eprintln!(
                    "[resize] hover={} dragged={} primary={primary} w={:.1}",
                    resp.hovered(),
                    resp.dragged(),
                    state.rect.width(),
                );
            }
        });
    sync_resizing_latch(ctx);
}
