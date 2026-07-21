//! Floating context menu — content-sized, shared chrome, optional icons,
//! and one-level submenus with **safe-corner** hover bridges.
//!
//! # Safe corners
//! Cascading menus fail if the submenu closes the moment the pointer leaves
//! the parent row on a diagonal toward the flyout. While a submenu is open we
//! treat the convex corridor between the parent row’s open edge and the
//! flyout’s near edge as still “on” that parent — classic Amazon/macOS
//! behavior — so the pointer can travel diagonally without thrashing.

use std::sync::Arc;

use egui::{
    Align, Area, Color32, FontFamily, FontId, Id, Layout, Order, Pos2, Rect, Response, Sense,
    Stroke, Vec2, pos2, vec2,
};

use super::FloatingChrome;
use crate::theme::palette_v2::{Theme, ThemeExt};

const MIN_W: f32 = 112.0;
const MAX_W: f32 = 280.0;
const ROW_H: f32 = 24.0;
const PAD_LEADING: f32 = 6.0;
const PAD_TRAILING: f32 = 12.0;
const ICON_SLOT: f32 = 16.0;
const ICON_SIZE: f32 = 13.0;
const ICON_TEXT_GAP: f32 = 6.0;
const CARET_SLOT: f32 = 14.0;
const PAD_X_TEXT: f32 = 12.0;
const LABEL_SIZE: f32 = 13.0;
const SEP_V: f32 = 4.0;
const ROW_RADIUS: f32 = 4.0;
const SUB_GAP: f32 = 2.0;
/// Align flyout top with menu frame content (frame pad is 5).
const SUB_Y_NUDGE: f32 = -5.0;

/// Phosphor caret-right for submenu parents.
const CARET_RIGHT: &str = "\u{E13A}";

fn open_key() -> Id {
    Id::new("lb_float_text_menu_open")
}

/// Whether a floating context menu is currently open (any host).
pub fn is_menu_open(ctx: &egui::Context) -> bool {
    ctx.memory(|m| m.data.get_temp::<OpenState>(open_key()).is_some())
}

fn root_area_key() -> Id {
    Id::new("lb_float_text_menu_area")
}

fn sub_area_key() -> Id {
    Id::new("lb_float_text_menu_sub_area")
}

fn phosphor_font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(Arc::from("phosphor")))
}

#[derive(Clone, Copy)]
struct OpenState {
    host: Id,
    pos: Pos2,
    /// Root row index of the open submenu parent, if any.
    open_sub: Option<usize>,
    /// Previous-frame parent row + flyout rects for safe-corner hit tests
    /// before this frame’s layout runs.
    parent_rect: Option<Rect>,
    sub_rect: Option<Rect>,
}

// ── Menu model ───────────────────────────────────────────────────────────────

enum Row<T> {
    Item {
        icon: Option<&'static str>,
        label: String,
        danger: bool,
        value: T,
    },
    Submenu {
        icon: Option<&'static str>,
        label: String,
        children: MenuEntries<T>,
    },
    Sep,
}

/// Builder for typed menu actions (labels, icons, separators, one-level subs).
pub struct MenuEntries<T> {
    rows: Vec<Row<T>>,
}

impl<T> MenuEntries<T> {
    pub fn new() -> Self {
        Self { rows: Vec::new() }
    }

    pub fn item(&mut self, label: impl Into<String>, value: T) {
        self.rows.push(Row::Item {
            icon: None,
            label: label.into(),
            danger: false,
            value,
        });
    }

    pub fn item_danger(&mut self, label: impl Into<String>, value: T) {
        self.rows.push(Row::Item {
            icon: None,
            label: label.into(),
            danger: true,
            value,
        });
    }

    pub fn item_icon(&mut self, icon: &'static str, label: impl Into<String>, value: T) {
        self.rows.push(Row::Item {
            icon: Some(icon),
            label: label.into(),
            danger: false,
            value,
        });
    }

    pub fn item_icon_danger(&mut self, icon: &'static str, label: impl Into<String>, value: T) {
        self.rows.push(Row::Item {
            icon: Some(icon),
            label: label.into(),
            danger: true,
            value,
        });
    }

    /// Parent row that opens a nested panel. `build` fills the submenu.
    pub fn submenu(
        &mut self, icon: Option<&'static str>, label: impl Into<String>,
        build: impl FnOnce(&mut MenuEntries<T>),
    ) {
        let mut children = MenuEntries::new();
        build(&mut children);
        if children.is_empty() {
            return;
        }
        self.rows.push(Row::Submenu { icon, label: label.into(), children });
    }

