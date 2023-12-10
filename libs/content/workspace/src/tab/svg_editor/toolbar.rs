use super::{Buffer, Eraser, Pen};
use crate::theme::icons::Icon;
use crate::widgets::Button;
use std::f32::consts::PI;

const ICON_SIZE: f32 = 30.0;
const COLOR_SWATCH_BTN_RADIUS: f32 = 9.0;
const THICKNESS_BTN_X_MARGIN: f32 = 5.0;
const THICKNESS_BTN_WIDTH: f32 = 30.0;

// todo: refactor toolbar, remove vec def and hardcode buttons
pub struct Toolbar {
    pub components: Vec<Component>,
    pub active_tool: Tool,
    pub pen: Pen,
    pub eraser: Eraser,
}

pub enum Tool {
    Pen,
    Eraser,
}

#[derive(Clone)]
pub enum Component {
    Button(SimpleButton),
    ColorSwatch(ColorSwatch),
    StrokeWidth(u32),
    Separator(egui::Margin),
}
#[derive(Clone)]
pub struct SimpleButton {
    id: String,
    icon: Icon,
    margin: egui::Margin,
    coming_soon_text: Option<String>,
}
#[derive(Clone)]
pub struct ColorSwatch {
    pub id: String,
    pub color: egui::Color32,
}

trait SizableComponent {
    fn get_width(&self) -> f32;
}

impl SizableComponent for Component {
    fn get_width(&self) -> f32 {
        match self {
            Component::Button(btn) => btn.margin.sum().x + ICON_SIZE,
            Component::Separator(margin) => margin.sum().x,
            Component::ColorSwatch(_color_btn) => COLOR_SWATCH_BTN_RADIUS * PI,
            Component::StrokeWidth(_) => THICKNESS_BTN_WIDTH + THICKNESS_BTN_X_MARGIN * 2.0,
        }
    }
}

impl Toolbar {
    fn width(&self) -> f32 {
        self.components.iter().map(|c| c.get_width()).sum()
    }
    fn calculate_rect(&self, ui: &mut egui::Ui) -> egui::Rect {
        let height = 0.0;
        let available_rect = ui.available_rect_before_wrap();

        let maximized_min_x = (available_rect.width() - self.width()) / 2.0 + available_rect.left();

        let min_pos = egui::Pos2 { x: maximized_min_x, y: available_rect.top() + height };

        let maximized_max_x =
            available_rect.right() - (available_rect.width() - self.width()) / 2.0;

        let max_pos = egui::Pos2 { x: maximized_max_x, y: available_rect.top() };
        egui::Rect { min: min_pos, max: max_pos }
    }

