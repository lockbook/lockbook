use std::collections::HashMap;

use lb_rs::Uuid;
use lb_rs::model::svg::element::Element;

use crate::tab::svg_editor::roger::RogerEvent;

use super::DeleteElement;
use super::toolbar::ToolContext;
use super::util::pointer_intersects_element;

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
    Build(egui::Pos2),
    End,
    Cancel,
}

pub fn from_roger_to_eraser_event(event: RogerEvent) -> Option<EraseEvent> {
    match event {
        RogerEvent::ToolStart(payload) | RogerEvent::ToolRun(payload) => {
            Some(EraseEvent::Build(payload.pos))
        }
        RogerEvent::ToolEnd(_) => Some(EraseEvent::End),
        RogerEvent::ToolCancel => Some(EraseEvent::Cancel),
        _ => None,
    }
}

pub const DEFAULT_ERASER_RADIUS: f32 = 5.0;

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
    pub fn show_tool_ui(&self, _: &mut egui::Ui, eraser_ctx: &mut ToolContext<'_>) {
        if self.is_building {
            self.show_eraser_circle(self.pos, eraser_ctx);
        }
    }

    pub fn handle_erase_event(&mut self, event: &EraseEvent, eraser_ctx: &mut ToolContext<'_>) {
        match *event {
            EraseEvent::Build(pos) => {
                self.is_building = true;
                self.pos = pos;

                eraser_ctx
                    .buffer
                    .elements
                    .iter()
                    .filter(|(_, el)| !el.deleted())
                    .for_each(|(id, el)| {
                        if self.delete_candidates.contains_key(id) {
                            return;
                        }
                        if pointer_intersects_element(el, pos, None, self.radius as f64) {
                            self.delete_candidates.insert(*id, el.opacity());
                        }
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
                let event = super::Event::Delete(
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

    pub fn show_eraser_circle(&self, pos: egui::Pos2, eraser_ctx: &mut ToolContext<'_>) {
        let old_layer = eraser_ctx.painter.layer_id();
        eraser_ctx.painter.set_layer_id(egui::LayerId {
            order: egui::Order::PanelResizeLine,
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
