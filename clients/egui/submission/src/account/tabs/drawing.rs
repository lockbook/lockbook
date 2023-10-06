use eframe::egui;
use egui_extras::{Size, StripBuilder};

use crate::theme::{DrawingPalette, Icon};
use crate::widgets::ButtonGroup;

pub struct Drawing {
    pub drawing: lb::Drawing,
    palette: DrawingPalette,
}

impl Drawing {
    pub fn boxed(drawing: lb::Drawing) -> Box<Self> {
        Box::new(Self { drawing, palette: DrawingPalette::get() })
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        StripBuilder::new(ui)
            .size(Size::remainder())
            .size(Size::exact(50.0))
            .vertical(|mut strip| {
                strip.cell(|ui| self.draw_canvas(ui));
                strip.cell(|ui| self.draw_toolbar(ui));
            });
    }

    fn draw_canvas(&mut self, ui: &mut egui::Ui) {
        egui::Frame::canvas(ui.style())
            .stroke(egui::Stroke::NONE)
            .show(ui, |ui| {
                let (response, painter) =
                    ui.allocate_painter(ui.available_size_before_wrap(), egui::Sense::drag());

                let current_stroke = self.current_stroke_mut();

                if let Some(pointer_pos) = response.interact_pointer_pos() {
                    if last_pos2(current_stroke) != Some(pointer_pos) {
                        append_pos2(current_stroke, pointer_pos);
                    }
                } else if !current_stroke.is_empty() {
                    let color = self.current_stroke_mut().color;
                    self.drawing.strokes.push(lb::Stroke::new(color));
                }

                let mut shapes = Vec::new();
                for s in &self.drawing.strokes {
                    if s.points_x.len() >= 2 {
                        let mut points = Vec::new();
                        for (i, x) in s.points_x.iter().enumerate() {
                            let y = s.points_y[i];
                            points.push(egui::pos2(*x, y));
                        }
                        shapes.push(egui::Shape::line(
                            points,
                            egui::Stroke::new(5.0, self.palette[s.color]),
                        ));
                    }
                }
                painter.extend(shapes);
            });
    }

    fn current_stroke_mut(&mut self) -> &mut lb::Stroke {
        let strokes = &mut self.drawing.strokes;
        if strokes.is_empty() {
            strokes.push(lb::Stroke::new(lb::ColorAlias::Black));
        }
        strokes.last_mut().unwrap()
    }

    fn draw_toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.add_space(5.0);

            let current_color = self.current_stroke_mut().color;
            let mut grp = ButtonGroup::toggle(current_color);

            for (alias, _name) in COLOR_ALIASES {
                grp = grp.btn_icon(alias, &Icon::CIRCLE.size(24.0).color(self.palette[alias]));
            }

            if let Some(new_color) = grp.show(ui) {
                self.current_stroke_mut().color = new_color;
            }

            ui.add_space(10.0);

            if let Some(action) = ButtonGroup::default()
                .btn_icon(Action::CopyToClipboard, &Icon::CONTENT_COPY.size(24.0))
                .btn_icon(Action::ClearAll, &Icon::CANCEL_PRESENTATION.size(24.0))
                .show(ui)
            {
                match action {
                    Action::CopyToClipboard => {}
                    Action::ClearAll => self.drawing.strokes.clear(),
                }
            }
        });
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Action {
    CopyToClipboard,
    ClearAll,
}

const COLOR_ALIASES: [(lb::ColorAlias, &str); 8] = [
    (lb::ColorAlias::Black, "Black"),
    (lb::ColorAlias::Red, "Red"),
    (lb::ColorAlias::Green, "Green"),
    (lb::ColorAlias::Yellow, "Yellow"),
    (lb::ColorAlias::Blue, "Blue"),
    (lb::ColorAlias::Magenta, "Magenta"),
    (lb::ColorAlias::Cyan, "Cyan"),
    (lb::ColorAlias::White, "White"),
];

fn last_pos2(s: &lb::Stroke) -> Option<egui::Pos2> {
    let x = s.points_x.last()?;
    let y = s.points_y.last()?;
    Some(egui::pos2(*x, *y))
}

fn append_pos2(s: &mut lb::Stroke, p: egui::Pos2) {
    s.points_x.push(p.x);
    s.points_y.push(p.y);
    s.points_girth.push(1.0);
}
