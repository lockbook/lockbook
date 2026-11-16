use std::collections::HashMap;

use lb_rs::Uuid;
use lb_rs::model::svg::element::Element;

use crate::tab::svg_editor::DeleteElement;
use crate::tab::svg_editor::coordinates::{ScreenLength, ScreenPoint};
use crate::tab::svg_editor::toolbar::ToolContext;
use crate::tab::svg_editor::tools::{CanvasPointer, CanvasToolEvent, InputControllerTool};

pub struct Eraser {
    pub radius: f32,
    delete_candidates: HashMap<Uuid, f32>,
    is_building: bool,
    pos: egui::Pos2,
    cursor_color: egui::Color32,
}

impl Default for Eraser {
    fn default() -> Self {
        Self {
            delete_candidates: HashMap::default(),
            radius: DEFAULT_ERASER_RADIUS,
            is_building: false,
            cursor_color: egui::Color32::GRAY,
            pos: egui::Pos2::ZERO,
        }
    }
}

#[derive(PartialEq, Debug)]
pub enum EraseEvent {
    Build(ScreenPoint),
    End,
    Cancel,
}

pub const DEFAULT_ERASER_RADIUS: f32 = 5.0;

impl InputControllerTool for Eraser {
    type ToolEvent = EraseEvent;

    fn controller_event_to_tool_event(
        &self, controller_event: CanvasToolEvent,
    ) -> Option<Self::ToolEvent> {
        match controller_event {
            CanvasToolEvent::Start(payload)
            | CanvasToolEvent::Run(payload)
            | CanvasToolEvent::PredictedRun(payload) => Some(EraseEvent::Build(payload.screen_pos)),
            CanvasToolEvent::End(_) => Some(EraseEvent::End),
            CanvasToolEvent::Cancel => Some(EraseEvent::Cancel),
            _ => None,
        }
    }

    fn handle_tool_event(
        &mut self, _: &mut egui::Ui, event: Self::ToolEvent, eraser_ctx: &mut ToolContext,
    ) {
        match event {
            EraseEvent::Build(pos) => {
                self.is_building = true;
                self.pos = pos.0;

                eraser_ctx
                    .view()
                    .hit_test_all(pos, ScreenLength(self.radius))
                    .for_each(|id| {
                        self.delete_candidates
                            .entry(id)
                            .or_insert_with(|| eraser_ctx.buffer.elements[&id].opacity());
                    });

                self.delete_candidates.iter().for_each(|(id, &opacity)| {
                    if let Some(Element::Path(p)) = eraser_ctx.buffer.elements.get_mut(id) {
                        if opacity == p.opacity {
                            p.opacity *= 0.3;
                            p.diff_state.opacity_changed = true
                        }
                    };
                });
            }
            EraseEvent::End => {
                self.is_building = false;

                if self.delete_candidates.is_empty() {
                    return;
                }

                self.delete_candidates.iter().for_each(|(id, &opacity)| {
                    if let Some(Element::Path(p)) = eraser_ctx.buffer.elements.get_mut(id) {
                        p.opacity = opacity;
                        p.deleted = true;
                        p.diff_state.delete_changed = true;
                    };
                });
                let event = crate::tab::svg_editor::Event::Delete(
                    self.delete_candidates
                        .keys()
                        .map(|id| DeleteElement { id: *id })
                        .collect(),
                );

                eraser_ctx.history.save(event);

                self.delete_candidates.clear();
            }
            EraseEvent::Cancel => {
                self.delete_candidates.iter().for_each(|(id, &opacity)| {
                    if let Some(Element::Path(p)) = eraser_ctx.buffer.elements.get_mut(id) {
                        p.opacity = opacity;
                        p.diff_state.opacity_changed = true;
                    };
                });

                self.delete_candidates.clear();
                self.is_building = false;
            }
        }
    }

    fn show_hover_point(
        &self, _: &mut egui::Ui, pointer: CanvasPointer, eraser_ctx: &mut ToolContext<'_>,
    ) {
        self.show_eraser_circle(pointer.screen_pos.0, eraser_ctx);
    }

    fn show_tool_ui(&mut self, _: &mut egui::Ui, eraser_ctx: &mut ToolContext) {
        if self.is_building {
            self.show_eraser_circle(self.pos, eraser_ctx);
        }
    }
}

impl Eraser {
    pub fn new(ui: &mut egui::Ui) -> Self {
        Eraser {
            delete_candidates: HashMap::default(),
            radius: DEFAULT_ERASER_RADIUS,
            is_building: false,
            cursor_color: ui.visuals().text_color(),
            pos: ui
                .input(|r| r.pointer.hover_pos())
                .unwrap_or(egui::Pos2::ZERO),
        }
    }

    fn show_eraser_circle(&self, pos: egui::Pos2, eraser_ctx: &mut ToolContext<'_>) {
        let old_layer = eraser_ctx.painter.layer_id();
        eraser_ctx.painter.set_layer_id(egui::LayerId {
            order: egui::Order::Foreground,
            id: "eraser_overlay".into(),
        });

        self.draw_eraser_cursor(eraser_ctx.painter, pos);

        eraser_ctx.painter.set_layer_id(old_layer);
    }

    pub fn draw_eraser_cursor(&self, painter: &egui::Painter, cursor_pos: egui::Pos2) {
        let stroke = egui::Stroke { width: 1.0, color: self.cursor_color };
        painter.circle_stroke(cursor_pos, self.radius, stroke);
    }
}
