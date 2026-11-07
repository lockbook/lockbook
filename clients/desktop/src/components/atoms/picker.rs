//! Single-value menu picker — color theme, landing page, etc.
//!
//! Trigger is a **canvas plate** quiet control ([`quiet_canvas_fills`] +
//! [`interact_fill`]). Menu is a canvas plate too. Keyboard: focus the
//! trigger, Space/Enter open, arrows move highlight, Enter commits, Esc closes.

use egui::{Color32, Key, Modifiers, Popup, PopupCloseBehavior, Response, Stroke, Ui, pos2, vec2};

use crate::components::foundation::chrome::{
    Radius, STROKE_HAIRLINE, canvas_overlay_frame, control_height, phosphor, phosphor_ui_font_id,
};
use crate::components::foundation::color::{STROKE_EMPHASIS, Theme};
use crate::components::foundation::interact::{
    canvas_selected_fills, interact_fill, quiet_canvas_fills,
};
use crate::components::foundation::space::Space;
use crate::components::foundation::space::control as control_space;
use crate::components::foundation::typography::TypeRole;

/// Compact picker (form trailing slot or standalone).
///
/// Shows `options[*selected]` with a caret; click or Space/Enter opens a menu.
/// Marks the response `.changed()` when the selection updates.
pub fn picker(ui: &mut Ui, t: &Theme, options: &[&str], selected: &mut usize) -> Response {
    let fonts = vec![OptionFont::Ui(TypeRole::Body.font_id()); options.len()];
    picker_fonts(ui, t, options, &fonts, None, selected)
}

/// Widest a closed picker grows, so a long option name ends in an ellipsis.
const MAX_W: f32 = Space::Xl.pts() * 6.0;

