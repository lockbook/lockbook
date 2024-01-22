use std::f32::consts::PI;

use crate::{theme::icons::Icon, widgets::Button};

use super::{Pen, Eraser, Buffer, selection::Selection};


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
    pub selection: Selection,
}

pub enum Tool {
    Pen,
    Eraser,
    Selection,
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
    key_shortcut: Option<(egui::Modifiers, egui::Key)>,
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
                id: "Undo".to_string(),
                icon: Icon::UNDO,
                margin: egui::Margin::symmetric(4.0, 7.0),
                key_shortcut: Some((egui::Modifiers::COMMAND, egui::Key::Z)),
            }),
            Component::Button(SimpleButton {
                id: "Redo".to_string(),
                icon: Icon::REDO,
                margin: egui::Margin::symmetric(4.0, 7.0),
                key_shortcut: Some((egui::Modifiers::COMMAND, egui::Key::R)),
            }),
            Component::Separator(egui::Margin::symmetric(10.0, 0.0)),
            Component::Button(SimpleButton {
                id: "Selection".to_string(),
                icon: Icon::HAND,
                key_shortcut: Some((egui::Modifiers::NONE, egui::Key::S)),
                margin: egui::Margin::symmetric(4.0, 7.0),
            }),
            Component::Button(SimpleButton {
                id: "Pen".to_string(),
                icon: Icon::BRUSH,
                key_shortcut: Some((egui::Modifiers::NONE, egui::Key::B)),
                margin: egui::Margin::symmetric(4.0, 7.0),
            }),
            Component::Button(SimpleButton {
                id: "Eraser".to_string(),
                icon: Icon::ERASER,
                key_shortcut: Some((egui::Modifiers::NONE, egui::Key::E)),
                margin: egui::Margin::symmetric(4.0, 7.0),
            }),
            Component::Separator(egui::Margin::symmetric(10.0, 0.0)),
            Component::StrokeWidth(default_stroke_width),
            Component::StrokeWidth(default_stroke_width * 2),
            Component::StrokeWidth(default_stroke_width * 3),
            Component::Separator(egui::Margin::symmetric(10.0, 0.0)),
        ];

        Toolbar {
            components,
            active_tool: Tool::Pen,
            pen: Pen::new(max_id),
            eraser: Eraser::new(),
            selection: Selection::new(),
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, buffer: &mut Buffer) {
        let rect = self.calculate_rect(ui);
        if ui.is_enabled() {
            self.components
                .iter()
                .filter_map(|component| {
                    if let Component::Button(btn) = component {
                        if btn.key_shortcut.is_some() {
                            Some(btn)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .for_each(|btn| {
                    let shortcut = btn.key_shortcut.unwrap();
                    if ui.input_mut(|r| r.consume_key(shortcut.0, shortcut.1)) {
                        match btn.id.as_str() {
                            "Pen" => {
                                self.active_tool = Tool::Pen;
                            }
                            "Eraser" => {
                                self.active_tool = Tool::Eraser;
                            }
                            "Selection" => {
                                self.active_tool = Tool::Selection;
                            }
                            "Undo" => buffer.undo(),
                            "Redo" => buffer.redo(),
                            _ => {}
                        }
                    }
                });
        }

        ui.allocate_ui_at_rect(rect, |ui| {
            ui.horizontal(|ui| {
                for c in self.components.iter() {
                    match c {
                        Component::Button(btn) => {
                            egui::Frame::default()
                                .inner_margin(btn.margin)
                                .show(ui, |ui| {
                                    let enabled = match btn.id.as_str() {
                                        "Undo" => buffer.has_undo(),
                                        "Redo" => buffer.has_redo(),
                                        _ => true,
                                    };

                                    let btn_res = ui
                                        .add_enabled_ui(enabled, |ui| {
                                            Button::default().icon(&btn.icon).show(ui)
                                        })
                                        .inner;

                                    if btn_res.clicked() {
                                        match btn.id.as_str() {
                                            "Pen" => {
                                                self.active_tool = Tool::Pen;
                                            }
                                            "Eraser" => {
                                                self.active_tool = Tool::Eraser;
                                            }
                                            "Selection" => {
                                                self.active_tool = Tool::Selection;
                                            }
                                            "Undo" => buffer.undo(),
                                            "Redo" => buffer.redo(),
                                            _ => {}
                                        }
                                    }
                                    let is_active = match self.active_tool {
                                        Tool::Pen => btn.id.eq("Pen"),
                                        Tool::Eraser => btn.id.eq("Eraser"),
                                        Tool::Selection => btn.id.eq("Selection"),
                                    };
                                    if is_active {
                                        ui.painter().rect_filled(
                                            btn_res.rect.expand2(egui::vec2(2.0, 2.0)),
                                            egui::Rounding::same(8.0),
                                            egui::Color32::GRAY.gamma_multiply(0.1),
                                        )
                                    }
                                    if let Some(shortcut) = &btn.key_shortcut {
                                        let mut is_mac = false;
                                        if cfg!(target_os = "macos") {
                                            is_mac = true;
                                        }

                                        if shortcut.0.is_none() {
                                            btn_res.on_hover_text(format!(
                                                "{} ({})",
                                                btn.id,
                                                shortcut.1.name()
                                            ));
                                        } else {
                                            let modifier = egui::ModifierNames::NAMES
                                                .format(&shortcut.0, is_mac);
                                            btn_res.on_hover_text(format!(
                                                "{} ({} + {})",
                                                btn.id,
                                                modifier,
                                                shortcut.1.name()
                                            ));
                                        }
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