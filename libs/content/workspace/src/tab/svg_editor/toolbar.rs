use std::ops::RangeInclusive;

use egui::{emath::RectTransform, Color32, InnerResponse, Response};
use egui_animation::{animate_eased, easing};
use resvg::usvg::Transform;

use crate::{
    theme::{icons::Icon, palette::ThemePalette},
    widgets::Button,
};

use super::{
    eraser::DEFAULT_ERASER_RADIUS,
    gesture_handler::{zoom_percentage_to_transform, GestureHandler},
    history::History,
    parser,
    pen::HIGHLIGHTER_STROKE_WIDTHS,
    pen::PEN_STROKE_WIDTHS,
    selection::Selection,
    Buffer, Eraser, Pen,
};

const COLOR_SWATCH_BTN_RADIUS: f32 = 11.0;
const THICKNESS_BTN_X_MARGIN: f32 = 5.0;
const THICKNESS_BTN_WIDTH: f32 = 25.0;

#[derive(Default)]
pub struct Toolbar {
    pub active_tool: Tool,
    right_tab_rect: Option<egui::Rect>,
    pub pen: Pen,
    pub highlighter: Pen,
    pub eraser: Eraser,
    pub selection: Selection,
    pub previous_tool: Option<Tool>,
    pub gesture_handler: GestureHandler,
    pub show_tool_controls: bool,
    layout: ToolbarLayout,
}

#[derive(Default)]
struct ToolbarLayout {
    tools_island: Option<egui::Rect>,
    history_island: Option<egui::Rect>,
    tool_controls: Option<egui::Rect>,
}
#[derive(PartialEq, Eq, Copy, Clone, Debug, Default)]
pub enum Tool {
    #[default]
    Pen,
    Eraser,
    Selection,
    Highlighter,
}

pub struct ToolContext<'a> {
    pub painter: &'a egui::Painter,
    pub buffer: &'a mut Buffer,
    pub history: &'a mut History,
    pub allow_viewport_changes: &'a mut bool,
    pub is_touch_frame: bool,
}
#[derive(Clone)]
pub struct ColorSwatch {
    pub id: Option<String>,
    pub color: egui::Color32,
}

macro_rules! set_tool {
    ($obj:expr, $new_tool:expr) => {
        if $obj.active_tool != $new_tool {
            $obj.show_tool_controls = false;
            $obj.layout.tool_controls = None;

            if (matches!($new_tool, Tool::Selection)) {
                $obj.selection = Selection::default();
            }
            $obj.previous_tool = Some($obj.active_tool);
            $obj.active_tool = $new_tool;
        } else {
            $obj.show_tool_controls = true;
        }
    };
}

const FIT_ZOOM: i32 = -1;

impl Toolbar {
    pub fn set_tool(&mut self, new_tool: Tool) {
        set_tool!(self, new_tool);
    }

    pub fn toggle_tool_between_eraser(&mut self) {
        let new_tool = if self.active_tool == Tool::Eraser {
            self.previous_tool.unwrap_or(Tool::Pen)
        } else {
            Tool::Eraser
        };

        self.set_tool(new_tool);
    }

    pub fn new() -> Self {
        let mut toolbar = Toolbar {
            pen: Pen::new(ThemePalette::get_fg_color(), PEN_STROKE_WIDTHS[0]),
            highlighter: Pen::new(get_highlighter_colors()[0], HIGHLIGHTER_STROKE_WIDTHS[0]),
            ..Default::default()
        };
        toolbar.highlighter.active_opacity = 0.1;
        toolbar.pen.active_opacity = 1.0;
        toolbar
    }