/// What a [`picker_fonts`] option draws in.
#[derive(Clone)]
pub enum OptionFont {
    Ui(egui::FontId),
    /// An installed family, loaded when its row first shows.
    Family(&'static str),
}

/// [`picker`] with each option drawn in its own font, and a rule under
/// option `divider_after`.
pub fn picker_fonts(
    ui: &mut Ui, t: &Theme, options: &[&str], fonts: &[OptionFont], divider_after: Option<usize>,
    selected: &mut usize,
) -> Response {
    let n = options.len().max(1);
    *selected = (*selected).min(n - 1);
    let label = options.get(*selected).copied().unwrap_or("—");

    let font = TypeRole::Body.font_id();
    let option_font = |i: usize| {
        fonts
            .get(i)
            .cloned()
            .unwrap_or(OptionFont::Ui(font.clone()))
    };
    let caret_g = ui.painter().layout_no_wrap(
        phosphor::CARET_DOWN.to_owned(),
        phosphor_ui_font_id(),
        Color32::PLACEHOLDER,
    );
    let gap = control_space::ICON_GAP.pts();
    let pad_x = control_space::PAD_X.pts();
    // a long name ends in an ellipsis inside the width the parent gives
    let max_w = crate::components::ui_width(ui).min(MAX_W);
    let label_max = (max_w - gap - caret_g.size().x - pad_x * 2.0).max(1.0);
    let label_g = OptionLabel::measure(ui, t, label, &option_font(*selected), label_max);
    let h = control_height();
    let min_w = Space::Xl.pts() * 3.0;
    let w = (label_g.width() + gap + caret_g.size().x + pad_x * 2.0).max(min_w);

    let (rect, mut resp) = ui.allocate_exact_size(vec2(w, h), crate::components::sense_click());
    if resp.clicked() {
        resp.request_focus();
    }

    let popup_id = Popup::default_response_id(&resp);
    let mut open = Popup::is_id_open(ui.ctx(), popup_id);
    let highlight_key = popup_id.with("hl");

    // Keyboard on the focused trigger (when menu closed).
    if resp.has_focus() && !open {
        let open_key = ui.ctx().input_mut(|i| {
            i.consume_key(Modifiers::NONE, Key::Enter)
                || i.consume_key(Modifiers::NONE, Key::Space)
                || i.consume_key(Modifiers::NONE, Key::ArrowDown)
        });
        if open_key {
            Popup::open_id(ui.ctx(), popup_id);
            open = true;
            ui.ctx()
                .data_mut(|d| d.insert_temp(highlight_key, *selected));
        }
    }

    // A list taller than the room below the trigger scrolls, and typing filters it.
    let row_h = control_height();
    let menu_pad = Space::Xxs.pts();
    let content = ui.ctx().content_rect();
    let below = content.bottom() - rect.bottom() - Space::Lg.pts() - menu_pad * 2.0;
    let above = rect.top() - content.top() - Space::Lg.pts() - menu_pad * 2.0;
    let long = n as f32 * row_h > below;
    // a long list opens toward the larger room
    let up = long && above > below;
    let room = if up { above } else { below };
    let query_key = popup_id.with("query");
    let query: String = ui
        .ctx()
        .data(|d| d.get_temp::<String>(query_key))
        .unwrap_or_default()
        .to_lowercase();
    let shown: Vec<usize> = (0..options.len())
        .filter(|&i| options[i].to_lowercase().contains(&query))
        .collect();

    // A menu opened this frame, or whose options just arrived, scrolls its
    // selection into view.
    let count_key = popup_id.with("n");
    let grew = open
        && ui
            .ctx()
            .data(|d| d.get_temp::<usize>(count_key))
            .is_some_and(|n| n != options.len());
    if grew {
        ui.ctx().data_mut(|d| d.remove_temp::<usize>(highlight_key));
    }
    ui.ctx()
        .data_mut(|d| d.insert_temp(count_key, options.len()));
    let fresh = open
        && ui
            .ctx()
            .data(|d| d.get_temp::<usize>(highlight_key))
            .is_none();

    // Keyboard while menu open (arrows / enter / esc).
    let mut pick_from_keys: Option<usize> = None;
    let mut close_menu = false;
    let mut moved = false;
    if open {
        let hl = ui
            .ctx()
            .data(|d| d.get_temp::<usize>(highlight_key))
            .unwrap_or(*selected);
        let mut at = shown.iter().position(|&i| i == hl).unwrap_or(0);
        let m = shown.len();
        ui.ctx().input_mut(|i| {
            if i.consume_key(Modifiers::NONE, Key::Escape) {
                close_menu = true;
            } else if m > 0 && i.consume_key(Modifiers::NONE, Key::ArrowDown) {
                at = (at + 1) % m;
                moved = true;
            } else if m > 0 && i.consume_key(Modifiers::NONE, Key::ArrowUp) {
                at = if at == 0 { m - 1 } else { at - 1 };
                moved = true;
            } else if i.consume_key(Modifiers::NONE, Key::Enter)
                || (!long && i.consume_key(Modifiers::NONE, Key::Space))
            {
                pick_from_keys = shown.get(at).copied();
            }
        });
        if close_menu {
            Popup::close_id(ui.ctx(), popup_id);
            open = false;
            ui.ctx().data_mut(|d| d.remove_temp::<usize>(highlight_key));
        } else if let Some(&hl) = shown.get(at) {
            ui.ctx().data_mut(|d| d.insert_temp(highlight_key, hl));
        }
    }

    // Quiet canvas interaction — same stack as quiet buttons.
    let pointer_over = ui.ctx().rect_contains_pointer(ui.layer_id(), rect) || open;
    let fills = quiet_canvas_fills(t);
    let fill = interact_fill(
        ui.ctx(),
        resp.id,
        pointer_over,
        resp.is_pointer_button_down_on(),
        resp.clicked(),
        fills,
    );
    let radius = Radius::Control.corner();
    ui.painter().rect_filled(rect, radius, fill);

    let stroke_on = pointer_over || open || resp.has_focus();
    let stroke_t = ui.ctx().animate_bool(resp.id.with("stroke"), stroke_on);
    ui.painter().rect_stroke(
        rect,
        radius,
        Stroke::new(
            STROKE_HAIRLINE,
            t.neutral()
                .lerp_to_gamma(t.neutral_fg(), STROKE_EMPHASIS * stroke_t),
        ),
        egui::StrokeKind::Inside,
    );

    if resp.has_focus() {
        ui.painter().rect_stroke(
            rect,
            radius,
            Stroke::new(STROKE_HAIRLINE, t.neutral_fg()),
            egui::StrokeKind::Outside,
        );
    }

    let hover_t = ui
        .ctx()
        .animate_bool(resp.id.with("ink"), pointer_over || open);
    let caret_ink = t
        .neutral_fg_secondary()
        .lerp_to_gamma(t.neutral_fg(), hover_t);
    let cy = rect.center().y;
    let mut x = rect.left() + pad_x;
    label_g.paint(ui, t, x, cy, ui.clip_rect());
    x = rect.right() - pad_x - caret_g.size().x;
    ui.painter()
        .galley(pos2(x, cy - caret_g.size().y / 2.0), caret_g, caret_ink);

    // Seed highlight when opened by click (toggle path).
    if open && resp.clicked() {
        ui.ctx()
            .data_mut(|d| d.insert_temp(highlight_key, *selected));
    }

    let highlight = ui
        .ctx()
        .data(|d| d.get_temp::<usize>(highlight_key))
        .unwrap_or(*selected)
        .min(n - 1);

    let mut menu_w = w.max(Space::Xl.pts() * 4.0);
    if open && long {
        let widest = options
            .iter()
            .enumerate()
            .map(|(i, o)| {
                // a family is measured in the body font, as loading each would stall
                let f = match fonts.get(i) {
                    Some(OptionFont::Ui(f)) => f.clone(),
                    _ => font.clone(),
                };
                ui.painter()
                    .layout_no_wrap(o.to_string(), f, t.neutral_fg())
                    .size()
                    .x
            })
            .fold(0.0, f32::max);
        menu_w = menu_w.max((widest + control_space::PAD_X.pts() * 2.0).min(MAX_W));
    }
    let mut changed = false;
    let mut pick = pick_from_keys;

    if let Some(inner) = Popup::from_toggle_button_response(&resp)
        .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
        .align(match (long, up) {
            (true, true) => egui::RectAlign::TOP_END,
            (true, false) => egui::RectAlign::BOTTOM_END,
            _ => egui::RectAlign::BOTTOM_START,
        })
        .width(menu_w)
        .frame(canvas_overlay_frame(t, Space::Xxs))
        .show(|ui| {
            ui.set_min_width(menu_w);
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            let rows = |ui: &mut Ui| {
                let mut click_pick = None;
                if shown.is_empty() {
                    let (rect, _) = ui.allocate_exact_size(
                        vec2(crate::components::ui_width(ui).max(1.0), row_h),
                        egui::Sense::hover(),
                    );
                    let g = ui.painter().layout_no_wrap(
                        "No matches".into(),
                        font.clone(),
                        t.neutral_fg_secondary(),
                    );
                    let at = pos2(rect.left() + control_space::PAD_X.pts(), rect.center().y);
                    ui.painter().galley(
                        at - vec2(0., g.size().y / 2.),
                        g,
                        t.neutral_fg_secondary(),
                    );
                }
                for &i in &shown {
                    let (clicked, row) =
                        menu_row(ui, t, options[i], option_font(i), i == *selected, i == highlight);
                    if clicked {
                        click_pick = Some(i);
                    }
                    if i == highlight && (moved || resp.clicked() || fresh) {
                        ui.scroll_to_rect(row, None);
                    }
                    if divider_after == Some(i) && query.is_empty() {
                        let (rule, _) = ui.allocate_exact_size(
                            vec2(crate::components::ui_width(ui).max(1.0), menu_pad * 2.0 + 1.0),
                            egui::Sense::hover(),
                        );
                        ui.painter().hline(
                            rule.x_range().shrink(control_space::PAD_X.pts()),
                            rule.center().y,
                            Stroke::new(STROKE_HAIRLINE, t.neutral()),
                        );
                    }
                }
                click_pick
            };
            if !long {
                return rows(ui);
            }
            let field = popup_id.with("field");
            let mut typed = ui
                .ctx()
                .data(|d| d.get_temp::<String>(query_key))
                .unwrap_or_default();
            workspace_rs::style::Field::new(t, &mut typed)
                .id(field)
                .hint("Search")
                .leading(phosphor::SEARCH)
                .clearable(true)
                .width(crate::components::ui_width(ui))
                .show(ui);
            if resp.clicked() || resp.has_focus() {
                ui.memory_mut(|m| m.request_focus(field.with("edit")));
            }
            ui.ctx().data_mut(|d| d.insert_temp(query_key, typed));
            ui.add(workspace_rs::style::Spacer::new(Space::Xxs));
            let max_h = (room - row_h - Space::Xxs.pts()).max(row_h * 3.0);
            workspace_rs::style::with_overlay_scroll(ui, popup_id.with("scroll"), |ui| {
                let out = egui::ScrollArea::vertical()
                    .id_salt(popup_id.with("scroll"))
                    .max_height(max_h)
                    .min_scrolled_height(max_h)
                    .auto_shrink([false, true])
                    .show(ui, rows);
                (out.inner, out.state.clamped_offset().y, out.id)
            })
        })
    {
        // The menu sits right above its sheet, even when a tap raises both in one frame.
        let menu_layer = inner.response.layer_id;
        if menu_layer.order == ui.layer_id().order {
            ui.ctx().set_sublayer(ui.layer_id(), menu_layer);
        }
        if let Some(i) = inner.inner {
            pick = Some(i);
        }
    }

    if let Some(i) = pick {
        if i != *selected {
            *selected = i;
            changed = true;
        }
        Popup::close_id(ui.ctx(), popup_id);
        ui.ctx().data_mut(|d| d.remove_temp::<usize>(highlight_key));
    }

    if !Popup::is_id_open(ui.ctx(), popup_id) {
        ui.ctx().data_mut(|d| {
            d.remove_temp::<usize>(highlight_key);
            d.remove_temp::<String>(query_key);
        });
    }

    if changed {
        resp.mark_changed();
    }
    resp
}

/// Menu option on canvas. Idle `fg` (available). `selected` = value wash;
/// `highlighted` = keyboard cursor (hover wash if not selected).
fn menu_row(
    ui: &mut Ui, t: &Theme, label: &str, font: OptionFont, selected: bool, highlighted: bool,
) -> (bool, egui::Rect) {
    let h = control_height();
    let (rect, resp) = ui.allocate_exact_size(
        vec2(crate::components::ui_width(ui).max(1.0), h),
        crate::components::sense_click(),
    );
    let over = ui.ctx().rect_contains_pointer(ui.layer_id(), rect) || highlighted;

    let fills = if selected { canvas_selected_fills(t) } else { quiet_canvas_fills(t) };
    let fill = interact_fill(
        ui.ctx(),
        resp.id,
        over,
        resp.is_pointer_button_down_on(),
        resp.clicked(),
        fills,
    );
    ui.painter().rect_filled(rect, Radius::Sm.corner(), fill);

    if ui.is_rect_visible(rect) {
        let x = rect.left() + control_space::PAD_X.pts();
        let max_w = rect.right() - control_space::PAD_X.pts() - x;
        OptionLabel::measure(ui, t, label, &font, max_w).paint(
            ui,
            t,
            x,
            rect.center().y,
            rect.intersect(ui.clip_rect()),
        );
    }

    (resp.clicked(), rect)
}

/// An option's label shaped in its font, ready to place.
enum OptionLabel {
    Ui(std::sync::Arc<egui::Galley>),
    Family(workspace_rs::widgets::ShapedLabel),
}

impl OptionLabel {
    /// `label` in `font`, cut short with an ellipsis past `max_w`.
    fn measure(ui: &Ui, t: &Theme, label: &str, font: &OptionFont, max_w: f32) -> Self {
        let ui_label = |font: egui::FontId| {
            let mut job = egui::text::LayoutJob::single_section(
                label.to_owned(),
                egui::TextFormat::simple(font, t.neutral_fg()),
            );
            job.wrap = egui::text::TextWrapping::truncate_at_width(max_w);
            Self::Ui(ui.fonts_mut(|f| f.layout_job(job)))
        };
        match font {
            OptionFont::Ui(font) => ui_label(font.clone()),
            OptionFont::Family(family) => {
                // the name shows in the interface font until its family is read
                let Some(adjust) = workspace_rs::font::load_family(ui.ctx(), family) else {
                    return ui_label(TypeRole::Body.font_id());
                };
                let size = TypeRole::Body.size() * adjust;
                Self::Family(
                    workspace_rs::widgets::GlyphonLabel::new(label, t.neutral_fg())
                        .family(family)
                        .font_size(size)
                        .line_height(TypeRole::Body.line_height().max(size * 1.4))
                        .max_width(max_w)
                        .text_overflow(workspace_rs::widgets::TextOverflow::EndEllipsis)
                        .build(ui.ctx()),
                )
            }
        }
    }

