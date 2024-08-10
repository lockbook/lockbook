use std::collections::HashMap;

use super::history::History;
use super::{util::pointer_intersects_element, Buffer, DeleteElement};

pub struct Eraser {
    pub thickness: f32,
    delete_candidates: HashMap<String, bool>,
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
pub const DEFAULT_ERASER_THICKNESS: f32 = 5.0;

impl Eraser {
    pub fn new() -> Self {
        Eraser {
            delete_candidates: HashMap::default(),
            thickness: DEFAULT_ERASER_THICKNESS,
            last_pos: None,
        }
    }

    pub fn handle_input(
        &mut self, ui: &mut egui::Ui, inner_rect: egui::Rect, buffer: &mut Buffer,
        history: &mut History,
    ) {
        let event = match self.setup_events(ui, inner_rect) {
            Some(e) => e,
            None => return,
        };

        match event {
            EraseEvent::Start(pos) => {
                buffer
                    .elements
                    .iter()
                    .filter(|(_, el)| !el.deleted())
                    .for_each(|(id, el)| {
                        if self.delete_candidates.contains_key(id) {
                            return;
                        }
                        if pointer_intersects_element(el, pos, self.last_pos, self.thickness as f64)
                        {
                            self.delete_candidates.insert(id.clone(), false);
                        }
                    });

                self.delete_candidates
                    .iter_mut()
                    .for_each(|(id, has_decreased_opacity)| {
                        if let Some(el) = buffer.elements.get_mut(id) {
                            if !*has_decreased_opacity {
                                match el {
                                    super::parser::Element::Path(p) => {
                                        p.opacity *= 0.3;
                                        p.diff_state.opacity_changed = true
                                    }
                                    super::parser::Element::Image(img) => {
                                        img.opacity = 0.3;
                                        img.diff_state.opacity_changed = true
                                    }
                                    super::parser::Element::Text(_) => todo!(),
                                }
                            }
                        };
                        *has_decreased_opacity = true;
                    });

                self.last_pos = Some(pos);
            }
            EraseEvent::End => {
                if self.delete_candidates.is_empty() {
                    return;
                }

                self.delete_candidates.iter().for_each(|(id, _)| {
                    if let Some(el) = buffer.elements.get_mut(id) {
                        match el {
                            super::parser::Element::Path(p) => {
                                p.opacity = 1.0;
                                p.deleted = true;
                                p.diff_state.delete_changed = true;
                            }
                            super::parser::Element::Image(img) => {
                                img.opacity = 1.0;
                                img.deleted = true;
                                img.diff_state.delete_changed = true
                            }
                            super::parser::Element::Text(_) => todo!(),
                        }
                    };
                });
                let event = super::Event::Delete(
                    self.delete_candidates
                        .keys()
                        .map(|id| DeleteElement { id: id.to_owned() })
                        .collect(),
                );

                history.save(event.clone());

                self.delete_candidates.clear();
            }
        }
    }

    pub fn setup_events(
        &mut self, ui: &mut egui::Ui, inner_rect: egui::Rect,
    ) -> Option<EraseEvent> {
        if let Some(cursor_pos) = ui.ctx().pointer_hover_pos() {
            if !inner_rect.contains(cursor_pos) || !ui.is_enabled() {
                return None;
            }

            let stroke = egui::Stroke { width: 1.0, color: ui.visuals().text_color() };
            ui.painter()
                .circle_stroke(cursor_pos, self.thickness, stroke);
            ui.output_mut(|w| w.cursor_icon = egui::CursorIcon::None);
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
}