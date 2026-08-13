//! Custom context menu — we do **not** use `Response::context_menu`.
//!
//! egui’s built-in path forces `spacing.menu_width` (default **400**) and
//! justified rows. This owns open-state and content sizing.
//!
//! Layout metrics use design tokens (space / radius / type / control height).
//! Placement-only knobs (min/max width, open nudge) stay local.

use egui::{
    Align, Area, Color32, FontFamily, FontId, Frame, Id, Layout, Margin, Order, Pos2, Response,
    Sense, Stroke, Ui, Vec2, pos2, vec2,
};
use std::sync::Arc;

use crate::components::foundation::chrome::{Radius, STROKE_HAIRLINE, control_height};
use crate::components::foundation::color::{FG_HOVER, Theme};
use crate::components::foundation::space::Space;
use crate::components::foundation::space::control as control_space;
use crate::components::foundation::spacer::Spacer;
use crate::components::foundation::typography::TypeRole;

// Placement bounds (not spacing language).
const MIN_W: f32 = 112.0;
const MAX_W: f32 = 260.0;

fn pad_leading() -> f32 {
    Space::Xs.pts()
}
fn pad_trailing() -> f32 {
    Space::Sm.pts()
}
fn icon_slot() -> f32 {
    TypeRole::Body.size()
}
fn icon_size() -> f32 {
    TypeRole::Body.size()
}
fn icon_text_gap() -> f32 {
    control_space::ICON_GAP.pts()
}
fn menu_pad() -> i8 {
    Space::Xs.pts() as i8
}
/// Root menu top-left relative to press (pad, not first row).
fn open_nudge() -> Vec2 {
    vec2(Space::Xxs.pts(), Space::Xs.pts())
}

fn open_id() -> Id {
    Id::new("lb_design_context_menu_open")
}
fn area_id() -> Id {
    Id::new("lb_design_context_menu_area")
}

#[derive(Clone, Copy)]
struct OpenState {
    host: Id,
    pos: Pos2,
}

enum Row<T> {
    Item { icon: Option<&'static str>, label: String, danger: bool, value: T },
    Sep,
}

/// Declarative entry list (items and separators).
pub struct Entries<T> {
    rows: Vec<Row<T>>,
}

impl<T> Entries<T> {
    pub fn new() -> Self {
        Self { rows: Vec::new() }
    }

    pub fn item(&mut self, icon: &'static str, label: impl Into<String>, value: T) {
        self.rows
            .push(Row::Item { icon: Some(icon), label: label.into(), danger: false, value });
    }

    pub fn item_danger(&mut self, icon: &'static str, label: impl Into<String>, value: T) {
        self.rows
            .push(Row::Item { icon: Some(icon), label: label.into(), danger: true, value });
    }

    pub fn separator(&mut self) {
        self.rows.push(Row::Sep);
    }

    /// Drop leading/trailing seps and collapse runs so empty sections don't
    /// paint double hairlines (e.g. multi-select with no outbound items).
    fn normalize_separators(&mut self) {
        let mut out = Vec::with_capacity(self.rows.len());
        let mut pending_sep = false;
        let mut saw_content = false;
        for row in self.rows.drain(..) {
            match row {
                Row::Sep => {
                    if saw_content {
                        pending_sep = true;
                    }
                }
                other => {
                    if pending_sep {
                        out.push(Row::Sep);
                        pending_sep = false;
                    }
                    out.push(other);
                    saw_content = true;
                }
            }
        }
        self.rows = out;
    }

    pub fn is_empty(&self) -> bool {
        !self.rows.iter().any(|r| matches!(r, Row::Item { .. }))
    }