    pub fn show(
        &mut self, ui: &mut egui::Ui, buffer: &mut parser::Buffer, history: &mut History,
        skip_frame: &mut bool, inner_rect: egui::Rect,
    ) {
        self.handle_keyboard_shortcuts(ui, history, buffer);

        let toolbar_margin = egui::Margin::symmetric(15.0, 7.0);
        ui.visuals_mut().window_rounding = egui::Rounding::same(30.0);
        ui.style_mut().spacing.window_margin = toolbar_margin;

        if ui.visuals().dark_mode {
            ui.visuals_mut().window_stroke =
                egui::Stroke::new(0.5, egui::Color32::from_rgb(56, 56, 56));
            ui.visuals_mut().window_fill = egui::Color32::from_rgb(36, 36, 36);
            ui.visuals_mut().window_shadow = egui::Shadow::NONE;
        } else {
            ui.visuals_mut().window_stroke =
                egui::Stroke::new(0.5, egui::Color32::from_rgb(220, 220, 220));
            ui.visuals_mut().window_shadow = egui::Shadow {
                offset: egui::vec2(1.0, 8.0),
                blur: 20.0,
                spread: 0.0,
                color: egui::Color32::from_black_alpha(5),
            };
            ui.visuals_mut().window_fill = ui.visuals().extreme_bg_color;
        }

        let history_island = self.show_history_island(ui, history, buffer);
        let viewport_island = self.show_viewport_island(ui, buffer);
        let tools_island = self.show_tools_island(ui);
        let tool_controls_res = self.show_tool_controls(ui);

        let mut overlay_res = history_island;
        if let Some(res) = tool_controls_res {
            overlay_res = overlay_res.union(res);
        }
        if let Some(res) = viewport_island {
            overlay_res = overlay_res.union(res);
        }
        overlay_res = overlay_res.union(tools_island.inner.response);

        if overlay_res.hovered() || overlay_res.clicked() || overlay_res.contains_pointer() {
            *skip_frame = true;
        }
    }

    fn show_tools_island(
        &mut self, ui: &mut egui::Ui,
    ) -> InnerResponse<InnerResponse<InnerResponse<()>>> {
        let outer_margin = 20.0;

        let tools_island_size = self.layout.tools_island.unwrap_or(egui::Rect::ZERO).size();

        let tools_island_x_start = ui.available_rect_before_wrap().left()
            + (ui.available_width() - tools_island_size.x) / 2.0;
        let tools_island_y_start =
            ui.available_rect_before_wrap().bottom() - outer_margin - tools_island_size.y;

        let tools_island_rect = egui::Rect {
            min: egui::pos2(tools_island_x_start, tools_island_y_start),
            max: egui::pos2(
                tools_island_x_start + tools_island_size.x,
                tools_island_y_start + tools_island_size.y,
            ),
        };

        let res = ui.allocate_ui_at_rect(tools_island_rect, |ui| {
            egui::Frame::window(ui.style()).show(ui, |ui| {
                ui.horizontal(|ui| {
                    let tool_icon_size = 25.0;

                    let selection_btn = Button::default()
                        .icon(&Icon::HAND.size(tool_icon_size))
                        .show(ui);
                    if selection_btn.clicked() {
                        set_tool!(self, Tool::Selection);
                    }

                    let pen_btn = Button::default()
                        .icon(&Icon::BRUSH.size(tool_icon_size))
                        .show(ui);
                    if pen_btn.clicked() {
                        set_tool!(self, Tool::Pen);
                    }

                    let highlighter_btn = Button::default()
                        .icon(&Icon::HIGHLIGHTER.size(tool_icon_size))
                        .show(ui);
                    if highlighter_btn.clicked() {
                        set_tool!(self, Tool::Highlighter);
                    }

                    let eraser_btn = Button::default()
                        .icon(&Icon::ERASER.size(tool_icon_size))
                        .show(ui);
                    if eraser_btn.clicked() {
                        set_tool!(self, Tool::Eraser);
                    }

                    let active_rect = match self.active_tool {
                        Tool::Pen => pen_btn.rect,
                        Tool::Eraser => eraser_btn.rect,
                        Tool::Selection => selection_btn.rect,
                        Tool::Highlighter => highlighter_btn.rect,
                    };

                    let min_x = animate_eased(
                        ui.ctx(),
                        "min",
                        active_rect.left() + 3.0,
                        0.5,
                        easing::cubic_in_out,
                    );

                    let max_x = animate_eased(
                        ui.ctx(),
                        "max",
                        active_rect.right() - 3.0,
                        0.5,
                        easing::cubic_in_out,
                    );

                    ui.painter().line_segment(
                        [
                            egui::pos2(min_x, active_rect.bottom() + 6.0),
                            egui::pos2(max_x, active_rect.bottom() + 6.0),
                        ],
                        egui::Stroke {
                            width: 3.0,
                            color: ui.visuals().text_color().gamma_multiply(0.2),
                        },
                    );
                })
            })
        });
        self.layout.tools_island = Some(res.response.rect);
        res
    }