    fn width(&self) -> f32 {
        match self {
            Self::Ui(g) => g.size().x,
            Self::Family(shaped) => shaped.size.x,
        }
    }

    /// Paints from `x` with its baseline where body text centered on `cy`
    /// has its own.
    fn paint(self, ui: &Ui, t: &Theme, x: f32, cy: f32, clip: egui::Rect) {
        match self {
            Self::Ui(g) => {
                let y = baseline_top(ui, cy, &g);
                ui.painter().galley(pos2(x, y), g, t.neutral_fg());
            }
            Self::Family(shaped) => {
                let body = ui.painter().layout_no_wrap(
                    "Hg".into(),
                    TypeRole::Body.font_id(),
                    Color32::PLACEHOLDER,
                );
                let target = cy - body.size().y / 2.0 + galley_baseline(&body);
                let top = target - shaped.baseline(ui.ctx()).unwrap_or(0.0);
                let at = egui::Rect::from_min_size(pos2(x, top), shaped.size);
                let area = shaped.text_area(at, ui.ctx(), clip);
                ui.painter()
                    .add(egui_wgpu_renderer::egui_wgpu::Callback::new_paint_callback(
                        clip,
                        workspace_rs::GlyphonRendererCallback::new(vec![area]),
                    ));
            }
        }
    }
}

/// A galley's first baseline, from its top.
fn galley_baseline(g: &egui::Galley) -> f32 {
    g.rows
        .first()
        .map_or(0.0, |r| r.pos.y + r.row.glyphs.first().map_or(0.0, |gl| gl.pos.y))
}

/// Where `g` goes so its baseline sits where body text centered on `cy`
/// would have its own, whatever the font's line box.
fn baseline_top(ui: &Ui, cy: f32, g: &egui::Galley) -> f32 {
    let body =
        ui.painter()
            .layout_no_wrap("Hg".into(), TypeRole::Body.font_id(), Color32::PLACEHOLDER);
    cy - body.size().y / 2.0 + galley_baseline(&body) - galley_baseline(g)
}
