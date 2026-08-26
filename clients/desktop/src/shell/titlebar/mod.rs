//! Frameless window top chrome: view toggles, tabs inset, window controls.
//!
//! | Platform | Window chrome |
//! |----------|---------------|
//! | macOS | Native traffic lights (host: fullsize content + transparent titlebar) |
//! | Windows | Full-height caption cells (min · max/restore · close) |
//! | Linux | Compact header icons, circular hover (min · max/restore · close) |
//!
//! View toggles (Files / Recents / Shared) stay top-left and stay visible with
//! the sidebar closed. Back / forward sit as a second group after them. Tab
//! strip insets clear this cluster (and macOS lights) — see [`tab_left_inset`]
//! / [`tab_right_inset`].

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod linux;
mod metrics;
#[cfg(target_os = "windows")]
mod windows;

pub use metrics::{
    HEADER_CENTER, HEADER_H, TOGGLE_X, controls_right, tab_drag_gap, tab_left_inset,
    tab_right_inset, toolbar_cluster_w,
};

#[cfg(not(target_os = "macos"))]
use egui::Sense;
use egui::{Area, Id, Order, Rect, pos2, vec2};

use crate::components::{
    Theme, claim, icon_button_glyph, phosphor, phosphor_font_id, place_at, tip_text,
};
use egui::{Align, Layout};

use super::ShellApp;
use super::action::Action as A;
use super::action::{Action, SidebarPane};

use metrics::{group_gap, icon_hit_size, icon_size, left_chrome_w, titlebar_glyph, toolbar_gap};

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
use linux::window_controls;
#[cfg(target_os = "windows")]
use windows::window_controls;

/// Temp-data id: rects where a primary drag must **not** move the OS window
/// (tabs, toolbar, window controls). Written during panel paint; read by
/// [`drag_strip`] after panels (shell root order).
fn window_drag_blockers_id() -> Id {
    Id::new("shell_window_drag_blockers")
}

/// Exclude `rect` from the window-move drag band for this frame.
///
/// **Rule:** any new interactive header widget (tabs, pane icons, caption
/// buttons, …) must call this before [`show`]. Empty chrome is the drag
/// region — do not block the leftover after the last tab.
pub fn block_window_drag(ctx: &egui::Context, rect: Rect) {
    if rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }
    ctx.data_mut(|d| {
        let list: &mut Vec<Rect> = d.get_temp_mut_or_default(window_drag_blockers_id());
        list.push(rect);
    });
}

fn take_window_drag_blockers(ctx: &egui::Context) -> Vec<Rect> {
    ctx.data_mut(|d| d.remove_temp::<Vec<Rect>>(window_drag_blockers_id()))
        .unwrap_or_default()
}

fn pointer_in_blockers(pos: egui::Pos2, blockers: &[Rect]) -> bool {
    blockers.iter().any(|r| r.contains(pos))
}

fn window_menu_request_id() -> Id {
    Id::new("shell_window_menu_at")
}

fn request_window_menu(ctx: &egui::Context, pos: egui::Pos2) {
    ctx.data_mut(|d| d.insert_temp(window_menu_request_id(), pos));
}

/// Position of a titleband right-click, if any this frame. Host shows the
/// system window menu via `winit::Window::show_window_menu` (no-op on macOS
/// and X11).
pub fn take_window_menu_request(ctx: &egui::Context) -> Option<egui::Pos2> {
    ctx.data_mut(|d| d.remove_temp(window_menu_request_id()))
}

#[cfg(target_os = "macos")]
fn titlebar_double_click_request_id() -> Id {
    Id::new("shell_titlebar_double_click")
}

#[cfg(target_os = "macos")]
fn request_titlebar_double_click(ctx: &egui::Context) {
    ctx.data_mut(|d| d.insert_temp(titlebar_double_click_request_id(), true));
}

/// Host: run the Mac system titlebar double-click action this frame.
#[cfg(target_os = "macos")]
pub fn take_titlebar_double_click(ctx: &egui::Context) -> bool {
    ctx.data_mut(|d| d.remove_temp::<bool>(titlebar_double_click_request_id()))
        .unwrap_or(false)
}