    pub fn new(max_id: usize) -> Self {
        let default_stroke_width = 3;
        let components = vec![
            Component::Button(SimpleButton {
                id: "undo".to_string(),
                icon: Icon::UNDO,
                margin: egui::Margin::symmetric(4.0, 7.0),
                coming_soon_text: None,
            }),
            Component::Button(SimpleButton {
                id: "redo".to_string(),
                icon: Icon::REDO,
                margin: egui::Margin::symmetric(4.0, 7.0),
                coming_soon_text: None,
            }),
            Component::Separator(egui::Margin::symmetric(10.0, 0.0)),
            Component::Button(SimpleButton {
                id: "pen".to_string(),
                icon: Icon::BRUSH,
                coming_soon_text: None,
                margin: egui::Margin::symmetric(4.0, 7.0),
            }),
            Component::Button(SimpleButton {
                id: "eraser".to_string(),
                icon: Icon::ERASER,
                coming_soon_text: None,
                margin: egui::Margin::symmetric(4.0, 7.0),
            }),
            Component::Separator(egui::Margin::symmetric(10.0, 0.0)),
            Component::StrokeWidth(default_stroke_width),
            Component::StrokeWidth(default_stroke_width * 2),
            Component::StrokeWidth(default_stroke_width * 3),
            Component::Separator(egui::Margin::symmetric(10.0, 0.0)),
        ];

        Toolbar { components, active_tool: Tool::Pen, pen: Pen::new(max_id), eraser: Eraser::new() }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, buffer: &mut Buffer) {
        let rect = self.calculate_rect(ui);

        ui.allocate_ui_at_rect(rect, |ui| {
            ui.horizontal(|ui| {
                for c in self.components.iter() {
                    match c {
                        Component::Button(btn) => {
                            egui::Frame::default()
                                .inner_margin(btn.margin)
                                .show(ui, |ui| {
                                    let enabled = match btn.id.as_str() {
                                        "undo" => buffer.has_undo(),
                                        "redo" => buffer.has_redo(),
                                        _ => true,
                                    };

                                    let btn_res = ui
                                        .add_enabled_ui(enabled, |ui| {
                                            Button::default().icon(&btn.icon).show(ui)
                                        })
                                        .inner;

                                    if btn_res.clicked() {
                                        match btn.id.as_str() {
                                            "pen" => {
                                                self.active_tool = Tool::Pen;
                                            }
                                            "eraser" => {
                                                self.active_tool = Tool::Eraser;
                                            }
                                            "undo" => buffer.undo(),
                                            "redo" => buffer.redo(),
                                            _ => {}
                                        }
                                    }
                                    if let Some(tooltip_text) = &btn.coming_soon_text {
                                        btn_res.on_hover_text(tooltip_text);
                                    }
                                });
                        }
                        Component::Separator(margin) => {
                            ui.add_space(margin.right);
                            ui.add(egui::Separator::default().shrink(ui.available_height() * 0.3));
                            ui.add_space(margin.left);
                        }
                        Component::ColorSwatch(btn) => {
                            let (response, painter) = ui.allocate_painter(
                                egui::vec2(COLOR_SWATCH_BTN_RADIUS * PI, ui.available_height()),
                                egui::Sense::click(),
                            );
                            if response.clicked() {
                                self.pen.active_color =
                                    Some(ColorSwatch { id: btn.id.clone(), color: btn.color });
                            }
                            if let Some(active_color) = &self.pen.active_color {
                                let opacity = if active_color.id.eq(&btn.id) {
                                    1.0
                                } else if response.hovered() {
                                    ui.output_mut(|w| {
                                        w.cursor_icon = egui::CursorIcon::PointingHand
                                    });
                                    0.9
                                } else {
                                    0.5
                                };

                                if active_color.id.eq(&btn.id) {
                                    painter.rect_filled(
                                        response.rect.shrink2(egui::vec2(0.0, 5.0)),
                                        egui::Rounding::same(8.0),
                                        btn.color.gamma_multiply(0.2),
                                    )
                                }
                                painter.circle_filled(
                                    response.rect.center(),
                                    COLOR_SWATCH_BTN_RADIUS,
                                    btn.color.gamma_multiply(opacity),
                                );
                            }
                        }
                        Component::StrokeWidth(thickness) => {
                            ui.add_space(THICKNESS_BTN_X_MARGIN);
                            let (response, painter) = ui.allocate_painter(
                                egui::vec2(THICKNESS_BTN_WIDTH, ui.available_height()),
                                egui::Sense::click(),
                            );

                            let rect = egui::Rect {
                                min: egui::Pos2 {
                                    x: response.rect.left(),
                                    y: response.rect.center().y - (*thickness as f32 / 3.0),
                                },
                                max: egui::Pos2 {
                                    x: response.rect.right(),
                                    y: response.rect.center().y + (*thickness as f32 / 3.0),
                                },
                            };

                            if thickness.eq(&self.pen.active_stroke_width) {
                                painter.rect_filled(
                                    response.rect.shrink2(egui::vec2(0.0, 5.0)),
                                    egui::Rounding::same(8.0),
                                    egui::Color32::GRAY.gamma_multiply(0.1),
                                )
                            }
                            if response.clicked() {
                                self.pen.active_stroke_width = *thickness;
                            }
                            if response.hovered() {
                                ui.output_mut(|w| w.cursor_icon = egui::CursorIcon::PointingHand);
                            }

                            painter.rect_filled(
                                rect,
                                egui::Rounding::same(2.0),
                                ui.visuals().text_color().gamma_multiply(0.8),
                            );
                            ui.add_space(THICKNESS_BTN_X_MARGIN);
                        }
                    };
                }
            })
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
}
