//! Tab strip driven by `Workspace::tab_strip`.
//!
//! Canvas bar flush to the panel top; always [`HEADER_H`], even with no tabs,
//! so traffic lights and the pane cluster sit on chrome, not the editor.
//! Tabs measured then placed on x. Active tab = canvas plate open into the
//! workspace (same fill, no bottom edge). Drag reorder; middle-click close;
//! active title → rename sheet; context menu.

use egui::{
    Align, CornerRadius, CursorIcon, DragAndDrop, Id, Layout, Sense, Stroke, StrokeKind, Ui,
    UiBuilder, pos2, scroll_area::ScrollBarVisibility, vec2,
};
use lb::Uuid;
use workspace_rs::file_cache::FilesExt;
use workspace_rs::tab::Destination;

use crate::components::{
    FG_HOVER, FG_PRESS, Radius, STROKE_HAIRLINE, Space, Theme, TypeRole, claim, context_menu,
    display_file_name, fit_outside_stroke_fill, phosphor, place_at, tab_icon, ui_width,
};

use crate::shell::ShellApp;
use crate::shell::action::Action;
use crate::shell::action::Action as A;
use crate::shell::titlebar::{
    HEADER_H, block_window_drag, tab_drag_gap, tab_left_inset, tab_right_inset,
};

const TAB_PAD_X: f32 = 12.0;
const TAB_MAX_W: f32 = 180.0;
const TAB_MIN_W: f32 = 88.0;
const CLOSE_SLOT: f32 = 18.0;
/// Window-top: 1 hairline for Outside stroke + 1 for macOS top-pixel fade so
/// the active tab top edge reads solid (not half-clipped / washed).
const TAB_TOP_AIR: f32 = STROKE_HAIRLINE * 2.0;
/// Inside each hit cell (tabs stay flush — no inter-tab gap). Reserves room for
/// Outside stroke so a neighbor hover wash cannot cover the shared edge.
const TAB_SIDE_AIR: f32 = STROKE_HAIRLINE;
/// Edge band + max speed for horizontal auto-scroll while reordering tabs.
const TAB_EDGE_BAND: f32 = 28.0;
const TAB_EDGE_SPEED: f32 = 900.0;

fn tab_edge_scroll_id() -> Id {
    Id::new("shell_tab_edge_scroll_x")
}

/// Typed so tab reorder never collides with tree file DnD payloads.
#[derive(Clone, Copy, Debug)]
struct TabReorder(usize);