    fn show_viewport_island(&mut self, ui: &mut egui::Ui, buffer: &mut Buffer) -> Option<Response> {
        let history_island = match self.layout.history_island {
            Some(val) => val,
            None => return None,
        };
        let viewport_island_x_start = history_island.right() + 15.0;
        let viewport_island_y_start = history_island.top();

        let viewport_rect = egui::Rect {
            min: egui::pos2(viewport_island_x_start, viewport_island_y_start),
            max: egui::Pos2 { x: viewport_island_x_start, y: history_island.bottom() },
        };

        let res = ui
            .allocate_ui_at_rect(viewport_rect, |ui| {
                egui::Frame::window(ui.style())
                    .inner_margin(egui::Margin::symmetric(7.5, 3.5))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let zoom_percentage =
                                ((buffer.master_transform.sx + buffer.master_transform.sy) / 2.0
                                    * 100.0)
                                    .round();

                            ui.label(format!("{}%", zoom_percentage as i32));
                        })
                    })
            })
            .inner
            .response;
        Some(res)
    }
    fn show_tool_controls(&mut self, ui: &mut egui::Ui) -> Option<Response> {
        if self.active_tool == Tool::Selection {
            return None;
        }
        let tools_island_rect = match self.layout.tools_island {
            Some(val) => val,
            None => return None,
        };

        let opacity = animate_eased(
            ui.ctx(),
            "opacity",
            if self.layout.tool_controls.is_none() { 0.0 } else { 1.0 },
            0.2,
            easing::cubic_in_out,
        );
        ui.set_opacity(opacity);
        let tool_controls_size = self.layout.tool_controls.unwrap_or(egui::Rect::ZERO).size();

        let tool_controls_x_start = ui.available_rect_before_wrap().left()
            + (ui.available_width() - tool_controls_size.x) / 2.0;
        let tool_controls_y_start = tools_island_rect.top() - tool_controls_size.y - 10.0;
        let tool_controls_rect = egui::Rect {
            min: egui::pos2(tool_controls_x_start, tool_controls_y_start),
            max: egui::pos2(tool_controls_x_start + tool_controls_size.x, tool_controls_y_start),
        };

        ui.allocate_rect(tool_controls_rect, egui::Sense::click());
        if self.show_tool_controls {
            let tool_controls = ui.allocate_ui_at_rect(tool_controls_rect, |ui| {
                egui::Frame::window(ui.style()).show(ui, |ui| {
                    match self.active_tool {
                        Tool::Pen => show_pen_controls(ui, &mut self.pen),
                        Tool::Eraser => self.show_eraser_controls(ui),
                        Tool::Highlighter => show_highlighter_controls(ui, &mut self.highlighter),
                        Tool::Selection => {}
                    };
                })
            });

            self.layout.tool_controls = Some(tool_controls.response.rect);
            Some(tool_controls.response)
        } else {
            None
        }
    }

    fn show_eraser_controls(&mut self, ui: &mut egui::Ui) {
        let width = 200.0;
        ui.style_mut().spacing.slider_width = width;
        ui.set_width(width);

        let (_, preview_rect) = ui.allocate_space(egui::vec2(ui.available_width(), 100.0));
        let mut painter = ui.painter().to_owned();
        painter.set_clip_rect(preview_rect);

        self.eraser
            .draw_eraser_cursor(ui, &painter, preview_rect.center());

        ui.add_space(20.0);
        show_thickness_slider(
            ui,
            &mut self.eraser.radius,
            DEFAULT_ERASER_RADIUS..=DEFAULT_ERASER_RADIUS * 20.0,
        );
        ui.add_space(10.0);
    }

    fn handle_keyboard_shortcuts(
        &mut self, ui: &mut egui::Ui, history: &mut History, buffer: &mut Buffer,
    ) {
        if ui.input_mut(|r| {
            r.consume_key(egui::Modifiers::CTRL | egui::Modifiers::SHIFT, egui::Key::Z)
        }) {
            history.redo(buffer);
        }

        if ui.input_mut(|r| r.consume_key(egui::Modifiers::NONE, egui::Key::B)) {
            set_tool!(self, Tool::Pen);
        }

        if ui.input_mut(|r| r.consume_key(egui::Modifiers::NONE, egui::Key::E)) {
            set_tool!(self, Tool::Eraser);
        }

        if ui.input_mut(|r| r.consume_key(egui::Modifiers::NONE, egui::Key::S)) {
            set_tool!(self, Tool::Selection);
        }

        if ui.input_mut(|r| r.consume_key(egui::Modifiers::COMMAND, egui::Key::Z)) {
            history.undo(buffer);
        }
    }

    // todo: move this into a selection tooltip
    fn show_selection_controls(&self, ui: &mut egui::Ui, buffer: &mut Buffer) {
        let mut max_current_index = 0;
        let mut min_cureent_index = usize::MAX;
        ui.label("layers: ");
        self.selection
            .selected_elements
            .iter()
            .for_each(|selected_element| {
                if let Some((el_id, _, _)) = buffer.elements.get_full(&selected_element.id) {
                    max_current_index = el_id.max(max_current_index);
                    min_cureent_index = el_id.min(min_cureent_index);
                }
            });

        if Button::default()
            .icon(&Icon::BRING_TO_BACK.color(if max_current_index == buffer.elements.len() - 1 {
                ui.visuals().text_color().gamma_multiply(0.4)
            } else {
                ui.visuals().text_color()
            }))
            .show(ui)
            .clicked()
            && max_current_index != buffer.elements.len() - 1
        {
            self.selection
                .selected_elements
                .iter()
                .for_each(|selected_element| {
                    if let Some((el_id, _, _)) = buffer.elements.get_full(&selected_element.id) {
                        buffer.elements.move_index(el_id, buffer.elements.len() - 1);
                    }
                });
        }

        if Button::default()
            .icon(&Icon::BRING_BACK.color(if max_current_index == buffer.elements.len() - 1 {
                ui.visuals().text_color().gamma_multiply(0.4)
            } else {
                ui.visuals().text_color()
            }))
            .show(ui)
            .clicked()
            && max_current_index != buffer.elements.len() - 1
        {
            self.selection
                .selected_elements
                .iter()
                .for_each(|selected_element| {
                    if let Some((el_id, _, _)) = buffer.elements.get_full(&selected_element.id) {
                        if el_id < buffer.elements.len() - 1 {
                            buffer.elements.swap_indices(el_id, el_id + 1);
                        }
                    }
                });
        }

        if Button::default()
            .icon(&Icon::BRING_FRONT.color(if min_cureent_index == 0 {
                ui.visuals().text_color().gamma_multiply(0.4)
            } else {
                ui.visuals().text_color()
            }))
            .show(ui)
            .clicked()
            && min_cureent_index != 0
        {
            self.selection
                .selected_elements
                .iter()
                .for_each(|selected_element| {
                    if let Some((el_id, _, _)) = buffer.elements.get_full(&selected_element.id) {
                        if el_id > 0 {
                            buffer.elements.swap_indices(el_id, el_id - 1);
                        }
                    }
                });
        }

        if Button::default()
            .icon(&Icon::BRING_TO_FRONT.color(if min_cureent_index == 0 {
                ui.visuals().text_color().gamma_multiply(0.4)
            } else {
                ui.visuals().text_color()
            }))
            .show(ui)
            .clicked()
            && min_cureent_index != 0
        {
            self.selection
                .selected_elements
                .iter()
                .for_each(|selected_element| {
                    if let Some((el_id, _, _)) = buffer.elements.get_full(&selected_element.id) {
                        buffer.elements.move_index(el_id, 0);
                    }
                });
        }
    }

    // todo: create zoom island
    fn show_zoom_controls(
        &mut self, ui: &mut egui::Ui, buffer: &mut Buffer, skip_frame: &mut bool,
        inner_rect: egui::Rect,
    ) -> Option<Transform> {
        let zoom_percentage =
            ((buffer.master_transform.sx + buffer.master_transform.sy) / 2.0 * 100.0).round();

        let mut selected = (zoom_percentage, false);

        let res = egui::ComboBox::from_id_source("zoom_percentage_combobox")
            .selected_text(format!("{}%", zoom_percentage as i32))
            .show_ui(ui, |ui| {
                let btns = [FIT_ZOOM, 50, 100, 200].iter().map(|&i| {
                    let label =
                        if i == FIT_ZOOM { "Content Fit".to_string() } else { format!("{}%", i) };
                    ui.selectable_value(&mut selected, (i as f32, true), label)
                        .rect
                });
                btns.reduce(|acc, e| e.union(acc))
            })
            .inner;

        if let Some(Some(r)) = res {
            if r.contains(ui.input(|r| r.pointer.hover_pos().unwrap_or_default())) {
                *skip_frame = true;
            }
        }

        if selected.1 {
            selected.1 = false;

            if selected.0 as i32 == FIT_ZOOM && !buffer.elements.is_empty() {
                let elements_bound = match calc_elements_bounds(buffer) {
                    Some(rect) => rect,
                    None => return None,
                };

                let is_width_smaller = elements_bound.width() < elements_bound.height();

                let padding_coeff = 0.7; // from 0 to 1. the closer you're to 0 the less zoomed in the fit will be
                let zoom_delta = if is_width_smaller {
                    inner_rect.height() * padding_coeff / elements_bound.height()
                } else {
                    inner_rect.width() * padding_coeff / elements_bound.width()
                };

                let center_x = inner_rect.center().x
                    - zoom_delta * (elements_bound.left() + elements_bound.width() / 2.0);
                let center_y = inner_rect.center().y
                    - zoom_delta * (elements_bound.top() + elements_bound.height() / 2.0);

                return Some(
                    Transform::identity()
                        .post_scale(zoom_delta, zoom_delta)
                        .post_translate(center_x, center_y),
                );
            } else {
                return Some(zoom_percentage_to_transform(selected.0, buffer, ui));
            }
        }

        None
    }

    fn show_history_island(
        &mut self, ui: &mut egui::Ui, history: &mut History, buffer: &mut Buffer,
    ) -> egui::Response {
        let outer_margin = 20.0;
        let history_island_x_start = ui.available_rect_before_wrap().left() + outer_margin;
        let history_island_y_start = ui.available_rect_before_wrap().top() + outer_margin;

        let history_rect = egui::Rect {
            min: egui::pos2(history_island_x_start, history_island_y_start),
            max: egui::Pos2 { x: history_island_x_start, y: history_island_y_start },
        };

        let res = ui.allocate_ui_at_rect(history_rect, |ui| {
            egui::Frame::window(ui.style())
                .inner_margin(egui::Margin::symmetric(7.5, 3.5))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        if ui
                            .add_enabled_ui(history.has_undo(), |ui| {
                                Button::default().icon(&Icon::UNDO).show(ui)
                            })
                            .inner
                            .clicked()
                        {
                            history.undo(buffer);
                        }

                        if ui
                            .add_enabled_ui(history.has_redo(), |ui| {
                                Button::default().icon(&Icon::REDO).show(ui)
                            })
                            .inner
                            .clicked()
                        {
                            history.redo(buffer);
                        }
                    })
                })
        });
        self.layout.history_island = Some(res.response.rect);
        res.inner.response
    }
}

