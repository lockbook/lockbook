use std::f32::consts::PI;

use egui::ScrollArea;

use crate::{
    theme::{icons::Icon, palette::ThemePalette},
    widgets::Button,
};

use super::{
    selection::Selection, util::deserialize_transform, zoom::zoom_to_percentage, Buffer, Eraser,
    Pen,
};

const COLOR_SWATCH_BTN_RADIUS: f32 = 9.0;
const THICKNESS_BTN_X_MARGIN: f32 = 5.0;
const THICKNESS_BTN_WIDTH: f32 = 30.0;

pub struct Toolbar {
    pub active_tool: Tool,
    right_tab_rect: Option<egui::Rect>,
    pub pen: Pen,
    pub eraser: Eraser,
    pub selection: Selection,
    pub previous_tool: Option<Tool>,
}

#[derive(PartialEq, Eq, Copy, Clone, Debug)]
pub enum Tool {
    Pen,
    Eraser,
    Selection,
}

#[derive(Clone)]
pub struct ColorSwatch {
    pub id: String,
    pub color: egui::Color32,
}
impl ColorSwatch {
    fn show(&self, ui: &mut egui::Ui, pen_active_color: Option<ColorSwatch>) -> egui::Response {
        let (response, painter) = ui.allocate_painter(
            egui::vec2(COLOR_SWATCH_BTN_RADIUS * PI, ui.available_height()),
            egui::Sense::click(),
        );

        if let Some(active_color) = pen_active_color {
            let opacity = if active_color.id.eq(&self.id) {
                1.0
            } else if response.hovered() {
                ui.output_mut(|w| w.cursor_icon = egui::CursorIcon::PointingHand);
                0.9
            } else {
                0.5
            };

            if active_color.id.eq(&self.id) {
                painter.rect_filled(
                    response.rect,
                    egui::Rounding::same(8.0),
                    self.color.gamma_multiply(0.2),
                );
            }
            painter.circle_filled(
                response.rect.center(),
                COLOR_SWATCH_BTN_RADIUS,
                self.color.gamma_multiply(opacity),
            );
        };
        response
    }
}