/// Paint the titleband. Always claims [`HEADER_H`] (including zero tabs) so the
/// editor / landing page sit below chrome. Returns **`true`** after the band is
/// claimed.
pub fn show(app: &mut ShellApp, ui: &mut Ui, t: &Theme, queue: &mut Vec<Action>) -> bool {
    ui.spacing_mut().item_spacing = vec2(0.0, 0.0);

    let (tabs, can_reopen) = app
        .session
        .ready()
        .map(|ready| {
            let current = ready.workspace.current_tab.clone();
            let can_reopen = ready.workspace.can_reopen_closed_tab();
            let tabs: Vec<TabInfo> = ready
                .workspace
                .tab_strip
                .iter()
                .enumerate()
                .map(|(i, slot)| {
                    let title = tab_label(ready, &slot.dest);
                    let active = current.as_ref() == Some(&slot.dest);
                    let file_id = match &slot.dest {
                        Destination::File(id) => Some(*id),
                        _ => None,
                    };
                    TabInfo { idx: i, title, active, file_id, dest: slot.dest.clone() }
                })
                .collect();
            (tabs, can_reopen)
        })
        .unwrap_or_default();
    let tab_count = tabs.len();

    let sidebar_w = ui.max_rect().left() - ui.ctx().screen_rect().left();
    let left = tab_left_inset(sidebar_w);
    let right = tab_right_inset();
    let bar_w = ui_width(ui);
    // Flush to the central panel top — do not use cursor after Frame/spacing quirks.
    let top_left = pos2(ui.max_rect().left(), ui.max_rect().top());
    let outer = egui::Rect::from_min_size(top_left, vec2(bar_w, HEADER_H));

    // Bar fill (canvas) under tabs — and under traffic lights / pane cluster
    // when the strip is empty. Same ground as the sidebar titleband.
    ui.painter().rect_filled(outer, 0.0, t.neutral_bg());

    // Full bar→workspace hairline **under** the tabs. The active tab's canvas
    // plate paints on top of this and covers the segment under the tab so the
    // plate bleeds into the workspace (same fill — no dividing edge).
    let edge = Stroke::new(STROKE_HAIRLINE, t.neutral());
    let y = outer.bottom() - STROKE_HAIRLINE * 0.5;
    ui.painter().hline(outer.x_range(), y, edge);

    if tab_count == 0 {
        // Empty chrome is a drag region (toolbar / window controls register
        // their own blockers). Do not `block_window_drag` the whole band.
        claim(ui, outer);
        return true;
    }

    // Parent-owned insets (toolbar / window controls) — not Space tokens.
    if left > 0.0 {
        let _ = ui.allocate_rect(
            egui::Rect::from_min_size(top_left, vec2(left, HEADER_H)),
            Sense::hover(),
        );
    }

    let mid_left = top_left.x + left;
    let content_w: f32 = tabs.iter().map(|tab| measure_tab_w(ui, &tab.title)).sum();
    let scroll_w = tab_scroll_w(bar_w, left, right, content_w);

    if scroll_w > 0.5 {
        let scroll_rect =
            egui::Rect::from_min_size(pos2(mid_left, top_left.y), vec2(scroll_w, HEADER_H));
        // Visible strip only. Tab *content* rects extend past this when overflow
        // is scrolled left, and those must not cover the right grab gap.
        block_window_drag(ui.ctx(), scroll_rect);
        let _ = place_at(ui, scroll_rect, Layout::top_down(Align::Min), |ui| {
            ui.set_width(scroll_w);
            ui.set_height(HEADER_H);
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            // No scrollbar (Zed / toolbar-style): wheel + DnD edge-scroll only.
            let mut hscroll = egui::ScrollArea::horizontal()
                .id_salt("shell_tab_scroll")
                .max_width(scroll_w)
                .auto_shrink([false, false])
                .scroll_bar_visibility(ScrollBarVisibility::AlwaysHidden);
            if let Some(x) = ui
                .ctx()
                .data_mut(|d| d.remove_temp::<f32>(tab_edge_scroll_id()))
            {
                hscroll = hscroll.horizontal_scroll_offset(x);
            }
            let out = hscroll.show(ui, |ui| {
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                ui.set_height(HEADER_H);
                // Flush scroll content to the bar top (same y as outer).
                let strip_tl = pos2(ui.max_rect().left(), ui.max_rect().top());
                let mut x = strip_tl.x;
                let mut total_w = 0.0_f32;
                let reordering = DragAndDrop::has_payload_of_type::<TabReorder>(ui.ctx());
                // Grabbing for the whole reorder — never NotAllowed on a no-op slot.
                if reordering {
                    ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
                }
                // Flush on x — no gap between tabs (shared edge = one drop
                // seam for reorder; Outside stroke sits in side air).
                for tab in &tabs {
                    let w = measure_tab_w(ui, &tab.title);
                    let tab_r = egui::Rect::from_min_size(pos2(x, strip_tl.y), vec2(w, HEADER_H));
                    let (out, _) = place_at(ui, tab_r, Layout::top_down(Align::Min), |ui| {
                        tab_button(ui, t, tab, tab_count, can_reopen)
                    });
                    apply_tab_out(queue, tab, out);
                    x += w;
                    total_w += w;
                }
                claim(ui, egui::Rect::from_min_size(strip_tl, vec2(total_w.max(1.0), HEADER_H)));
            });
            // Edge auto-scroll when reordering past the visible strip.
            if DragAndDrop::has_payload_of_type::<TabReorder>(ui.ctx()) {
                if let Some(pointer) = ui.input(|i| i.pointer.interact_pos()) {
                    let clip = scroll_rect;
                    if clip.width() >= TAB_EDGE_BAND * 2.0 {
                        let dt = ui.input(|i| i.unstable_dt).clamp(1.0 / 240.0, 0.05);
                        let max_off = (out.content_size.x - scroll_w).max(0.0);
                        if max_off > 0.5 {
                            let mut off = out.state.offset.x;
                            let mut moved = false;
                            if pointer.x < clip.left() + TAB_EDGE_BAND {
                                let depth = ((clip.left() + TAB_EDGE_BAND - pointer.x)
                                    / TAB_EDGE_BAND)
                                    .clamp(0.0, 1.0);
                                off = (off - TAB_EDGE_SPEED * depth * dt).max(0.0);
                                moved = true;
                            } else if pointer.x > clip.right() - TAB_EDGE_BAND {
                                let depth = ((pointer.x - (clip.right() - TAB_EDGE_BAND))
                                    / TAB_EDGE_BAND)
                                    .clamp(0.0, 1.0);
                                off = (off + TAB_EDGE_SPEED * depth * dt).min(max_off);
                                moved = true;
                            }
                            if moved && (off - out.state.offset.x).abs() > 0.25 {
                                ui.ctx()
                                    .data_mut(|d| d.insert_temp(tab_edge_scroll_id(), off));
                                ui.ctx().request_repaint();
                            }
                        }
                    }
                }
                ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
            }
        });
    }

    if right > 0.0 {
        let right_r = egui::Rect::from_min_size(
            pos2(top_left.x + bar_w - right, top_left.y),
            vec2(right, HEADER_H),
        );
        let _ = ui.allocate_rect(right_r, Sense::hover());
    }

    claim(ui, outer);
    true
}

