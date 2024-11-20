use std::collections::HashMap;

use lb_rs::svg::element::Element;
use lb_rs::Uuid;

use super::toolbar::ToolContext;
use super::util::is_multi_touch;
use super::{util::pointer_intersects_element, DeleteElement};

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

        if let Some(event) =
            self.setup_events(ui, eraser_ctx.painter, eraser_ctx.painter.clip_rect())
        {
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
                        if let Some(el) = eraser_ctx.buffer.elements.get_mut(id) {
                            if opacity == el.opacity() {
                                match el {
                                    Element::Path(p) => {
                                        p.opacity *= 0.3;
                                        p.diff_state.opacity_changed = true
                                    }
                                    Element::Image(img) => {
                                        img.opacity = 0.3;
                                        img.diff_state.opacity_changed = true
                                    }
                                    Element::Text(_) => todo!(),
                                }
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
                        if let Some(el) = eraser_ctx.buffer.elements.get_mut(id) {
                            match el {
                                Element::Path(p) => {
                                    p.opacity = opacity;
                                    p.deleted = true;
                                    p.diff_state.delete_changed = true;
                                }
                                Element::Image(img) => {
                                    img.opacity = opacity;
                                    img.deleted = true;
                                    img.diff_state.delete_changed = true
                                }
                                Element::Text(_) => todo!(),
                            }
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
        &mut self, ui: &mut egui::Ui, painter: &egui::Painter, inner_rect: egui::Rect,
    ) -> Option<EraseEvent> {
        if let Some(cursor_pos) = ui.ctx().pointer_hover_pos() {
            if !inner_rect.contains(cursor_pos) || !ui.is_enabled() {
                return None;
            }

            self.draw_eraser_cursor(ui, painter, cursor_pos);

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