    pub fn submenu_icon(
        &mut self, icon: &'static str, label: impl Into<String>,
        build: impl FnOnce(&mut MenuEntries<T>),
    ) {
        self.submenu(Some(icon), label, build);
    }

    pub fn separator(&mut self) {
        self.rows.push(Row::Sep);
    }

    pub fn is_empty(&self) -> bool {
        !self
            .rows
            .iter()
            .any(|r| matches!(r, Row::Item { .. } | Row::Submenu { .. }))
    }

    fn any_icon(&self) -> bool {
        self.rows.iter().any(|r| match r {
            Row::Item { icon: Some(_), .. } | Row::Submenu { icon: Some(_), .. } => true,
            Row::Submenu { children, .. } => children.any_icon(),
            _ => false,
        })
    }
}

impl<T> Default for MenuEntries<T> {
    fn default() -> Self {
        Self::new()
    }
}

// ── Geometry ─────────────────────────────────────────────────────────────────

fn measure_panel_width<T>(
    ui: &egui::Ui, rows: &[Row<T>], with_icons: bool, carets: bool,
) -> f32 {
    let mut content_w = MIN_W;
    for row in rows {
        let (icon, label, is_sub) = match row {
            Row::Item { icon, label, .. } => (*icon, label.as_str(), false),
            Row::Submenu { icon, label, .. } => (*icon, label.as_str(), true),
            Row::Sep => continue,
        };
        let g = ui.painter().layout_no_wrap(
            label.to_owned(),
            FontId::proportional(LABEL_SIZE),
            Color32::PLACEHOLDER,
        );
        let mut w = if with_icons || icon.is_some() {
            PAD_LEADING + ICON_SLOT + ICON_TEXT_GAP + g.size().x + PAD_TRAILING
        } else {
            PAD_X_TEXT * 2.0 + g.size().x
        };
        if carets && is_sub {
            w += CARET_SLOT;
        }
        content_w = content_w.max(w);
    }
    content_w.clamp(MIN_W, MAX_W)
}

fn point_in_triangle(p: Pos2, a: Pos2, b: Pos2, c: Pos2) -> bool {
    let v0 = c - a;
    let v1 = b - a;
    let v2 = p - a;
    let dot00 = v0.dot(v0);
    let dot01 = v0.dot(v1);
    let dot02 = v0.dot(v2);
    let dot11 = v1.dot(v1);
    let dot12 = v1.dot(v2);
    let denom = dot00 * dot11 - dot01 * dot01;
    if denom.abs() < 1e-6 {
        return false;
    }
    let inv = 1.0 / denom;
    let u = (dot11 * dot02 - dot01 * dot12) * inv;
    let v = (dot00 * dot12 - dot01 * dot02) * inv;
    u >= -0.02 && v >= -0.02 && (u + v) <= 1.02
}

/// Convex corridor between parent row and flyout (safe corners).
fn in_safe_corridor(p: Pos2, parent: Rect, sub: Rect) -> bool {
    if parent.contains(p) || sub.contains(p) {
        return true;
    }
    let opens_right = sub.center().x >= parent.center().x;
    let (a, b, c, d) = if opens_right {
        (
            pos2(parent.right(), parent.top()),
            pos2(parent.right(), parent.bottom()),
            pos2(sub.left(), sub.top()),
            pos2(sub.left(), sub.bottom()),
        )
    } else {
        (
            pos2(parent.left(), parent.top()),
            pos2(parent.left(), parent.bottom()),
            pos2(sub.right(), sub.top()),
            pos2(sub.right(), sub.bottom()),
        )
    };
    // Convex quad a–b–d–c as two triangles.
    point_in_triangle(p, a, b, d) || point_in_triangle(p, a, d, c)
}

// ── Painting ─────────────────────────────────────────────────────────────────

struct PaintRowOut {
    rect: Rect,
    hovered: bool,
    clicked: bool,
}

struct PanelPaint {
    content_w: f32,
    with_icons: bool,
    show_carets: bool,
    open_sub: Option<usize>,
}

struct RowPaint<'a> {
    content_w: f32,
    with_icons: bool,
    show_carets: bool,
    is_sub: bool,
    icon: Option<&'static str>,
    label: &'a str,
    danger: bool,
    force_active: bool,
}

