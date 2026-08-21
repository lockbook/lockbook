//! Frameless window top chrome: view toggles, tabs inset, window controls.
//!
//! | Platform | Window chrome |
//! |----------|---------------|
//! | macOS | Native traffic lights (host: fullsize content + transparent titlebar) |
//! | Windows / Linux | Minimize · maximize/restore · close — Chrome/Win11 caption plates |
//!
//! View toggles (Files / Recents / Shared) float top-left and stay visible with
//! the sidebar closed. Tab strip insets clear this cluster (and macOS lights) —
//! see [`tab_left_inset`] / [`tab_right_inset`].

use egui::{Area, Id, Order, Rect, pos2, vec2};
#[cfg(not(target_os = "macos"))]
use egui::{Sense, Ui};

#[cfg(not(target_os = "macos"))]
use crate::components::FG_HOVER;
#[cfg(not(target_os = "macos"))]
use crate::components::foundation::chrome::phosphor_font_id;
#[cfg(not(target_os = "macos"))]
use crate::components::phosphor;
use crate::components::{
    Space, Theme, claim, control_height, icon_button_hit_font, phosphor_titleband_font_id,
    place_at, tip_text,
};
use egui::{Align, Layout};

use super::ShellApp;
use super::action::Action as A;
use super::action::{Action, SidebarPane};

/// y-center of the title row. macOS keeps traffic-light alignment (~16pt);
/// Win/Linux is a Chrome-like ~40pt strip.
#[cfg(target_os = "macos")]
pub const HEADER_CENTER: f32 = 16.0;
#[cfg(not(target_os = "macos"))]
pub const HEADER_CENTER: f32 = 20.0;
/// Full title / tab strip height.
pub const HEADER_H: f32 = HEADER_CENTER * 2.0;

/// Left edge of the floating toolbar. Cleared past traffic lights on macOS.
#[cfg(target_os = "macos")]
pub const TOGGLE_X: f32 = 76.0;
#[cfg(not(target_os = "macos"))]
pub const TOGGLE_X: f32 = 10.0;

const TOOLBAR_GAP: f32 = 4.0;
/// Win11 / Chrome caption button width. Height is [`HEADER_H`] (full-bleed).
#[cfg(not(target_os = "macos"))]
const CAPTION_BTN_W: f32 = 46.0;
/// Extra air above tabs when the window is restored (Chrome restored padding).
/// Maximized and macOS stay flush aside from tab stroke air.
#[cfg(not(target_os = "macos"))]
const RESTORED_TAB_AIR: f32 = 8.0;

/// View-toggle cluster only (Files / Recents / Shared). Settings lives in the
/// footer with sync — app chrome, not part of the pane multi-select.
const TOOLBAR_ICONS: usize = 3;

/// Hit / hover-wash size for titleband icons. Glyph is
/// [`phosphor_titleband_font_id`]; hit is inset so wash is not flush to the
/// window top/bottom of `HEADER_H`.
fn icon_hit_size() -> f32 {
    let air = Space::Xs.pts() * 2.0; // top + bottom within the strip
    (HEADER_H - air).min(control_height()).max(1.0)
}

/// Layout stride for titleband icon clusters (hit size, not full control height).
fn icon_size() -> f32 {
    icon_hit_size()
}

/// Width of the floating Files/Recents/Shared cluster.
pub fn toolbar_cluster_w() -> f32 {
    let n = TOOLBAR_ICONS as f32;
    n * icon_size() + (n - 1.0) * TOOLBAR_GAP
}

/// Extra gap between toolbar cluster and first tab when sidebar is collapsed.
fn tab_gap_after_toolbar() -> f32 {
    Space::Md.pts()
}

/// Left pad inside the **content** tab strip so tabs clear traffic lights +
/// view toggles when the sidebar is closed. Zero when the sidebar is open
/// (tabs live only in the central column).
pub fn tab_left_inset(sidebar_open: bool) -> f32 {
    if sidebar_open {
        0.0
    } else {
        // Toolbar is window-absolute at TOGGLE_X; central panel starts at 0 when
        // sidebar is closed, so inset = toolbar right edge + air.
        TOGGLE_X + toolbar_cluster_w() + tab_gap_after_toolbar()
    }
}

/// Right pad so tabs never sit under drawn window controls (Windows/Linux).
/// Includes a small always-empty gap before the caption cluster so there is a
/// grab handle even when tabs fill the strip.
pub fn tab_right_inset() -> f32 {
    #[cfg(target_os = "macos")]
    {
        Space::Sm.pts()
    }
    #[cfg(not(target_os = "macos"))]
    {
        caption_cluster_w() + Space::Md.pts()
    }
}

/// Caption-button cluster width (min + max + close). Not the tab inset — the
/// inset is wider by a drag gap that must remain a window-move region.
#[cfg(not(target_os = "macos"))]
fn caption_cluster_w() -> f32 {
    3.0 * CAPTION_BTN_W
}

/// Extra top inset for tab hit cells so restored Win/Linux windows have a
/// Chrome-like grab strip above the tabs. 0 on macOS and when maximized.
pub fn tab_caption_air(ctx: &egui::Context) -> f32 {
    #[cfg(target_os = "macos")]
    {
        let _ = ctx;
        0.0
    }
    #[cfg(not(target_os = "macos"))]
    {
        if ctx.input(|i| i.viewport().maximized.unwrap_or(false)) { 0.0 } else { RESTORED_TAB_AIR }
    }
}

/// Temp-data id: rects where a primary drag must **not** move the OS window
/// (tabs, toolbar, window controls). Written during panel paint; read by
/// [`drag_strip`] after panels (shell root order).
fn window_drag_blockers_id() -> Id {
    Id::new("shell_window_drag_blockers")
}