fn show_pen_controls(ui: &mut egui::Ui, pen: &mut Pen) {
    let width = 200.0;
    ui.style_mut().spacing.slider_width = width;
    ui.set_width(width);

    show_stroke_preview(ui, pen);
    // a bit hacky but without this there will be collision with
    // thickness hints.
    ui.add_space(20.0);

    show_thickness_slider(ui, &mut pen.active_stroke_width, 3.0..=30.0);

    ui.add_space(10.0);

    ui.horizontal_wrapped(|ui| {
        show_color_swatches(ui, get_pen_colors(), pen);
    });

    ui.add_space(10.0);
}

fn show_highlighter_controls(ui: &mut egui::Ui, pen: &mut Pen) {
    let width = 200.0;
    ui.style_mut().spacing.slider_width = width;
    ui.set_width(width);

    show_stroke_preview(ui, pen);

    // a bit hacky but without this there will be collision with
    // thickness hints.
    ui.add_space(20.0);

    show_thickness_slider(ui, &mut pen.active_stroke_width, 15.0..=40.0);

    ui.add_space(10.0);

    ui.horizontal_wrapped(|ui| {
        show_color_swatches(ui, get_highlighter_colors(), pen);
    });

    ui.add_space(10.0);
}

fn show_color_swatches(
    ui: &mut egui::Ui, colors: Vec<(egui::Color32, egui::Color32)>, pen: &mut Pen,
) {
    colors.iter().for_each(|c| {
        let color = ThemePalette::resolve_dynamic_color(*c, ui);
        let active_color = ThemePalette::resolve_dynamic_color(pen.active_color, ui);
        if show_color_btn(ui, color, active_color).clicked() {
            pen.active_color = *c;
        }
    });
}