fn paint_rows<T>(
    ui: &mut egui::Ui, rows: &[Row<T>], theme: &Theme, id_salt: Id, panel: PanelPaint,
) -> Vec<PaintRowOut> {
    let mut out = Vec::with_capacity(rows.len());
    for (i, row) in rows.iter().enumerate() {
        match row {
            Row::Sep => {
                ui.add_space(SEP_V);
                let (rect, _) =
                    ui.allocate_exact_size(vec2(panel.content_w, 1.0), Sense::hover());
                ui.painter().hline(
                    (rect.left() + 6.0)..=(rect.right() - 6.0),
                    rect.center().y,
                    Stroke::new(1.0, theme.neutral()),
                );
                ui.add_space(SEP_V);
                out.push(PaintRowOut { rect, hovered: false, clicked: false });
            }
            Row::Item { icon, label, danger, .. } => {
                out.push(paint_one_row(
                    ui,
                    theme,
                    id_salt.with(i),
                    RowPaint {
                        content_w: panel.content_w,
                        with_icons: panel.with_icons,
                        show_carets: panel.show_carets,
                        is_sub: false,
                        icon: *icon,
                        label,
                        danger: *danger,
                        force_active: false,
                    },
                ));
            }
            Row::Submenu { icon, label, .. } => {
                out.push(paint_one_row(
                    ui,
                    theme,
                    id_salt.with(i),
                    RowPaint {
                        content_w: panel.content_w,
                        with_icons: panel.with_icons,
                        show_carets: panel.show_carets,
                        is_sub: true,
                        icon: *icon,
                        label,
                        danger: false,
                        force_active: panel.open_sub == Some(i),
                    },
                ));
            }
        }
    }
    out
}

fn paint_one_row(
    ui: &mut egui::Ui, theme: &Theme, row_id: Id, p: RowPaint<'_>,
) -> PaintRowOut {
    let (rect, row_resp) = ui.allocate_exact_size(vec2(p.content_w, ROW_H), Sense::click());
    let active = row_resp.hovered() || row_resp.is_pointer_button_down_on() || p.force_active;
    let hover_t = ui.ctx().animate_bool(row_id, active);
    if hover_t > 0.0 {
        let accent = if p.danger { theme.fg().red } else { theme.neutral_fg() };
        let fill = theme
            .neutral_bg()
            .lerp_to_gamma(accent, 0.08 * hover_t.max(0.4));
        ui.painter().rect_filled(rect, ROW_RADIUS, fill);
    }
    let ink = if p.danger {
        theme.fg().red
    } else {
        theme.neutral_fg()
    };

    let text_x = if p.with_icons {
        if let Some(glyph) = p.icon {
            let ig =
                ui.painter()
                    .layout_no_wrap(glyph.into(), phosphor_font(ICON_SIZE), ink);
            let ix = rect.left() + PAD_LEADING + (ICON_SLOT - ig.size().x) / 2.0;
            ui.painter()
                .galley(pos2(ix, rect.center().y - ig.size().y / 2.0), ig, ink);
        }
        rect.left() + PAD_LEADING + ICON_SLOT + ICON_TEXT_GAP
    } else {
        rect.left() + PAD_X_TEXT
    };

    let g = ui
        .painter()
        .layout_no_wrap(p.label.to_owned(), FontId::proportional(LABEL_SIZE), ink);
    ui.painter()
        .galley(pos2(text_x, rect.center().y - g.size().y / 2.0), g, ink);

    if p.show_carets && p.is_sub {
        let muted = theme.neutral_fg().gamma_multiply(0.65);
        let cg = ui
            .painter()
            .layout_no_wrap(CARET_RIGHT.into(), phosphor_font(ICON_SIZE), muted);
        let cx = rect.right() - PAD_TRAILING - cg.size().x;
        ui.painter()
            .galley(pos2(cx, rect.center().y - cg.size().y / 2.0), cg, muted);
    }

    PaintRowOut {
        rect,
        hovered: row_resp.hovered(),
        clicked: row_resp.clicked() && !p.is_sub,
    }
}

// ── Public API ───────────────────────────────────────────────────────────────