macro_rules! set_tool {
    ($obj:expr, $new_tool:expr) => {
        if $obj.active_tool != $new_tool {
            if (matches!($new_tool, Tool::Selection)) {
                $obj.selection = Selection::default();
            }
            $obj.previous_tool = Some($obj.active_tool);
            $obj.active_tool = $new_tool;
        }
    };
}

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

    pub fn new(max_id: usize) -> Self {
        Toolbar {
            active_tool: Tool::Pen,
            previous_tool: None,
            right_tab_rect: None,
            pen: Pen::new(max_id),
            eraser: Eraser::new(),
            selection: Selection::default(),
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, buffer: &mut Buffer, skip_frame: &mut bool) {
        if ui.input_mut(|r| {
            r.consume_key(egui::Modifiers::CTRL | egui::Modifiers::SHIFT, egui::Key::Z)
        }) {
            buffer.redo();
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
            buffer.undo();
        }

        ScrollArea::both().show(ui, |ui| {
            egui::Frame::default()
                .inner_margin(egui::Margin::symmetric(15.0, 7.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        self.show_left_toolbar(ui, buffer);

                        let right_bar_width =
                            if let Some(r) = self.right_tab_rect { r.width() } else { 0.0 };
                        ui.add_space(right_bar_width + 10.0);

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                            self.show_right_toolbar(ui, buffer, skip_frame);
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
            .gamma_multiply(0.4);
        ui.separator();
    }

    fn show_left_toolbar(&mut self, ui: &mut egui::Ui, buffer: &mut Buffer) {
        // show history controls: redo and undo
        let undo = ui
            .add_enabled_ui(buffer.has_undo(), |ui| Button::default().icon(&Icon::UNDO).show(ui))
            .inner;

        let redo = ui
            .add_enabled_ui(buffer.has_redo(), |ui| Button::default().icon(&Icon::REDO).show(ui))
            .inner;

        if undo.clicked() {
            buffer.undo();
        }

        if redo.clicked() {
            buffer.redo();
        }

        ui.add_space(4.0);
        ui.add(egui::Separator::default().shrink(ui.available_height() * 0.3));
        ui.add_space(4.0);

        // show drawings tools like pen, selection, and eraser
        let selection_btn = Button::default().icon(&Icon::HAND).show(ui);
        if selection_btn.clicked() {
            set_tool!(self, Tool::Selection);
        }

        let pen_btn = Button::default().icon(&Icon::BRUSH).show(ui);
        if pen_btn.clicked() {
            set_tool!(self, Tool::Pen);
        }

        let eraser_btn = Button::default().icon(&Icon::ERASER).show(ui);
        if eraser_btn.clicked() {
            set_tool!(self, Tool::Eraser);
        }

        let active_rect = match self.active_tool {
            Tool::Pen => pen_btn.rect,
            Tool::Eraser => eraser_btn.rect,
            Tool::Selection => selection_btn.rect,
        };

        ui.painter().rect_filled(
            active_rect,
            egui::Rounding::same(8.0),
            egui::Color32::GRAY.gamma_multiply(0.1),
        );

        ui.add_space(4.0);
        ui.add(egui::Separator::default().shrink(ui.available_height() * 0.3));
        ui.add_space(4.0);

        self.show_tool_inline_controls(ui);

        ui.add_space(4.0);
        ui.add(egui::Separator::default().shrink(ui.available_height() * 0.3));
        ui.add_space(4.0);
    }

    fn show_tool_inline_controls(&mut self, ui: &mut egui::Ui) {
        match self.active_tool {
            Tool::Pen => {
                if let Some(thickness) = self.show_thickness_pickers(
                    ui,
                    self.pen.active_stroke_width as f32,
                    vec![3.0, 6.0, 9.0],
                ) {
                    self.pen.active_stroke_width = thickness as u32;
                }

                ui.add_space(4.0);
                ui.add(egui::Separator::default().shrink(ui.available_height() * 0.3));
                ui.add_space(4.0);

                self.show_default_color_swatches(ui);
            }
            Tool::Eraser => {
                if let Some(thickness) =
                    self.show_thickness_pickers(ui, self.eraser.thickness, vec![10.0, 30.0, 90.0])
                {
                    self.eraser.thickness = thickness;
                }
            }
            Tool::Selection => {}
        }
    }

    fn show_default_color_swatches(&mut self, ui: &mut egui::Ui) {
        let theme_colors = ThemePalette::as_array(ui.visuals().dark_mode);

        theme_colors.iter().for_each(|theme_color| {
            let color = ColorSwatch { id: theme_color.0.clone(), color: theme_color.1 };
            if color.show(ui, self.pen.active_color.clone()).clicked() {
                self.pen.active_color = Some(color);
            }
        });
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
                    egui::Color32::GRAY.gamma_multiply(0.1),
                );
            }

            painter.rect_filled(
                rect,
                egui::Rounding::same(2.0),
                ui.visuals().text_color().gamma_multiply(0.8),
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
    ) {
        let mut zoom_percentage = 100;
        if let Some(transform) = buffer.current.attr("transform") {
            let [a, b, c, d, _, _] = deserialize_transform(transform);
            let scale_x = (a * a + b * b).sqrt();
            let scale_y = (c * c + d * d).sqrt();

            // Calculate the zoom percentage
            zoom_percentage = ((scale_x + scale_y) / 2.0 * 100.0).round() as i32;
        }

        if Button::default().icon(&Icon::ZOOM_IN).show(ui).clicked() {
            zoom_to_percentage(buffer, zoom_percentage + 10, ui.ctx().screen_rect());
        };

        let mut selected = (zoom_percentage, false);

        let res = egui::ComboBox::from_id_source("zoom_percentage_combobox")
            .selected_text(format!("{:?}%", zoom_percentage))
            .show_ui(ui, |ui| {
                let btns = [50, 100, 200].iter().map(|&i| {
                    ui.selectable_value(&mut selected, (i, true), format!("{}%", i))
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
            zoom_to_percentage(buffer, zoom_percentage - 10, ui.ctx().screen_rect());
        }

        if selected.1 {
            zoom_to_percentage(buffer, selected.0, ui.ctx().screen_rect());
            selected.1 = false;
        }

        self.right_tab_rect = Some(ui.min_rect());
    }
}