fn show_color_btn(
    ui: &mut egui::Ui, color: egui::Color32, active_color: egui::Color32,
) -> egui::Response {
    let circle_diamter = COLOR_SWATCH_BTN_RADIUS * 2.0;
    let margin = 6.0;
    let (id, rect) =
        ui.allocate_space(egui::vec2(circle_diamter + margin, circle_diamter + margin));

    ui.painter()
        .circle_filled(rect.center(), COLOR_SWATCH_BTN_RADIUS, color);

    if get_non_additive(&active_color).eq(&color) {
        ui.painter().circle_stroke(
            rect.center(),
            COLOR_SWATCH_BTN_RADIUS - 3.0,
            egui::Stroke { width: 1.5, color: ui.visuals().extreme_bg_color },
        );
    }
    ui.interact(rect, id, egui::Sense::click())
}

fn show_stroke_preview(ui: &mut egui::Ui, pen: &mut Pen) {
    let preview_stroke = egui::Stroke {
        width: pen.active_stroke_width,
        color: ThemePalette::resolve_dynamic_color(pen.active_color, ui)
            .linear_multiply(pen.active_opacity),
    };

    let bez1 = epaint::CubicBezierShape::from_points_stroke(
        [
            egui::pos2(146.814, 162.413),
            egui::pos2(146.814, 162.413),
            egui::pos2(167.879, 128.734),
            egui::pos2(214.253, 129.08),
        ],
        false,
        egui::Color32::TRANSPARENT,
        preview_stroke,
    );
    let bez2 = epaint::CubicBezierShape::from_points_stroke(
        [
            egui::pos2(214.253, 129.08),
            egui::pos2(260.627, 129.426),
            egui::pos2(302.899, 190.097),
            egui::pos2(337.759, 189.239),
        ],
        false,
        egui::Color32::TRANSPARENT,
        preview_stroke,
    );
    let bez3 = epaint::CubicBezierShape::from_points_stroke(
        [
            egui::pos2(337.759, 189.239),
            egui::pos2(372.619, 188.381),
            egui::pos2(394.388, 137.297),
            egui::pos2(394.388, 137.297),
        ],
        false,
        egui::Color32::TRANSPARENT,
        preview_stroke,
    );

    let (_, preview_rect) = ui.allocate_space(egui::vec2(ui.available_width(), 100.0));

    let mut path_rect = egui::Rect::NOTHING;
    for bez in [&bez1, &bez2, &bez3] {
        path_rect = path_rect.union(bez.visual_bounding_rect());
    }

    let bezs = [bez1, bez2, bez3].map(|bez| {
        bez.transform(&RectTransform::from_to(
            path_rect,
            preview_rect.shrink2(egui::vec2(30.0, 40.0)),
        ))
        .into()
    });

    ui.painter().extend(bezs);
}

