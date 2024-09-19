use std::f32::consts::PI;

use egui::{Color32, ScrollArea};
use resvg::usvg::Transform;

use crate::{
    theme::{icons::Icon, palette::ThemePalette},
    widgets::Button,
};

use super::{
    eraser::DEFAULT_ERASER_RADIUS, history::History, parser, pen::DEFAULT_PEN_STROKE_WIDTH,
    selection::Selection, zoom::zoom_percentage_to_transform, Buffer, Eraser, Pen,
};

const COLOR_SWATCH_BTN_RADIUS: f32 = 9.0;
const THICKNESS_BTN_X_MARGIN: f32 = 5.0;
const THICKNESS_BTN_WIDTH: f32 = 30.0;

#[derive(Default)]
pub struct Toolbar {
    pub active_tool: Tool,
    right_tab_rect: Option<egui::Rect>,
    pub pen: Pen,
    pub eraser: Eraser,
    pub selection: Selection,
    pub previous_tool: Option<Tool>,
}

#[derive(PartialEq, Eq, Copy, Clone, Debug, Default)]
pub enum Tool {
    #[default]
    Pen,
    Eraser,
    Selection,
    Highlighter,
    Brush,
}

pub struct ToolContext<'a> {
    pub painter: &'a egui::Painter,
    pub buffer: &'a mut Buffer,
    pub history: &'a mut History,
    pub allow_viewport_changes: &'a mut bool,
    pub is_viewport_changing: &'a mut bool,
    pub is_touch_frame: &'a mut bool,
}
#[derive(Clone)]
pub struct ColorSwatch {
    pub id: Option<String>,
    pub color: egui::Color32,
}