fn titleband_last_click_id() -> Id {
    Id::new("shell_titleband_last_click")
}

fn titleband_double_click_interval() -> f64 {
    #[cfg(target_os = "macos")]
    {
        crate::shell::macos_window::double_click_interval_secs()
    }
    #[cfg(not(target_os = "macos"))]
    {
        0.5
    }
}

/// Paint drag strip, resize edges, floating toolbar, and window controls.
/// Call once per frame from the shell root (after panels so tab blockers exist).
pub fn show(app: &mut ShellApp, ctx: &egui::Context, t: &Theme, queue: &mut Vec<Action>) {
    // Chrome first so its rects are in the blocker list before drag_strip.
    floating_toolbar(app, ctx, t, queue);

    #[cfg(not(target_os = "macos"))]
    {
        window_resize_edges(ctx);
        window_controls(ctx, t);
    }

    // macOS: fullsize + transparent titlebar still needs app-driven `StartDrag`
    // (content view owns mouseDown). Win/Linux: frameless — same path.
    // Runs last: tabs (central panel) + toolbar blockers already registered.
    drag_strip(ctx);
}

/// Full-width band of height [`HEADER_H`] (40 = `HEADER_CENTER * 2`).
///
/// Does **not** allocate a hit-target over the whole band (that would steal
/// tab clicks / tab reorder DnD). Instead: if the press originated in the
/// header, outside registered blockers, and the pointer is decidedly
/// dragging — and nothing else owns a drag / DnD payload — send
/// `ViewportCommand::StartDrag`.
///
/// On macOS, automatic titlebar-band move is disabled (`macos_window`); only
/// this explicit path moves the window.
fn drag_strip(ctx: &egui::Context) {
    use egui::{DragAndDrop, PointerButton, ViewportCommand};

    let screen = ctx.screen_rect();
    let header = Rect::from_min_size(screen.min, vec2(screen.width(), HEADER_H));
    let blockers = take_window_drag_blockers(ctx);
    let free = |pos: egui::Pos2| header.contains(pos) && !pointer_in_blockers(pos, &blockers);

    // Always clear the one-shot arm when the primary button is up.
    let armed_id = Id::new("shell_window_drag_armed");
    if !ctx.input(|i| i.pointer.primary_down()) {
        ctx.data_mut(|d| d.insert_temp(armed_id, false));
    }

    // Tab reorder / tree DnD / any payload: never steal the gesture.
    if DragAndDrop::has_any_payload(ctx) {
        return;
    }
    // A widget already owns this drag (e.g. tab `click_and_drag` mid-gesture).
    if ctx.dragged_id().is_some() {
        return;
    }

    // Right-click empty chrome → system window menu (host; no-op where unsupported).
    if ctx.input(|i| i.pointer.button_clicked(PointerButton::Secondary)) {
        if let Some(pos) = ctx.pointer_interact_pos() {
            if free(pos) {
                request_window_menu(ctx, pos);
            }
        }
    }

    // Clicks are reported on *release*, and egui clears `press_origin` that same
    // frame — so this must run before the press_origin early-return. Two clicks
    // in the OS double-click interval (not egui's 0.3s) count.
    if ctx.input(|i| i.pointer.button_clicked(PointerButton::Primary)) {
        let last_id = titleband_last_click_id();
        if let Some(pos) = ctx.input(|i| i.pointer.interact_pos().or(i.pointer.latest_pos())) {
            if !free(pos) {
                ctx.data_mut(|d| d.remove_temp::<(f64, egui::Pos2)>(last_id));
            } else {
                let time = ctx.input(|i| i.time);
                let last = ctx.data(|d| d.get_temp::<(f64, egui::Pos2)>(last_id));
                let is_double = last.is_some_and(|(t, p)| {
                    (time - t) < titleband_double_click_interval() && p.distance(pos) <= 6.0
                });
                if is_double {
                    ctx.data_mut(|d| d.remove_temp::<(f64, egui::Pos2)>(last_id));
                    #[cfg(target_os = "macos")]
                    request_titlebar_double_click(ctx);
                    #[cfg(not(target_os = "macos"))]
                    {
                        let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                        ctx.send_viewport_cmd(ViewportCommand::Maximized(!maximized));
                    }
                    return;
                }
                ctx.data_mut(|d| d.insert_temp(last_id, (time, pos)));
            }
        } else {
            ctx.data_mut(|d| d.remove_temp::<(f64, egui::Pos2)>(last_id));
        }
    }

    let Some(origin) = ctx.input(|i| i.pointer.press_origin()) else {
        return;
    };
    if !free(origin) {
        return;
    }

    let decided = ctx.input(|i| i.pointer.primary_down() && i.pointer.is_decidedly_dragging());
    if !decided {
        return;
    }

    let already = ctx.data(|d| d.get_temp::<bool>(armed_id).unwrap_or(false));
    if already {
        return;
    }
    ctx.data_mut(|d| d.insert_temp(armed_id, true));
    ctx.data_mut(|d| d.remove_temp::<(f64, egui::Pos2)>(titleband_last_click_id()));
    ctx.send_viewport_cmd(ViewportCommand::StartDrag);
}