fn show_thickness_slider(ui: &mut egui::Ui, value: &mut f32, value_range: RangeInclusive<f32>) {
    let width = ui.available_width();
    let slider_rect = ui
        .add(
            egui::Slider::new(value, value_range.clone())
                // .step_by(1.0)
                .show_value(false)
                .handle_shape(egui::style::HandleShape::Rect { aspect_ratio: 0.5 }),
        )
        .rect;

    let middle_range = value_range.start() + (value_range.end() - value_range.start()).abs() / 2.0;
    let ticks = [value_range.start(), &middle_range, value_range.end()];

    for (i, t) in ticks.iter().enumerate() {
        let margin = egui::vec2(2.0, 10.0);
        let end_y = slider_rect.top() - margin.y + (i as f32 * 3.0 + 1.0);

        let total_spacing = width - (THICKNESS_BTN_WIDTH * ticks.len() as f32);
        let spacing_between = total_spacing / (ticks.len() as f32 + 1.0);

        let rect_start_x = slider_rect.left()
            + spacing_between
            + i as f32 * (THICKNESS_BTN_WIDTH + spacing_between);

        let rect = match i {
            0 => {
                let rect = egui::Rect {
                    min: egui::pos2(slider_rect.left() + margin.x, slider_rect.top() - margin.y),
                    max: egui::pos2(slider_rect.left() + margin.x + THICKNESS_BTN_WIDTH, end_y),
                };
                // thickness_pickers_rect = rect;
                rect
            }
            1 => egui::Rect {
                min: egui::pos2(rect_start_x, slider_rect.top() - margin.y),
                max: egui::pos2(rect_start_x + THICKNESS_BTN_WIDTH, end_y),
            },
            2 => egui::Rect {
                min: egui::pos2(
                    slider_rect.right() - margin.x - THICKNESS_BTN_WIDTH,
                    slider_rect.top() - margin.y,
                ),
                max: egui::pos2(slider_rect.right() - margin.x, end_y),
            },
            _ => break,
        };

        let response = ui.allocate_rect(rect.expand2(egui::vec2(0.0, 5.0)), egui::Sense::click());

        if t.eq(&value) {
            ui.painter().rect_filled(
                rect.expand(5.0),
                egui::Rounding::same(8.0),
                egui::Color32::GRAY.linear_multiply(0.1),
            );
        }

        ui.painter().rect_filled(
            rect,
            egui::Rounding::same(2.0),
            ui.visuals().text_color().linear_multiply(0.8),
        );

        if response.clicked() {
            *value = **t;
        }
    }
    ui.advance_cursor_after_rect(slider_rect);
}