fn tab_label(ready: &crate::shell::session::Ready, dest: &Destination) -> String {
    match dest {
        Destination::File(id) | Destination::MindMap(id) | Destination::SpaceInspector(id) => ready
            .workspace
            .files
            .read()
            .unwrap()
            .get_by_id(*id)
            .map(|f| f.name.clone())
            .unwrap_or_else(|| "Unknown".into()),
        Destination::Search => "Search".into(),
    }
}

fn measure_tab_w(ui: &Ui, name: &str) -> f32 {
    let nw = crate::components::measure_file_name(ui, display_file_name(name));
    let icon_w = TypeRole::Body.size() + Space::Xs.pts();
    (TAB_PAD_X + icon_w + nw + Space::Xs.pts() + CLOSE_SLOT + TAB_PAD_X).clamp(TAB_MIN_W, TAB_MAX_W)
}

/// Visible tab-scroll width. Never consumes [`tab_drag_gap`] or the caption
/// inset; leftover after the last tab is empty chrome (window-move).
fn tab_scroll_w(bar_w: f32, left: f32, right: f32, content_w: f32) -> f32 {
    let chrome = left + right.max(0.0);
    let with_gap = (bar_w - chrome - tab_drag_gap()).max(0.0);
    // Keep one tab hittable on a narrow window even if it eats the drag gap.
    let available = if with_gap >= TAB_MIN_W { with_gap } else { (bar_w - chrome).max(0.0) };
    content_w.min(available)
}

struct TabInfo {
    idx: usize,
    title: String,
    active: bool,
    file_id: Option<Uuid>,
    dest: Destination,
}

#[derive(Clone, Copy)]
enum TabMenu {
    Close,
    CloseOthers,
    CloseLeft,
    CloseRight,
    CloseAll,
    Rename,
    Share,
    CopyLink,
    ReopenClosed,
}

struct TabOut {
    /// Primary click on body (not X) — select if inactive.
    select: bool,
    /// Primary click on active file tab body — open rename sheet.
    rename: bool,
    close: bool,
    /// Pre-remove insert-before index pair when a drop completed on this tab.
    reorder: Option<(usize, usize)>,
    menu: Option<TabMenu>,
}