fn floating_toolbar(app: &mut ShellApp, ctx: &egui::Context, t: &Theme, queue: &mut Vec<Action>) {
    let y = HEADER_CENTER - icon_size() / 2.0;
    // Always over the canvas titleband: sidebar head when open, tab-strip bar
    // (left inset past traffic lights) when closed. Do not flip ground with
    // sidebar_open.
    let ground = t.neutral_bg();
    let cluster_w = left_chrome_w();
    let cluster_h = icon_size();
    // Same-frame blocker: drag_strip runs after this in `show`.
    block_window_drag(ctx, Rect::from_min_size(pos2(TOGGLE_X, y), vec2(cluster_w, cluster_h)));

    let (can_back, can_fwd) = app
        .session
        .ready()
        .map(|r| (r.workspace.can_back(), r.workspace.can_forward()))
        .unwrap_or((false, false));

    Area::new(Id::new("shell_sidebar_toolbar"))
        .order(Order::Foreground)
        .fixed_pos(pos2(TOGGLE_X, y))
        .show(ctx, |ui| {
            // Absolute place: icon slots at fixed x (no horizontal placer).
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            let origin = ui.cursor().min;
            let mut x = origin.x;
            for (i, pane) in SidebarPane::ALL.iter().enumerate() {
                if i > 0 {
                    x += toolbar_gap();
                }
                let slot = Rect::from_min_size(pos2(x, origin.y), vec2(icon_size(), cluster_h));
                let active = app.sidebar_open && app.pane == *pane;
                let (resp, _) = place_at(ui, slot, Layout::top_down(Align::Min), |ui| {
                    titlebar_icon(ui, t, pane.icon(), active, true, ground, pane.title())
                });
                if resp.clicked() {
                    queue.push(A::SelectPane(*pane));
                }
                x += icon_size();
            }
            x += group_gap();
            let nav = [
                (phosphor::ARROW_LEFT, can_back, A::NavBack, "Back"),
                (phosphor::ARROW_RIGHT, can_fwd, A::NavForward, "Forward"),
            ];
            for (i, (icon, enabled, action, tip)) in nav.into_iter().enumerate() {
                if i > 0 {
                    x += toolbar_gap();
                }
                let slot = Rect::from_min_size(pos2(x, origin.y), vec2(icon_size(), cluster_h));
                let (resp, _) = place_at(ui, slot, Layout::top_down(Align::Min), |ui| {
                    titlebar_icon(ui, t, icon, enabled, enabled, ground, tip)
                });
                if enabled && resp.clicked() {
                    queue.push(action);
                }
                x += icon_size();
            }
            claim(ui, Rect::from_min_size(origin, vec2(cluster_w, cluster_h)));
        });
}