pub fn get_highlighter_colors() -> Vec<(Color32, Color32)> {
    let yellow = (Color32::from_rgb(244, 250, 65), Color32::from_rgb(244, 250, 65));
    let blue = (Color32::from_rgb(65, 194, 250), Color32::from_rgb(65, 194, 250));
    let pink = (Color32::from_rgb(254, 110, 175), Color32::from_rgb(254, 110, 175));

    let highlighter_colors = vec![yellow, blue, pink];
    highlighter_colors
}

pub fn get_pen_colors() -> Vec<(Color32, Color32)> {
    let blue = (Color32::from_rgb(62, 130, 230), Color32::from_rgb(54, 116, 207));
    let green = (Color32::from_rgb(42, 136, 49), Color32::from_rgb(56, 176, 65));
    let red = (Color32::from_rgb(218, 21, 21), Color32::from_rgb(174, 33, 33));
    vec![
        ThemePalette::get_fg_color(),
        blue,
        green,
        red,
        (ThemePalette::LIGHT.magenta, ThemePalette::DARK.magenta),
        (ThemePalette::LIGHT.cyan, ThemePalette::DARK.cyan),
        (ThemePalette::LIGHT.yellow, ThemePalette::DARK.yellow),
    ]
}

fn calc_elements_bounds(buffer: &mut Buffer) -> Option<egui::Rect> {
    let mut elements_bound =
        egui::Rect { min: egui::pos2(f32::MAX, f32::MAX), max: egui::pos2(f32::MIN, f32::MIN) };
    let mut dirty_bound = false;
    for (_, el) in buffer.elements.iter() {
        if el.deleted() {
            continue;
        }

        let el_rect = el.bounding_box();
        dirty_bound = true;

        elements_bound.min.x = elements_bound.min.x.min(el_rect.min.x);
        elements_bound.min.y = elements_bound.min.y.min(el_rect.min.y);

        elements_bound.max.x = elements_bound.max.x.max(el_rect.max.x);
        elements_bound.max.y = elements_bound.max.y.max(el_rect.max.y);
    }
    if !dirty_bound {
        None
    } else {
        Some(elements_bound)
    }
}

fn get_non_additive(color: &egui::Color32) -> egui::Color32 {
    egui::Color32::from_rgb(color.r(), color.g(), color.b())
}