    fn any_icon(&self) -> bool {
        self.rows
            .iter()
            .any(|r| matches!(r, Row::Item { icon: Some(_), .. }))
    }
}

impl<T> Default for Entries<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Open / keep / paint a context menu for `resp`.
///
/// - Secondary-click opens near the pointer, **nudged** so the press is not on
///   the first row.
/// - Returns `Some(value)` when an item is chosen (not on the open frame).
/// - Closes on Escape, click outside, or choose.
///
/// Only one menu is open app-wide; right-clicking another host replaces it.
pub fn show<T: Clone>(
    resp: &Response, t: &Theme, build: impl FnOnce(&mut Entries<T>),
) -> Option<T> {
    let ctx = &resp.ctx;

    let mut menu = Entries::new();
    build(&mut menu);
    menu.normalize_separators();
    if menu.is_empty() {
        return None;
    }

    if resp.secondary_clicked() {
        let press = ctx
            .pointer_interact_pos()
            .or_else(|| ctx.input(|i| i.pointer.hover_pos()))
            .unwrap_or(resp.rect.left_bottom());
        // Not under the cursor tip — pad/chrome first, then rows.
        let pos = press + open_nudge();
        ctx.memory_mut(|m| {
            m.data
                .insert_temp(open_id(), OpenState { host: resp.id, pos });
        });
    }

    let state = ctx.memory(|m| m.data.get_temp::<OpenState>(open_id()))?;
    if state.host != resp.id {
        return None;
    }

    let just_opened = resp.secondary_clicked();
    let mut chosen: Option<T> = None;
    let with_icons = menu.any_icon();
    let pointer = ctx
        .input(|i| i.pointer.interact_pos())
        .or_else(|| ctx.input(|i| i.pointer.hover_pos()));

    let root_area = Area::new(area_id())
        .order(Order::Foreground)
        .fixed_pos(state.pos)
        .constrain(true)
        .default_size(vec2(0.0, 0.0))
        .sense(Sense::hover())
        .show(ctx, |ui| {
            menu_frame(t).show(ui, |ui| {
                ui.with_layout(Layout::top_down(Align::Min), |ui| {
                    ui.spacing_mut().item_spacing = Vec2::ZERO;
                    let content_w = measure_panel_width(ui, &menu.rows, with_icons);
                    ui.set_min_width(content_w);
                    ui.set_max_width(content_w);

                    let painted = paint_rows(
                        ui,
                        t,
                        &menu.rows,
                        Id::new(("lb_ctx_root", state.host)),
                        content_w,
                        with_icons,
                    );

                    for (row, pr) in menu.rows.iter().zip(painted.iter()) {
                        // Ignore the open frame’s click (pointer still down / same event).
                        if pr.clicked && !just_opened {
                            if let Row::Item { value, .. } = row {
                                chosen = Some(value.clone());
                            }
                        }
                    }
                });
            });
        });
    let root_panel_rect = root_area.response.rect;

    let escape = ctx.input(|i| i.key_pressed(egui::Key::Escape));
    let click_outside = !just_opened
        && ctx.input(|i| i.pointer.any_click())
        && pointer.is_some_and(|p| !root_panel_rect.contains(p));

    if chosen.is_some() || escape || click_outside {
        ctx.memory_mut(|m| {
            m.data.remove::<OpenState>(open_id());
        });
    } else {
        ctx.memory_mut(|m| {
            m.data.insert_temp(open_id(), state);
        });
        ctx.request_repaint();
    }

    chosen
}

fn measure_panel_width<T>(ui: &Ui, rows: &[Row<T>], with_icons: bool) -> f32 {
    let mut content_w = MIN_W;
    let lead = pad_leading();
    let trail = pad_trailing();
    let gap = icon_text_gap();
    let slot = icon_slot();
    for row in rows {
        let (icon, label) = match row {
            Row::Item { icon, label, .. } => (*icon, label.as_str()),
            Row::Sep => continue,
        };
        let g = ui.painter().layout_no_wrap(
            label.to_owned(),
            TypeRole::Body.font_id(),
            Color32::PLACEHOLDER,
        );
        let w = if with_icons || icon.is_some() {
            lead + slot + gap + g.size().x + trail
        } else {
            lead + g.size().x + trail
        };
        content_w = content_w.max(w);
    }
    content_w.clamp(MIN_W, MAX_W)
}

struct PaintRowOut {
    clicked: bool,
}

fn paint_rows<T>(
    ui: &mut Ui, t: &Theme, rows: &[Row<T>], id_salt: Id, content_w: f32, with_icons: bool,
) -> Vec<PaintRowOut> {
    let mut out = Vec::with_capacity(rows.len());
    for (i, row) in rows.iter().enumerate() {
        match row {
            Row::Sep => {
                let inset = Space::Xs.pts();
                ui.add(Spacer::new(Space::Xxs));
                let (rect, _) =
                    ui.allocate_at_least(vec2(content_w, STROKE_HAIRLINE), Sense::hover());
                ui.painter().hline(
                    (rect.left() + inset)..=(rect.right() - inset),
                    rect.center().y,
                    Stroke::new(STROKE_HAIRLINE, t.neutral()),
                );
                ui.add(Spacer::new(Space::Xxs));
                out.push(PaintRowOut { clicked: false });
            }
            Row::Item { icon, label, danger, .. } => {
                out.push(paint_one_row(
                    ui,
                    t,
                    id_salt.with(i),
                    content_w,
                    with_icons,
                    *icon,
                    label,
                    *danger,
                ));
            }
        }
    }
    out
}

fn paint_one_row(
    ui: &mut Ui, t: &Theme, row_id: Id, content_w: f32, with_icons: bool,
    icon: Option<&'static str>, label: &str, danger: bool,
) -> PaintRowOut {
    let (rect, _) = ui.allocate_at_least(vec2(content_w, control_height()), Sense::hover());
    let resp = ui.interact(rect, row_id, Sense::click());

    let pointer_over = ui
        .input(|i| i.pointer.interact_pos())
        .is_some_and(|p| rect.contains(p));
    let active = pointer_over || resp.is_pointer_button_down_on() || resp.clicked();
    let hover_t = ui.ctx().animate_bool(row_id.with("hov"), active);

    if hover_t > 0.0 {
        let target = if danger {
            t.neutral_bg().lerp_to_gamma(t.danger(), FG_HOVER)
        } else {
            t.wash_toward_neutral_fg(t.neutral_bg(), FG_HOVER)
        };
        let fill = t.neutral_bg().lerp_to_gamma(target, hover_t);
        let wash = rect.shrink(crate::components::foundation::chrome::row_wash_inset());
        ui.painter().rect_filled(wash, Radius::Sm.corner(), fill);
    }

    let ink = if danger { t.danger() } else { t.neutral_fg() };
    let cy = rect.center().y;
    let lead = pad_leading();
    let slot = icon_slot();
    let gap = icon_text_gap();
    let icon_font = FontId::new(icon_size(), FontFamily::Name(Arc::from("phosphor")));

    let text_x = if with_icons {
        if let Some(glyph) = icon {
            let ig = ui
                .painter()
                .layout_no_wrap(glyph.into(), icon_font, Color32::PLACEHOLDER);
            let ix = rect.left() + lead + (slot - ig.size().x).max(0.0) * 0.5;
            ui.painter()
                .galley(pos2(ix, cy - ig.size().y / 2.0), ig, ink);
        }
        rect.left() + lead + slot + gap
    } else {
        rect.left() + lead
    };

    let g = ui.painter().layout_no_wrap(
        label.to_owned(),
        TypeRole::Body.font_id(),
        Color32::PLACEHOLDER,
    );
    ui.painter()
        .galley(pos2(text_x, cy - g.size().y / 2.0), g, ink);

    PaintRowOut { clicked: resp.clicked() }
}

fn menu_frame(t: &Theme) -> Frame {
    let pad = menu_pad();
    crate::components::foundation::chrome::canvas_overlay_frame(t, Space::Xxs)
        .inner_margin(Margin::symmetric(pad, pad))
}