/// Right-click menu on `resp`. Returns the chosen leaf value, if any.
pub fn show_menu<T: Clone>(resp: &Response, build: impl FnOnce(&mut MenuEntries<T>)) -> Option<T> {
    let ctx = resp.ctx.clone();
    let host = resp.id;

    let mut menu = MenuEntries::new();
    build(&mut menu);
    if menu.is_empty() {
        return None;
    }

    if resp.secondary_clicked() {
        let pos = resp
            .interact_pointer_pos()
            .unwrap_or_else(|| resp.rect.left_bottom());
        ctx.memory_mut(|m| {
            m.data.insert_temp(
                open_key(),
                OpenState {
                    host,
                    pos,
                    open_sub: None,
                    parent_rect: None,
                    sub_rect: None,
                },
            );
        });
    }

    let mut state = ctx.memory(|m| m.data.get_temp::<OpenState>(open_key()))?;
    if state.host != host {
        return None;
    }

    let just_opened = resp.secondary_clicked();
    let mut chosen: Option<T> = None;
    let chrome = FloatingChrome::from_ctx(&ctx);
    let with_icons = menu.any_icon();
    let theme = ctx.get_lb_theme();
    let pointer = ctx
        .input(|i| i.pointer.interact_pos())
        .or_else(|| ctx.input(|i| i.pointer.hover_pos()));

    // ── Safe-corner gate (using last frame’s rects) ──────────────────────
    let in_corridor = match (pointer, state.parent_rect, state.sub_rect) {
        (Some(p), Some(pr), Some(sr)) => in_safe_corridor(p, pr, sr),
        _ => false,
    };

    // ── Root panel ───────────────────────────────────────────────────────
    let mut root_row_rects: Vec<Rect> = Vec::new();
    let mut hover_root_idx: Option<usize> = None;

    let root_area = Area::new(root_area_key())
        .order(Order::Foreground)
        .fixed_pos(state.pos)
        .constrain(true)
        .default_size(vec2(0.0, 0.0))
        .sense(Sense::hover())
        .show(&ctx, |ui| {
            chrome.menu_frame().show(ui, |ui| {
                ui.with_layout(Layout::top_down(Align::Min), |ui| {
                    ui.spacing_mut().item_spacing = Vec2::ZERO;
                    let content_w = measure_panel_width(ui, &menu.rows, with_icons, true);
                    ui.set_min_width(content_w);
                    ui.set_max_width(content_w);

                    let painted = paint_rows(
                        ui,
                        &menu.rows,
                        &theme,
                        Id::new(("lb_float_root", host)),
                        PanelPaint {
                            content_w,
                            with_icons,
                            show_carets: true,
                            open_sub: state.open_sub,
                        },
                    );

                    for (i, (row, pr)) in menu.rows.iter().zip(painted.iter()).enumerate() {
                        root_row_rects.push(pr.rect);
                        if pr.hovered {
                            hover_root_idx = Some(i);
                        }
                        if pr.clicked {
                            if let Row::Item { value, .. } = row {
                                chosen = Some(value.clone());
                            }
                        }
                    }
                });
            });
        });
    let root_panel_rect = root_area.response.rect;

    // Prefer pointer-based hover over row.hovered when available (more stable).
    if let Some(p) = pointer {
        hover_root_idx = root_row_rects.iter().enumerate().find_map(|(i, r)| {
            if r.contains(p) && matches!(menu.rows.get(i), Some(Row::Item { .. } | Row::Submenu { .. }))
            {
                Some(i)
            } else {
                None
            }
        });
    }

    // ── Decide open submenu for this frame ───────────────────────────────
    let mut open_sub = state.open_sub;
    if let Some(idx) = hover_root_idx {
        match &menu.rows[idx] {
            Row::Submenu { .. } => open_sub = Some(idx),
            Row::Item { .. } if !in_corridor => open_sub = None,
            _ => {}
        }
    } else if let Some(p) = pointer {
        let over_sub = state.sub_rect.is_some_and(|r| r.contains(p));
        let over_root = root_panel_rect.contains(p);
        if !over_sub && !over_root && !in_corridor {
            open_sub = None;
        }
        // else keep open_sub (corridor or over flyout)
    }
    state.open_sub = open_sub;

    // ── Submenu flyout ───────────────────────────────────────────────────
    let mut new_parent_rect: Option<Rect> = None;
    let mut new_sub_rect: Option<Rect> = None;

    if let Some(sub_i) = state.open_sub {
        if let (Some(Row::Submenu { children, .. }), Some(parent_r)) =
            (menu.rows.get(sub_i), root_row_rects.get(sub_i).copied())
        {
            new_parent_rect = Some(parent_r);
            let sub_with_icons = children.any_icon() || with_icons;
            let screen = ctx.input(|i| i.screen_rect);

            // Measure width in a throwaway way: use a short pass via Area.
            // We place right first; if it would clip hard, place left.
            let mut content_w = MIN_W;
            // Approximate from labels without a full Ui measure — refined inside Area.
            for row in &children.rows {
                if let Row::Item { label, .. } | Row::Submenu { label, .. } = row {
                    content_w = content_w.max(MIN_W.max(label.len() as f32 * 7.0 + 48.0));
                }
            }
            content_w = content_w.clamp(MIN_W, MAX_W);

            let open_right = parent_r.right() + SUB_GAP + content_w <= screen.right()
                || parent_r.left() - SUB_GAP - content_w < screen.left();

            let sub_pos = if open_right {
                pos2(parent_r.right() + SUB_GAP, parent_r.top() + SUB_Y_NUDGE)
            } else {
                pos2(
                    parent_r.left() - SUB_GAP - content_w,
                    parent_r.top() + SUB_Y_NUDGE,
                )
            };

            let sub_area = Area::new(sub_area_key())
                .order(Order::Foreground)
                .fixed_pos(sub_pos)
                .constrain(true)
                .default_size(vec2(0.0, 0.0))
                .sense(Sense::hover())
                .show(&ctx, |ui| {
                    chrome.menu_frame().show(ui, |ui| {
                        ui.with_layout(Layout::top_down(Align::Min), |ui| {
                            ui.spacing_mut().item_spacing = Vec2::ZERO;
                            let w = measure_panel_width(ui, &children.rows, sub_with_icons, false);
                            ui.set_min_width(w);
                            ui.set_max_width(w);

                            let painted = paint_rows(
                                ui,
                                &children.rows,
                                &theme,
                                Id::new(("lb_float_sub", host, sub_i)),
                                PanelPaint {
                                    content_w: w,
                                    with_icons: sub_with_icons,
                                    show_carets: false,
                                    open_sub: None,
                                },
                            );
                            for (row, pr) in children.rows.iter().zip(painted.iter()) {
                                if pr.clicked {
                                    if let Row::Item { value, .. } = row {
                                        chosen = Some(value.clone());
                                    }
                                }
                            }
                        });
                    });
                });
            new_sub_rect = Some(sub_area.response.rect);
            let _ = open_right;
        } else {
            state.open_sub = None;
        }
    }

    state.parent_rect = new_parent_rect;
    state.sub_rect = new_sub_rect;

    // ── Close ────────────────────────────────────────────────────────────
    let escape = ctx.input(|i| i.key_pressed(egui::Key::Escape));
    let click_outside = !just_opened
        && ctx.input(|i| i.pointer.any_click())
        && pointer.is_some_and(|p| {
            let on_root = root_panel_rect.contains(p);
            let on_sub = state.sub_rect.is_some_and(|r| r.contains(p));
            !on_root && !on_sub
        });

    if chosen.is_some() || escape || click_outside {
        ctx.memory_mut(|m| {
            m.data.remove::<OpenState>(open_key());
        });
    } else {
        ctx.memory_mut(|m| {
            m.data.insert_temp(open_key(), state);
        });
        ctx.request_repaint();
    }

    chosen
}

// ── Simple text menu ─────────────────────────────────────────────────────────

pub struct TextMenu {
    items: Vec<(String, bool)>,
}

impl TextMenu {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    pub fn item(&mut self, label: impl Into<String>) {
        self.items.push((label.into(), false));
    }

    pub fn item_danger(&mut self, label: impl Into<String>) {
        self.items.push((label.into(), true));
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

impl Default for TextMenu {
    fn default() -> Self {
        Self::new()
    }
}

pub fn show_text_menu(resp: &Response, build: impl FnOnce(&mut TextMenu)) -> Option<usize> {
    let mut simple = TextMenu::new();
    build(&mut simple);
    if simple.is_empty() {
        return None;
    }
    show_menu(resp, |m| {
        for (i, (label, danger)) in simple.items.into_iter().enumerate() {
            if danger {
                m.item_danger(label, i);
            } else {
                m.item(label, i);
            }
        }
    })
}