/// Exclude `rect` from the window-move drag band for this frame.
/// Call from interactive top chrome (tabs, etc.) before [`show`].
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

/// Full-width band of height [`HEADER_H`].
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

    let Some(origin) = ctx.input(|i| i.pointer.press_origin()) else {
        return;
    };
    if !header.contains(origin) || pointer_in_blockers(origin, &blockers) {
        return;
    }

    // Double-click free header → toggle maximize.
    if ctx.input(|i| i.pointer.button_double_clicked(PointerButton::Primary)) {
        if let Some(pos) = ctx.pointer_interact_pos() {
            if header.contains(pos) && !pointer_in_blockers(pos, &blockers) {
                let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                ctx.send_viewport_cmd(ViewportCommand::Maximized(!maximized));
            }
        }
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
    ctx.send_viewport_cmd(ViewportCommand::StartDrag);
}

fn floating_toolbar(app: &mut ShellApp, ctx: &egui::Context, t: &Theme, queue: &mut Vec<Action>) {
    let y = HEADER_CENTER - icon_size() / 2.0;
    // Always over surface chrome: sidebar head when open, tab-strip bar (left
    // inset past traffic lights) when closed. Canvas starts at the first tab
    // plate — never under these toggles. Do not flip ground with sidebar_open.
    let ground = t.neutral_bg_secondary();
    let n = TOOLBAR_ICONS;
    let cluster_w = toolbar_cluster_w();
    let cluster_h = icon_size();
    // Same-frame blocker: drag_strip runs after this in `show`.
    block_window_drag(ctx, Rect::from_min_size(pos2(TOGGLE_X, y), vec2(cluster_w, cluster_h)));

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
                    x += TOOLBAR_GAP;
                }
                let slot = Rect::from_min_size(pos2(x, origin.y), vec2(icon_size(), cluster_h));
                let active = app.sidebar_open && app.pane == *pane;
                let (resp, _) = place_at(ui, slot, Layout::top_down(Align::Min), |ui| {
                    let resp = icon_button_hit_font(
                        ui,
                        t,
                        pane.icon(),
                        active,
                        ground,
                        icon_hit_size(),
                        phosphor_titleband_font_id(),
                    );
                    tip_text(ui.ctx(), &resp, pane.title());
                    resp
                });
                if resp.clicked() {
                    queue.push(A::SelectPane(*pane));
                }
                x += icon_size();
            }
            let _ = n;
            claim(ui, Rect::from_min_size(origin, vec2(cluster_w, cluster_h)));
        });
}

/// Win/Linux: full-bleed min · max/restore · close, flush to the top-right.
/// Only the caption plates block window-move; the gap before them is a grab.
#[cfg(not(target_os = "macos"))]
fn window_controls(ctx: &egui::Context, t: &Theme) {
    use egui::{Align2, ViewportCommand};
    let cluster_w = caption_cluster_w();
    let screen = ctx.screen_rect();
    block_window_drag(
        ctx,
        Rect::from_min_size(
            pos2(screen.right() - cluster_w, screen.top()),
            vec2(cluster_w, HEADER_H),
        ),
    );
    Area::new(Id::new("shell_window_controls"))
        .order(Order::Foreground)
        .anchor(Align2::RIGHT_TOP, vec2(0.0, 0.0))
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            let origin = ui.cursor().min;
            let ground = t.neutral_bg();
            let maximized = ui.input(|i| i.viewport().maximized.unwrap_or(false));
            let max_icon = if maximized { phosphor::COPY } else { phosphor::SQUARE };
            let marks = [phosphor::MINUS, max_icon, phosphor::X];
            for (i, icon) in marks.into_iter().enumerate() {
                let slot = Rect::from_min_size(
                    pos2(origin.x + i as f32 * CAPTION_BTN_W, origin.y),
                    vec2(CAPTION_BTN_W, HEADER_H),
                );
                let _ = place_at(ui, slot, Layout::top_down(Align::Min), |ui| {
                    let danger = i == 2;
                    if caption_button(ui, t, icon, danger, ground).clicked() {
                        match i {
                            0 => ui.ctx().send_viewport_cmd(ViewportCommand::Minimized(true)),
                            1 => ui
                                .ctx()
                                .send_viewport_cmd(ViewportCommand::Maximized(!maximized)),
                            _ => ui.ctx().send_viewport_cmd(ViewportCommand::Close),
                        }
                    }
                });
            }
            claim(ui, Rect::from_min_size(origin, vec2(cluster_w, HEADER_H)));
        });
}

/// Full-bleed caption plate. Small glyph, large hit, square hover (Win11 / Chrome).
/// Close hover is solid danger with a light mark.
#[cfg(not(target_os = "macos"))]
fn caption_button(
    ui: &mut Ui, t: &Theme, icon: &'static str, danger: bool, ground: egui::Color32,
) -> egui::Response {
    use crate::components::TypeRole;
    use crate::components::foundation::chrome::HOVER_ANIM_SECS;
    let (rect, resp) = ui.allocate_exact_size(vec2(CAPTION_BTN_W, HEADER_H), Sense::click());
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
        ui.painter().rect_filled(rect, 0.0, wash);
    }
    let color = if danger {
        t.neutral_fg_secondary()
            .lerp_to_gamma(egui::Color32::WHITE, hover)
    } else {
        t.neutral_fg_secondary()
            .lerp_to_gamma(t.neutral_fg(), hover)
    };
    let g =
        ui.painter()
            .layout_no_wrap(icon.into(), phosphor_font_id(TypeRole::Mono.size()), color);
    ui.painter()
        .galley(rect.center() - g.size() / 2.0, g, color);
    resp
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
