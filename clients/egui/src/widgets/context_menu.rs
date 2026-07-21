//! Custom context menu — we do **not** use `Response::context_menu`.
//!
//! egui’s built-in path forces `spacing.menu_width` (default **400**) as the
//! Area’s default width and lays out with `top_down_justified`, so rows either
//! stretch or sit in a wide empty panel. Style tweaks inside the content
//! closure also run *after* `Frame::menu` has already applied `menu_margin`.
//!
//! This widget owns open-state, placement, metrics, and painting so menus size
//! to their labels (macOS-like) without that magic.

use egui::{
    Align, Area, Color32, CornerRadius, FontId, Id, Layout, Order, Pos2, Response, Sense, Stroke,
    Ui, Vec2, pos2, vec2,
};

use crate::theme::icons;
use crate::theme::tokens::Tokens;

// ── macOS-ish metrics (points) ──────────────────────────────────────────────

/// Floor so a lone short action doesn’t look stubby.
const MIN_W: f32 = 112.0;
const MAX_W: f32 = 260.0;

/// Pad inside the row before the icon slot.
const PAD_LEADING: f32 = 6.0;
const ICON_SLOT: f32 = 16.0;
const ICON_SIZE: f32 = 13.0;
const ICON_TEXT_GAP: f32 = 6.0;
/// Trailing pad after the title (no key-equivalent column yet).
const PAD_TRAILING: f32 = 12.0;

const LABEL_SIZE: f32 = 13.0;
/// Row height — a touch roomier than stock 22pt NSMenu for readability.
const ROW_H: f32 = 24.0;
const ROW_RADIUS: u8 = 4;
/// Air above/below soft group rules.
const SEP_V: f32 = 4.0;

/// Temp-data: which host owns the open menu + anchor position.
fn open_id() -> Id {
    Id::new("lb_context_menu_open")
}

/// Stable Area id (one global menu at a time).
fn area_id() -> Id {
    Id::new("lb_context_menu_area")
}

#[derive(Clone, Copy)]
struct OpenState {
    host: Id,
    pos: Pos2,
}

enum Row {
    Item {
        icon: &'static str,
        label: String,
        danger: bool,
    },
    Sep,
}

/// Declarative entry list built by the caller, then measured and painted once.
pub struct Entries<T> {
    rows: Vec<(Row, Option<T>)>,
}

impl<T> Entries<T> {
    fn new() -> Self {
        Self { rows: Vec::new() }
    }

    pub fn item(&mut self, icon: &'static str, label: impl Into<String>, value: T) {
        self.rows.push((
            Row::Item { icon, label: label.into(), danger: false },
            Some(value),
        ));
    }

    pub fn item_danger(&mut self, icon: &'static str, label: impl Into<String>, value: T) {
        self.rows.push((
            Row::Item { icon, label: label.into(), danger: true },
            Some(value),
        ));
    }

    pub fn separator(&mut self) {
        self.rows.push((Row::Sep, None));
    }
}