fn apply_tab_out(queue: &mut Vec<Action>, tab: &TabInfo, out: TabOut) {
    if let Some((src, dst)) = out.reorder {
        queue.push(A::ReorderTab { src, dst });
        return;
    }
    if let Some(cmd) = out.menu {
        match cmd {
            TabMenu::Close => queue.push(A::CloseTab(tab.idx)),
            TabMenu::CloseOthers => queue.push(A::CloseOtherTabs(tab.idx)),
            TabMenu::CloseLeft => queue.push(A::CloseTabsToLeft(tab.idx)),
            TabMenu::CloseRight => queue.push(A::CloseTabsToRight(tab.idx)),
            TabMenu::CloseAll => queue.push(A::CloseAllTabs),
            TabMenu::Rename => {
                if let Some(id) = tab.file_id {
                    queue.push(A::OpenRename(id));
                }
            }
            TabMenu::Share => {
                if let Some(id) = tab.file_id {
                    queue.push(A::OpenShare(id));
                }
            }
            TabMenu::CopyLink => {
                if let Some(id) = tab.file_id {
                    queue.push(A::CopyLink(id));
                }
            }
            TabMenu::ReopenClosed => queue.push(A::ReopenClosedTab),
        }
        return;
    }
    if out.close {
        queue.push(A::CloseTab(tab.idx));
    } else if out.rename {
        if let Some(id) = tab.file_id {
            queue.push(A::OpenRename(id));
        }
    } else if out.select {
        queue.push(A::SelectTab(tab.idx));
    }
}

/// Visual plate inside the hit cell, fitted so Outside stroke stays visible.
///
/// - Top: [`TAB_TOP_AIR`] (stroke + OS top-row fade)
/// - Sides: [`TAB_SIDE_AIR`] so flush neighbors share an edge without hover
///   covering the active tab’s Outside stroke (no inter-tab *layout* gap)
/// - Clip: [`fit_outside_stroke_fill`] for panel / window edges
/// - Bottom: inactive leaves bar hairline; active fill covers it (stroke is
///   clipped open — see [`tab_button`])
fn tab_chrome_rect(ui: &Ui, body: egui::Rect, active: bool) -> egui::Rect {
    let top = body.top() + TAB_TOP_AIR;
    let bottom = if active {
        // Cover bar hairline and open into workspace canvas.
        body.bottom() + STROKE_HAIRLINE
    } else {
        // Leave the strip→workspace hairline intact under the wash.
        body.bottom() - STROKE_HAIRLINE
    };
    let desired = egui::Rect::from_min_max(
        pos2(body.left() + TAB_SIDE_AIR, top),
        pos2(body.right() - TAB_SIDE_AIR, bottom.max(top + 1.0)),
    );
    // Clip for Outside budget: panel/scroll clip. Expand bottom when active so
    // open-into-workspace bleed is not pulled back in.
    let mut clip = ui.clip_rect();
    if active {
        clip.max.y = clip.max.y.max(desired.bottom() + STROKE_HAIRLINE);
    }
    // Side air already applied; still clamp to panel/window edges if tighter.
    fit_outside_stroke_fill(desired, clip)
}

