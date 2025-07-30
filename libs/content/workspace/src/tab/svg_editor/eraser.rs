use std::collections::HashMap;

use lb_rs::Uuid;
use lb_rs::model::svg::element::Element;

use super::DeleteElement;
use super::toolbar::ToolContext;
use super::util::{is_multi_touch, pointer_intersects_element};

pub struct Eraser {
    pub radius: f32,
    delete_candidates: HashMap<Uuid, f32>,
    last_pos: Option<egui::Pos2>,
}

pub enum EraseEvent {
    Start(egui::Pos2),
    End,
}

impl Default for Eraser {
    fn default() -> Self {
        Self::new()
    }
}
pub const DEFAULT_ERASER_RADIUS: f32 = 5.0;

impl Eraser {
    pub fn new() -> Self {
        Eraser {
            delete_candidates: HashMap::default(),
            radius: DEFAULT_ERASER_RADIUS,
            last_pos: None,
        }
    }

    pub fn handle_input(&mut self, ui: &mut egui::Ui, eraser_ctx: &mut ToolContext) {
        if is_multi_touch(ui)
            || (eraser_ctx.is_locked_vw_pen_only && eraser_ctx.settings.pencil_only_drawing)
        {
            return;
        }

        if let Some(event) = self.setup_events(ui, eraser_ctx.painter) {
            match event {
                EraseEvent::Start(pos) => {
                    eraser_ctx
                        .buffer
                        .elements
                        .iter()
                        .filter(|(_, el)| !el.deleted())
                        .for_each(|(id, el)| {
                            if self.delete_candidates.contains_key(id) {
                                return;
                            }
                            if pointer_intersects_element(
                                el,
                                pos,
                                self.last_pos,
                                self.radius as f64,
                            ) {
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

                    self.last_pos = Some(pos);
                }
                EraseEvent::End => {
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
            }
        }
    }

    pub fn setup_events(
        &mut self, ui: &mut egui::Ui, painter: &mut egui::Painter,
    ) -> Option<EraseEvent> {
        let inner_rect = painter.clip_rect();

        if let Some(cursor_pos) = ui.ctx().pointer_hover_pos() {
            if !inner_rect.contains(cursor_pos) || !ui.is_enabled() {
                return None;
            }

            let old_layer = painter.layer_id();
            painter.set_layer_id(egui::LayerId {
                order: egui::Order::PanelResizeLine,
                id: "eraser_overlay".into(),
            });

            self.draw_eraser_cursor(ui, painter, cursor_pos);

            painter.set_layer_id(old_layer);

            if ui.input(|i| i.pointer.primary_down()) {
                return Some(EraseEvent::Start(cursor_pos));
            }
            if ui.input(|i| i.pointer.primary_released()) {
                self.last_pos = None;
                Some(EraseEvent::End)
            } else {
                None
            }
        } else {
            self.last_pos = None;
            Some(EraseEvent::End)
        }
    }

    pub fn draw_eraser_cursor(
        &mut self, ui: &mut egui::Ui, painter: &egui::Painter, cursor_pos: egui::Pos2,
    ) {
        let stroke = egui::Stroke { width: 1.0, color: ui.visuals().text_color() };
        painter.circle_stroke(cursor_pos, self.radius, stroke);

        // todo: apple integration doesn't support this correctly.
        // ui.output_mut(|w| w.cursor_icon = egui::CursorIcon::None);
    }
}