macro_rules! set_tool {
    ($obj:expr, $new_tool:expr) => {
        if $obj.active_tool != $new_tool {
            if (matches!($new_tool, Tool::Selection)) {
                $obj.selection = Selection::default();
            }
            if (matches!($new_tool, Tool::Pen)) {
                $obj.pen.active_opacity = 1.0;
                $obj.pen.supports_pressure = false;
            }
            $obj.previous_tool = Some($obj.active_tool);
            $obj.active_tool = $new_tool;
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
        Toolbar {
            active_tool: Tool::Pen,
            previous_tool: None,
            right_tab_rect: None,
            pen: Pen::new(),
            eraser: Eraser::new(),
            selection: Selection::default(),
        }
    }

    pub fn show(
        &mut self, ui: &mut egui::Ui, buffer: &mut parser::Buffer, history: &mut History,
        skip_frame: &mut bool, inner_rect: egui::Rect,
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

        ScrollArea::both().show(ui, |ui| {
            egui::Frame::default()
                .inner_margin(egui::Margin::symmetric(15.0, 7.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        self.show_left_toolbar(ui, buffer, history);

                        let right_bar_width =
                            if let Some(r) = self.right_tab_rect { r.width() } else { 0.0 };
                        ui.add_space(right_bar_width + 10.0);

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                            if let Some(transform) =
                                self.show_right_toolbar(ui, buffer, skip_frame, inner_rect)
                            {
                                buffer.master_transform =
                                    buffer.master_transform.post_concat(transform);

                                buffer
                                    .elements
                                    .iter_mut()
                                    .for_each(|(_, el)| el.transform(transform));
                            }
                        });
                    });
                });
        });

        ui.visuals_mut().widgets.noninteractive.bg_stroke.color = ui
            .visuals()
            .widgets
            .noninteractive
            .bg_stroke
            .color
            .linear_multiply(0.4);
        ui.separator();
    }

    fn show_left_toolbar(&mut self, ui: &mut egui::Ui, buffer: &mut Buffer, history: &mut History) {
        // show history controls: redo and undo
        let undo = ui
            .add_enabled_ui(history.has_undo(), |ui| Button::default().icon(&Icon::UNDO).show(ui))
            .inner;

        let redo = ui
            .add_enabled_ui(history.has_redo(), |ui| Button::default().icon(&Icon::REDO).show(ui))
            .inner;

        if undo.clicked() {
            history.undo(buffer);
        }

        if redo.clicked() {
            history.redo(buffer);
        }

        ui.add_space(4.0);
        ui.add(egui::Separator::default().shrink(ui.available_height() * 0.3));
        ui.add_space(4.0);

        // show drawings tools like pen, selection, and eraser
        let selection_btn = Button::default().icon(&Icon::HAND).show(ui);
        if selection_btn.clicked() {
            set_tool!(self, Tool::Selection);
        }

        let pen_btn = Button::default().icon(&Icon::PEN).show(ui);
        if pen_btn.clicked() {
            set_tool!(self, Tool::Pen);
            self.pen.supports_pressure = false;
            self.pen.active_opacity = 1.0;
            self.pen.active_stroke_width = DEFAULT_PEN_STROKE_WIDTH;
            self.pen.active_color = None;
        }

        let brush = Button::default().icon(&Icon::BRUSH).show(ui);
        if brush.clicked() {
            set_tool!(self, Tool::Brush);
            self.pen.supports_pressure = true;
            self.pen.active_opacity = 1.0;
            self.pen.active_stroke_width = DEFAULT_PEN_STROKE_WIDTH;
            self.pen.active_color = None;
        }

        let highlighter_btn = Button::default().icon(&Icon::HIGHLIGHTER).show(ui);
        let highlighter_opacity = 0.3;
        if highlighter_btn.clicked() {
            set_tool!(self, Tool::Highlighter);
            self.pen.active_opacity = highlighter_opacity;
            self.pen.supports_pressure = false;
            self.pen.active_stroke_width = DEFAULT_PEN_STROKE_WIDTH * 2.0;
            self.pen.active_color = Some(get_highlighter_colors()[0]);
        }

        let eraser_btn = Button::default().icon(&Icon::ERASER).show(ui);
        if eraser_btn.clicked() {
            set_tool!(self, Tool::Eraser);
        }

        let active_rect = match self.active_tool {
            Tool::Pen => pen_btn.rect,
            Tool::Eraser => eraser_btn.rect,
            Tool::Selection => selection_btn.rect,
            Tool::Highlighter => highlighter_btn.rect,
            Tool::Brush => brush.rect,
        };

        ui.painter().rect_filled(
            active_rect,
            egui::Rounding::same(8.0),
            egui::Color32::GRAY.linear_multiply(0.1),
        );

        ui.add_space(4.0);
        ui.add(egui::Separator::default().shrink(ui.available_height() * 0.3));
        ui.add_space(4.0);

        self.show_tool_inline_controls(ui);
    }

    fn show_tool_inline_controls(&mut self, ui: &mut egui::Ui) {
        match self.active_tool {
            Tool::Pen | Tool::Brush => {
                if let Some(thickness) = self.show_thickness_pickers(
                    ui,
                    self.pen.active_stroke_width,
                    vec![DEFAULT_PEN_STROKE_WIDTH, 4.0, 6.0],
                ) {
                    self.pen.active_stroke_width = thickness;
                }

                ui.add_space(4.0);
                ui.add(egui::Separator::default().shrink(ui.available_height() * 0.3));
                ui.add_space(4.0);

                self.show_default_color_swatches(ui);
            }
            Tool::Eraser => {
                if let Some(thickness) = self.show_thickness_pickers(
                    ui,
                    self.eraser.radius,
                    vec![DEFAULT_ERASER_RADIUS, 30.0, 90.0],
                ) {
                    self.eraser.radius = thickness;
                }
            }
            Tool::Selection => {}
            Tool::Highlighter => {
                if let Some(thickness) = self.show_thickness_pickers(
                    ui,
                    self.pen.active_stroke_width,
                    vec![DEFAULT_PEN_STROKE_WIDTH * 2.0, 4.0 * 2.0, 6.0 * 2.0],
                ) {
                    self.pen.active_stroke_width = thickness;
                }

                ui.add_space(4.0);
                ui.add(egui::Separator::default().shrink(ui.available_height() * 0.3));
                ui.add_space(4.0);

                let highlighter_colors = get_highlighter_colors();

                highlighter_colors.iter().for_each(|theme_color| {
                    let color = ThemePalette::resolve_dynamic_color(*theme_color, ui);
                    if self.show_color_btn(ui, color).clicked() {
                        self.pen.active_color = Some(*theme_color);
                    }
                });
            }
        }
    }

    fn show_default_color_swatches(&mut self, ui: &mut egui::Ui) {
        let theme_colors = ThemePalette::as_array();

        theme_colors.iter().for_each(|theme_color| {
            let color = ThemePalette::resolve_dynamic_color(*theme_color, ui);
            if self.show_color_btn(ui, color).clicked() {
                self.pen.active_color = Some(*theme_color);
            }
        });
    }

    fn show_color_btn(&self, ui: &mut egui::Ui, color: egui::Color32) -> egui::Response {
        let (response, painter) = ui.allocate_painter(
            egui::vec2(COLOR_SWATCH_BTN_RADIUS * PI, ui.available_height()),
            egui::Sense::click(),
        );

        if let Some(active_color) = self.pen.active_color {
            let active_color = if ui.visuals().dark_mode { active_color.1 } else { active_color.0 };
            let opacity = if active_color.eq(&color) {
                1.0
            } else if response.hovered() {
                0.9
            } else {
                0.5
            };

            if active_color.eq(&color) {
                painter.rect_filled(
                    response.rect,
                    egui::Rounding::same(8.0),
                    color.linear_multiply(0.2),
                );
            }
            painter.circle_filled(
                response.rect.center(),
                COLOR_SWATCH_BTN_RADIUS,
                color.linear_multiply(opacity),
            );
        };
        response
    }

    fn show_thickness_pickers(
        &mut self, ui: &mut egui::Ui, active_thickness: f32, options: Vec<f32>,
    ) -> Option<f32> {
        let mut chosen = None;
        options.iter().enumerate().for_each(|(i, t)| {
            ui.add_space(THICKNESS_BTN_X_MARGIN);
            let (response, painter) = ui.allocate_painter(
                egui::vec2(THICKNESS_BTN_WIDTH, ui.available_height()),
                egui::Sense::click(),
            );

            let rect = egui::Rect {
                min: egui::Pos2 {
                    x: response.rect.left(),
                    y: response.rect.center().y - ((i as f32 * 3.0 + 3.0) / 3.0),
                },
                max: egui::Pos2 {
                    x: response.rect.right(),
                    y: response.rect.center().y + ((i as f32 * 3.0 + 3.0) / 3.0),
                },
            };

            if t.eq(&active_thickness) {
                painter.rect_filled(
                    response.rect,
                    egui::Rounding::same(8.0),
                    egui::Color32::GRAY.linear_multiply(0.1),
                );
            }

            painter.rect_filled(
                rect,
                egui::Rounding::same(2.0),
                ui.visuals().text_color().linear_multiply(0.8),
            );

            ui.add_space(THICKNESS_BTN_X_MARGIN);

            if response.clicked() {
                chosen = Some(*t);
            }
        });
        chosen
    }

    fn show_right_toolbar(
        &mut self, ui: &mut egui::Ui, buffer: &mut Buffer, skip_frame: &mut bool,
        inner_rect: egui::Rect,
    ) -> Option<Transform> {
        let zoom_percentage =
            ((buffer.master_transform.sx + buffer.master_transform.sy) / 2.0 * 100.0).round();

        if Button::default().icon(&Icon::ZOOM_IN).show(ui).clicked() {
            return Some(zoom_percentage_to_transform(zoom_percentage + 10., buffer, ui));
        };

        let mut selected = (zoom_percentage, false);

        let res = egui::ComboBox::from_id_source("zoom_percentage_combobox")
            .selected_text(format!("{:?}%", zoom_percentage))
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
        if Button::default().icon(&Icon::ZOOM_OUT).show(ui).clicked() {
            return Some(zoom_percentage_to_transform(zoom_percentage - 10., buffer, ui));
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

        self.right_tab_rect = Some(ui.min_rect());
        None
    }
}

fn get_highlighter_colors() -> Vec<(Color32, Color32)> {
    let yellow = (Color32::from_rgb(244, 250, 65), Color32::from_rgb(244, 250, 65));
    let blue = (Color32::from_rgb(65, 194, 250), Color32::from_rgb(65, 194, 250));
    let pink = (Color32::from_rgb(254, 110, 175), Color32::from_rgb(254, 110, 175));

    let highlighter_colors = vec![yellow, blue, pink];
    highlighter_colors
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