fn tab_button(ui: &mut Ui, t: &Theme, tab: &TabInfo, tab_count: usize, can_reopen: bool) -> TabOut {
    let name = &tab.title;
    let active = tab.active;
    let index = tab.idx;
    let w = measure_tab_w(ui, name);
    let h = HEADER_H;
    let (rect, _) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
    let id = ui.id().with("shell_tab").with(index);
    let resp = ui.interact(rect, id, Sense::click_and_drag());

    // No hover wash / ink while any drag is live (tab reorder or tree DnD).
    let dragging = DragAndDrop::has_any_payload(ui.ctx()) || ui.ctx().dragged_id().is_some();
    let over = !dragging && (resp.hovered() || ui.ctx().rect_contains_pointer(ui.layer_id(), rect));
    let hover_t = ui.ctx().animate_bool(resp.id.with("hov"), over);

    // Drag-reorder: arm payload on drag_started (egui drag threshold).
    resp.dnd_set_drag_payload(TabReorder(index));

    // Hit cell = full height; chrome is inset (settings Outside-stroke idea +
    // top air so the border is not clipped by the window edge).
    let radius =
        CornerRadius { nw: Radius::Control.pts(), ne: Radius::Control.pts(), sw: 0, se: 0 };
    let body = rect;
    let chrome = tab_chrome_rect(ui, body, active);
    let canvas = t.neutral_bg();
    let edge = Stroke::new(STROKE_HAIRLINE, t.neutral());

    if active {
        // Plate covers the strip hairline under this tab. Outside stroke so
        // label/close cannot cover rounded corners.
        ui.painter().rect_filled(chrome, radius, canvas);
        // Clip at the strip hairline (not chrome.bottom()) so L/R strokes meet
        // the bar instead of running past it. A closed-rect punch left L-hooks.
        let mut stroke_clip = ui.clip_rect();
        let hairline_y = body.bottom() - STROKE_HAIRLINE * 0.5;
        stroke_clip.max.y = stroke_clip.max.y.min(hairline_y);
        ui.painter().with_clip_rect(stroke_clip).rect_stroke(
            chrome,
            radius,
            edge,
            StrokeKind::Outside,
        );
    } else if hover_t > 0.0 {
        let wash = t.wash_toward_neutral_fg(t.neutral_bg(), FG_HOVER * hover_t);
        ui.painter().rect_filled(chrome, radius, wash);
    }

    let ink = if active {
        t.neutral_fg()
    } else {
        t.neutral_fg_secondary()
            .lerp_to_gamma(t.neutral_fg(), hover_t)
    };

    let icon = tab_icon(&tab.dest, name);
    let icon_g =
        ui.painter()
            .layout_no_wrap(icon.into(), crate::components::phosphor_ui_font_id(), ink);
    let label = display_file_name(name);
    let show_close = over || active;
    let close_reserve = if show_close { CLOSE_SLOT + 2.0 } else { 0.0 };
    let text_max =
        (body.width() - TAB_PAD_X * 2.0 - icon_g.size().x - Space::Xs.pts() - close_reserve)
            .max(8.0);

    let cy = body.center().y;
    let mut cx = body.left() + TAB_PAD_X;
    ui.painter()
        .galley(pos2(cx, cy - icon_g.size().y / 2.0), icon_g.clone(), ink);
    cx += icon_g.size().x + Space::Xs.pts();
    let name_lh = TypeRole::Body.line_height();
    crate::components::paint_file_name(
        ui,
        label,
        ink,
        egui::Rect::from_min_size(pos2(cx, cy - name_lh / 2.0), vec2(text_max, name_lh)),
    );

    let mut close = false;
    if show_close {
        let close_rect = egui::Rect::from_center_size(
            pos2(body.right() - TAB_PAD_X - CLOSE_SLOT / 2.0, cy),
            vec2(CLOSE_SLOT, CLOSE_SLOT),
        );
        let close_id = ui.id().with("shell_tab_close").with(index);
        let close_resp = ui
            .interact(close_rect, close_id, Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand);
        let close_over = ui.ctx().rect_contains_pointer(ui.layer_id(), close_rect);
        let close_h = ui.ctx().animate_bool(close_resp.id, close_over);
        // Ground = plate under the X (canvas when active; hover wash when inactive).
        let tab_ground = if active {
            t.neutral_bg()
        } else {
            t.wash_toward_neutral_fg(t.neutral_bg(), FG_HOVER * hover_t)
        };
        if close_h > 0.0 || close_over {
            let amount =
                if close_resp.is_pointer_button_down_on() { FG_PRESS } else { FG_HOVER * close_h };
            ui.painter().rect_filled(
                close_rect,
                Radius::Sm.corner(),
                t.wash_toward_neutral_fg(tab_ground, amount),
            );
        }
        let x_color = t
            .neutral_fg_secondary()
            .lerp_to_gamma(t.neutral_fg(), close_h);
        let xg = ui.painter().layout_no_wrap(
            phosphor::X.into(),
            egui::FontId::new(11.0, egui::FontFamily::Name(std::sync::Arc::from("phosphor"))),
            x_color,
        );
        ui.painter()
            .galley(close_rect.center() - xg.size() / 2.0, xg, x_color);
        if close_resp.clicked() {
            close = true;
        }
    }

    // Drop target: insert-before indicator on left or right half of this tab.
    // Skip no-ops (same slot: `dst == src` or `dst == src + 1`) — match apply.
    let mut reorder = None;
    if let (Some(pointer), Some(payload)) =
        (ui.input(|i| i.pointer.interact_pos()), DragAndDrop::payload::<TabReorder>(ui.ctx()))
    {
        if body.contains(pointer) {
            let drop_left = pointer.x < body.center().x;
            let dst = if drop_left { index } else { index + 1 };
            let src = payload.0;
            let is_noop = dst == src || dst == src + 1;
            if !is_noop {
                let x = if drop_left { body.left() } else { body.right() };
                let stroke = Stroke::new(STROKE_HAIRLINE * 2.0, t.accent());
                ui.scope_builder(
                    UiBuilder::new().layer_id(egui::LayerId::new(
                        egui::Order::Foreground,
                        Id::new("shell_tab_reorder_drop"),
                    )),
                    |ui| {
                        ui.painter().vline(x, chrome.y_range(), stroke);
                    },
                );
                if let Some(src) = resp.dnd_release_payload::<TabReorder>() {
                    reorder = Some((src.0, dst));
                }
            }
        }
    }

    let is_file = tab.file_id.is_some();
    let menu = context_menu::show(&resp, t, |m| {
        m.item(phosphor::X, "Close", TabMenu::Close);
        if tab_count >= 2 {
            m.item(phosphor::X_CIRCLE, "Close Others", TabMenu::CloseOthers);
        }
        if index > 0 {
            m.item(phosphor::CARET_LEFT, "Close to the Left", TabMenu::CloseLeft);
        }
        if index + 1 < tab_count {
            m.item(phosphor::CARET_RIGHT, "Close to the Right", TabMenu::CloseRight);
        }
        m.item(phosphor::TABS, "Close All", TabMenu::CloseAll);
        if is_file || can_reopen {
            m.separator();
        }
        if is_file {
            m.item(phosphor::PENCIL, "Rename…", TabMenu::Rename);
            m.item(phosphor::USERS, "Share…", TabMenu::Share);
            m.item(phosphor::LINK, "Copy link", TabMenu::CopyLink);
        }
        if can_reopen {
            if is_file {
                m.separator();
            }
            m.item(phosphor::ARROWS_CLOCKWISE, "Reopen Closed Tab", TabMenu::ReopenClosed);
        }
    });

    let middle_close = resp.middle_clicked();
    let primary = resp.clicked() && !close && reorder.is_none() && menu.is_none();
    // Active file tab body click → rename sheet (workspace: inline; shell: sheet).
    let rename = primary && active && is_file;
    let select = primary && !active;

    TabOut { select, rename, close: close || middle_close, reorder, menu }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::{ThemeExt, install};
    use crate::shell::ShellApp;
    use egui::{CentralPanel, Context, FullOutput, Pos2, RawInput, Rect, Vec2};

    #[test]
    fn empty_strip_still_claims_titleband() {
        let mut app = ShellApp::default();
        let ctx = Context::default();
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 800.0))),
            ..Default::default()
        };
        let FullOutput { .. } = ctx.run(input.clone(), |ctx| {
            install(ctx);
        });
        let mut claimed_h = 0.0_f32;
        let FullOutput { .. } = ctx.run(input, |ctx| {
            install(ctx);
            let t = ctx.get_lb_theme();
            CentralPanel::default().show(ctx, |ui| {
                let top = ui.max_rect().top();
                let mut queue = Vec::new();
                assert!(show(&mut app, ui, &t, &mut queue));
                claimed_h = ui.cursor().top() - top;
            });
        });
        assert!(
            (claimed_h - HEADER_H).abs() < 0.5,
            "empty strip claimed {claimed_h:.1}, want HEADER_H={HEADER_H:.1}"
        );
    }

    #[test]
    fn overflowing_tabs_leave_drag_gap() {
        let bar = 800.0;
        let left = 0.0;
        let right = 10.0;
        let content = 5000.0;
        let w = tab_scroll_w(bar, left, right, content);
        let leftover = bar - left - w - right;
        assert!(
            leftover + 0.01 >= tab_drag_gap(),
            "leftover={leftover:.1} want ≥ drag gap {}",
            tab_drag_gap()
        );
        assert!((leftover - tab_drag_gap()).abs() < 0.5);
    }

    #[test]
    fn few_tabs_scroll_matches_content() {
        let w = tab_scroll_w(800.0, 0.0, 10.0, 200.0);
        assert!((w - 200.0).abs() < 0.5);
    }

    #[test]
    fn narrow_bar_keeps_drag_gap() {
        let gap = tab_drag_gap();
        let bar = 200.0;
        let left = 40.0;
        let right = 20.0;
        let w = tab_scroll_w(bar, left, right, 500.0);
        assert!((w - (bar - left - right - gap)).abs() < 0.5);
        assert!(w >= 0.0);
        assert!(bar - left - w - right + 0.01 >= gap);
    }
}