/// Titleband icon: pane toggles (active ink) and back/forward (disabled = mute, no hover).
fn titlebar_icon(
    ui: &mut egui::Ui, t: &Theme, icon: &'static str, active: bool, enabled: bool,
    ground: egui::Color32, tip: &str,
) -> egui::Response {
    let hit = icon_hit_size();
    if enabled {
        let resp = icon_button_glyph(ui, t, icon, active, ground, hit, titlebar_glyph())
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        tip_text(ui.ctx(), &resp, tip);
        resp
    } else {
        let (rect, resp) = ui.allocate_exact_size(vec2(hit, hit), egui::Sense::hover());
        let color = t.neutral_fg_secondary();
        let g = ui
            .painter()
            .layout_no_wrap(icon.into(), phosphor_font_id(titlebar_glyph()), color);
        ui.painter()
            .galley(rect.center() - g.size() / 2.0, g, color);
        tip_text(ui.ctx(), &resp, tip);
        resp
    }
}

#[cfg(not(target_os = "macos"))]
fn window_resize_edges(ctx: &egui::Context) {
    use egui::{CursorIcon, PointerButton, ResizeDirection, ViewportCommand};
    let r = ctx.screen_rect();
    let b = 6.0;
    let zones = [
        (
            "shell_rz_n",
            egui::Rect::from_min_max(pos2(r.left() + b, r.top()), pos2(r.right() - b, r.top() + b)),
            ResizeDirection::North,
            CursorIcon::ResizeNorth,
        ),
        (
            "shell_rz_s",
            egui::Rect::from_min_max(
                pos2(r.left() + b, r.bottom() - b),
                pos2(r.right() - b, r.bottom()),
            ),
            ResizeDirection::South,
            CursorIcon::ResizeSouth,
        ),
        (
            "shell_rz_w",
            egui::Rect::from_min_max(
                pos2(r.left(), r.top() + b),
                pos2(r.left() + b, r.bottom() - b),
            ),
            ResizeDirection::West,
            CursorIcon::ResizeWest,
        ),
        (
            "shell_rz_e",
            egui::Rect::from_min_max(
                pos2(r.right() - b, r.top() + b),
                pos2(r.right(), r.bottom() - b),
            ),
            ResizeDirection::East,
            CursorIcon::ResizeEast,
        ),
        (
            "shell_rz_nw",
            egui::Rect::from_min_max(r.min, pos2(r.left() + b, r.top() + b)),
            ResizeDirection::NorthWest,
            CursorIcon::ResizeNwSe,
        ),
        (
            "shell_rz_ne",
            egui::Rect::from_min_max(pos2(r.right() - b, r.top()), pos2(r.right(), r.top() + b)),
            ResizeDirection::NorthEast,
            CursorIcon::ResizeNeSw,
        ),
        (
            "shell_rz_sw",
            egui::Rect::from_min_max(
                pos2(r.left(), r.bottom() - b),
                pos2(r.left() + b, r.bottom()),
            ),
            ResizeDirection::SouthWest,
            CursorIcon::ResizeNeSw,
        ),
        (
            "shell_rz_se",
            egui::Rect::from_min_max(pos2(r.right() - b, r.bottom() - b), r.max),
            ResizeDirection::SouthEast,
            CursorIcon::ResizeNwSe,
        ),
    ];
    for (id, rect, dir, cursor) in zones {
        Area::new(Id::new(id))
            .order(Order::Foreground)
            .fixed_pos(rect.min)
            .show(ctx, |ui| {
                let resp = ui
                    .allocate_response(rect.size(), Sense::drag())
                    .on_hover_cursor(cursor);
                if resp.drag_started_by(PointerButton::Primary) {
                    ui.ctx()
                        .send_viewport_cmd(ViewportCommand::BeginResize(dir));
                }
            });
    }
}

#[cfg(all(test, not(target_os = "macos")))]
mod tests {
    use super::metrics::{caption_cluster_w, tab_right_inset};

    #[test]
    fn caption_inset_matches_cluster() {
        assert!((tab_right_inset() - caption_cluster_w()).abs() < 0.01);
    }
}