/// Open / keep / paint a context menu for `resp`.
///
/// - Secondary-click on `resp` opens (or repositions) the menu at the pointer.
/// - Returns `Some(value)` when an item is chosen (menu closes).
/// - Closes on Escape, click outside, or choosing an item.
///
/// Only one menu is open app-wide; right-clicking another host replaces it.
pub fn show<T>(
    resp: &Response,
    t: &Tokens,
    build: impl FnOnce(&mut Entries<T>),
) -> Option<T> {
    let ctx = &resp.ctx;

    // ── open / replace ───────────────────────────────────────────────────
    if resp.secondary_clicked() {
        let pos = ctx
            .pointer_interact_pos()
            .or_else(|| ctx.input(|i| i.pointer.hover_pos()))
            .unwrap_or(resp.rect.left_bottom());
        ctx.memory_mut(|m| {
            m.data.insert_temp(
                open_id(),
                OpenState { host: resp.id, pos },
            );
        });
    }

    let state = ctx.memory(|m| m.data.get_temp::<OpenState>(open_id()))?;
    if state.host != resp.id {
        return None;
    }

    // Build rows first so we can measure the widest label before painting.
    let mut entries = Entries::new();
    build(&mut entries);
    if entries.rows.is_empty() {
        return None;
    }

    let just_opened = resp.secondary_clicked();
    let mut chosen: Option<T> = None;

    // Content-sized Area — do **not** use egui’s 400px menu_width default.
    // `default_size(0,0)` + sizing pass lets min_size follow our rows.
    let area_resp = Area::new(area_id())
        .order(Order::Foreground)
        .fixed_pos(state.pos)
        .constrain(true)
        .default_size(vec2(0.0, 0.0))
        .sense(Sense::hover())
        .show(ctx, |ui| {
            // Shared floating chrome (canvas + line + shadow).
            t.floating().menu_frame().show(ui, |ui| {
                // Critical: *not* justified — rows size to content, menu follows.
                ui.with_layout(Layout::top_down(Align::Min), |ui| {
                    ui.spacing_mut().item_spacing = Vec2::ZERO;

                    // Measure width from labels.
                    let mut content_w = MIN_W;
                    for (row, _) in &entries.rows {
                        if let Row::Item { label, .. } = row {
                            let g = ui.painter().layout_no_wrap(
                                label.clone(),
                                FontId::proportional(LABEL_SIZE),
                                Color32::PLACEHOLDER,
                            );
                            let w = PAD_LEADING
                                + ICON_SLOT
                                + ICON_TEXT_GAP
                                + g.size().x
                                + PAD_TRAILING;
                            content_w = content_w.max(w);
                        }
                    }
                    content_w = content_w.clamp(MIN_W, MAX_W);
                    ui.set_min_width(content_w);
                    ui.set_max_width(content_w);

                    for (i, (row, value)) in entries.rows.into_iter().enumerate() {
                        match row {
                            Row::Sep => {
                                ui.add_space(SEP_V);
                                let (rect, _) =
                                    ui.allocate_exact_size(vec2(content_w, 1.0), Sense::hover());
                                let color = t.line();
                                ui.painter().hline(
                                    (rect.left() + 6.0)..=(rect.right() - 6.0),
                                    rect.center().y,
                                    Stroke::new(1.0, color),
                                );
                                ui.add_space(SEP_V);
                            }
                            Row::Item { icon, label, danger } => {
                                let clicked = paint_item(
                                    ui,
                                    t,
                                    content_w,
                                    icon,
                                    &label,
                                    danger,
                                    Id::new(("lb_ctx_item", state.host, i)),
                                );
                                if clicked {
                                    chosen = value;
                                }
                            }
                        }
                    }
                });
            });
        });

    let menu_rect = area_resp.response.rect;

    // ── close conditions ─────────────────────────────────────────────────
    let escape = ctx.input(|i| i.key_pressed(egui::Key::Escape));
    let click_outside = !just_opened
        && ctx.input(|i| i.pointer.any_click())
        && ctx
            .input(|i| i.pointer.interact_pos())
            .is_some_and(|p| !menu_rect.contains(p));

    if chosen.is_some() || escape || click_outside {
        ctx.memory_mut(|m| {
            m.data.remove::<OpenState>(open_id());
        });
    }

    chosen
}

fn paint_item(
    ui: &mut Ui,
    t: &Tokens,
    width: f32,
    icon: &str,
    label: &str,
    danger: bool,
    id: Id,
) -> bool {
    let (rect, _) = ui.allocate_exact_size(vec2(width, ROW_H), Sense::hover());
    // Stable id so hover animation doesn’t thrash across rebuilds.
    let resp = ui.interact(rect, id, Sense::click());

    let hover = ui.ctx().animate_bool(resp.id, resp.hovered());
    let pressed = resp.is_pointer_button_down_on();

    let (fill, ink, icon_ink) = if danger {
        let red = t.danger();
        let fill = if pressed {
            t.surface_raised()
        } else if hover > 0.0 {
            t.surface().lerp_to_gamma(t.surface_raised(), hover)
        } else {
            Color32::TRANSPARENT
        };
        (fill, red, red)
    } else {
        // Hover wash over canvas — palette ends only.
        let fill = if pressed {
            t.surface_raised()
        } else if hover > 0.0 {
            t.canvas().lerp_to_gamma(t.surface_raised(), hover)
        } else {
            Color32::TRANSPARENT
        };
        // Icon same weight as label — muted was too quiet next to full ink text.
        (fill, t.fg(), t.fg())
    };

    if fill.a() > 0 {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(ROW_RADIUS), fill);
    }

    let cy = rect.center().y;
    let icon_g = ui
        .painter()
        .layout_no_wrap(icon.into(), icons::font(ICON_SIZE), icon_ink);
    let icon_left = rect.left() + PAD_LEADING;
    let icon_x = icon_left + (ICON_SLOT - icon_g.size().x).max(0.0) * 0.5;
    ui.painter()
        .galley(pos2(icon_x, cy - icon_g.size().y / 2.0), icon_g, icon_ink);

    let label_g = ui.painter().layout_no_wrap(
        label.into(),
        FontId::proportional(LABEL_SIZE),
        ink,
    );
    let label_x = icon_left + ICON_SLOT + ICON_TEXT_GAP;
    ui.painter()
        .galley(pos2(label_x, cy - label_g.size().y / 2.0), label_g, ink);

    resp.clicked()
}
