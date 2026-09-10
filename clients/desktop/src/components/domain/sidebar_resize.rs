//! SidePanel resize grab vs overlay scrollbar policy.
//!
//! egui store width from content; resize chrome and the floating bar fight over
//! the right edge. These helpers codify grab size, latch, and content-side half.

use crate::components::compounds::scroll::SIDEBAR_RESIZING_LATCH;
use crate::components::foundation::{STROKE_HAIRLINE, SurfaceMotion, Theme, surface_motion};

/// Must match [`egui::SidePanel::left`] id in the shell.
pub const PANEL_ID: &str = "shell_sidebar";
/// Keep in sync with SidePanel `.default_width` in the shell.
pub const WIDTH_DEFAULT: f32 = 300.0;
/// Sidebar min width = titleband controls' right edge, so a min-width split
/// lines up with parked tabs (open vs closed, tabs do not move).
pub fn width_min() -> f32 {
    crate::shell::titlebar::controls_right()
}

/// `max(min, half the window)`. Never below min, even on a narrow window.
pub fn width_max(ctx: &egui::Context) -> f32 {
    width_min().max(ctx.screen_rect().width() * 0.5)
}
/// Into each side of the edge (default egui is 5). Smaller = less fight with the bar.
pub const RESIZE_GRAB: f32 = 3.0;

/// Open/close t (0 closed, 1 open). Stable id — not [`egui::Id::with`] (call-site unique).
pub fn animation_id() -> egui::Id {
    egui::Id::new((PANEL_ID, "animation"))
}

/// 0 = closed, 1 = open. First call at a target snaps (no launch animation).
pub fn open_motion(ctx: &egui::Context, open: bool) -> SurfaceMotion {
    let motion = surface_motion(ctx, animation_id(), open);
    if motion.slide > 0.0 && motion.slide < 1.0 {
        tracing::debug!(
            target: "lockbook_desktop::sidebar",
            open,
            slide = motion.slide,
            "sidebar motion"
        );
    }
    motion
}

pub fn split_stroke(t: &Theme) -> egui::Stroke {
    egui::Stroke::new(STROKE_HAIRLINE, t.neutral())
}

fn resting_width_id() -> egui::Id {
    egui::Id::new((PANEL_ID, "resting_width"))
}

fn panel_id() -> egui::Id {
    egui::Id::new(PANEL_ID)
}

/// User's sidebar width. Independent of the slide so `exact_width` during
/// the wipe does not persist a shrinking panel.
pub fn resting_width(ctx: &egui::Context) -> f32 {
    ctx.data_mut(|d| d.get_persisted::<f32>(resting_width_id()))
        .unwrap_or(WIDTH_DEFAULT)
        .clamp(width_min(), width_max(ctx))
}

pub fn remember_resting_width(ctx: &egui::Context, w: f32) {
    let w = w.clamp(width_min(), width_max(ctx));
    ctx.data_mut(|d| d.insert_persisted(resting_width_id(), w));
}

/// Force the live panel rect to `w` so `exact_width` cannot lose to a stale
/// [`PanelState`] from the last rest.
pub fn set_animating_width(ctx: &egui::Context, w: f32) {
    let id = panel_id();
    let w = w.max(1.0);
    let Some(mut state) = egui::containers::panel::PanelState::load(ctx, id) else {
        return;
    };
    state.rect.set_right(state.rect.left() + w);
    ctx.data_mut(|d| d.insert_persisted(id, state));
}

/// After a close/open slide, [`PANEL_ID`] may still be 1px wide. Restore the
/// last resting width. No-op when already in range (live resize).
pub fn restore_panel_width_if_collapsed(ctx: &egui::Context) {
    let id = panel_id();
    let Some(mut state) = egui::containers::panel::PanelState::load(ctx, id) else {
        return;
    };
    if state.rect.width() >= width_min() - 0.5 {
        return;
    }
    let w = resting_width(ctx);
    state.rect.set_right(state.rect.left() + w);
    ctx.data_mut(|d| d.insert_persisted(id, state));
}

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

fn resize_cursor(width: f32, max: f32) -> egui::CursorIcon {
    if width <= width_min() + 0.5 {
        egui::CursorIcon::ResizeEast
    } else if width >= max - 0.5 {
        egui::CursorIcon::ResizeWest
    } else {
        egui::CursorIcon::ResizeHorizontal
    }
}

/// Resize handle straddling the split. SidePanel's built-in grab is off.
pub fn resize_over_workspace(ctx: &egui::Context, header_h: f32, stroke: egui::Stroke) {
    let panel_id = panel_id();
    let Some(state) = egui::containers::panel::PanelState::load(ctx, panel_id) else {
        return;
    };

    let edge_x = state.rect.right();
    let grab_w = RESIZE_GRAB.max(1.0);
    let rect = egui::Rect::from_min_max(
        egui::pos2(edge_x - grab_w, state.rect.top()),
        egui::pos2(edge_x + grab_w, state.rect.bottom()),
    );
    let line_top = (state.rect.top() + header_h.max(0.0)).min(state.rect.bottom());
    let line_y = egui::Rangef::new(line_top, state.rect.bottom());
    let max_w = width_max(ctx);
    let cursor = resize_cursor(state.rect.width(), max_w);
    let drag_id = resize_drag_ids().1;
    let diag = std::env::var_os("LOCKBOOK_RESIZE_DIAG").is_some();

    egui::Area::new(resize_area_id())
        .order(egui::Order::Middle)
        .fixed_pos(rect.min)
        .default_size(rect.size())
        .sense(egui::Sense::click_and_drag())
        .show(ctx, |ui| {
            ui.set_min_size(rect.size());
            ui.set_max_size(rect.size());
            let resp = ui
                .interact(ui.max_rect(), drag_id, egui::Sense::click_and_drag())
                .on_hover_cursor(cursor);
            if resp.hovered() || resp.dragged() {
                ui.ctx().set_cursor_icon(cursor);
            }

            let line_x = edge_x + STROKE_HAIRLINE * 0.5;
            ui.painter().vline(line_x, line_y, stroke);

            if resp.dragged() {
                if let Some(p) = resp.interact_pointer_pos() {
                    let new_w = (p.x - state.rect.left()).clamp(width_min(), max_w);
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
                    "[resize] hover={} dragged={} w={:.1}",
                    resp.hovered(),
                    resp.dragged(),
                    state.rect.width(),
                );
            }
        });
    sync_resizing_latch(ctx);
}

/// Split hairline only (no resize hit). Used while the sidebar is sliding.
pub fn paint_split_line(ctx: &egui::Context, header_h: f32, stroke: egui::Stroke) {
    let Some(state) = egui::containers::panel::PanelState::load(ctx, panel_id()) else {
        return;
    };
    paint_vline(ctx, state.rect.right(), state.rect.top(), state.rect.bottom(), header_h, stroke);
}

fn paint_vline(
    ctx: &egui::Context, edge_x: f32, top: f32, bottom: f32, header_h: f32, stroke: egui::Stroke,
) {
    let line_top = (top + header_h.max(0.0)).min(bottom);
    if line_top >= bottom - 0.5 {
        return;
    }
    let line_x = edge_x + STROKE_HAIRLINE * 0.5;
    ctx.layer_painter(egui::LayerId::new(egui::Order::Middle, resize_area_id()))
        .vline(line_x, egui::Rangef::new(line_top, bottom), stroke);
}
