use resvg::usvg::Visibility;
use std::collections::HashSet;
use std::sync::mpsc;

use super::history::History;
use super::{util::pointer_intersects_element, Buffer, DeleteElement};

pub struct Eraser {
    pub rx: mpsc::Receiver<EraseEvent>,
    pub tx: mpsc::Sender<EraseEvent>,
    pub thickness: f32,
    delete_candidates: HashSet<String>,
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

impl Eraser {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();

        Eraser { rx, tx, delete_candidates: HashSet::default(), thickness: 10.0, last_pos: None }
    }

    pub fn handle_events(&mut self, event: EraseEvent, buffer: &mut Buffer, history: &mut History) {
        match event {
            EraseEvent::Start(pos) => {
                buffer.elements.iter().for_each(|(id, el)| {
                    if self.delete_candidates.contains(id) {
                        return;
                    }
                    if pointer_intersects_element(&el, pos, self.last_pos, self.thickness as f64) {
                        self.delete_candidates.insert(id.clone());
                    }
                });

                self.delete_candidates.iter().for_each(|id| {
                    if let Some(el) = buffer.elements.get_mut(id) {
                        match el {
                            super::parser::Element::Path(p) => p.opacity = 0.3,
                            super::parser::Element::Image(img) => img.opacity = 0.3,
                            super::parser::Element::Text(_) => todo!(),
                        }
                    };
                });

                self.last_pos = Some(pos);
            }
            EraseEvent::End => {
                if self.delete_candidates.is_empty() {
                    return;
                }

                self.delete_candidates.iter().for_each(|id| {
                    if let Some(el) = buffer.elements.get_mut(id) {
                        match el {
                            super::parser::Element::Path(p) => {
                                p.opacity = 1.0;
                            }
                            super::parser::Element::Image(img) => {
                                img.opacity = 1.0;
                            }
                            super::parser::Element::Text(_) => todo!(),
                        }
                    };
                });
                let event = super::Event::Delete(
                    self.delete_candidates
                        .iter()
                        .map(|id| DeleteElement { id: id.to_owned() })
                        .collect(),
                );

                // todo: figure out if the history api should automatically apply the event on save
                history.save(event.clone());
                history.apply_event(&event, buffer);

                self.delete_candidates.clear();
            }
        }
    }

    pub fn setup_events(&mut self, ui: &mut egui::Ui, inner_rect: egui::Rect) {
        if let Some(cursor_pos) = ui.ctx().pointer_hover_pos() {
            if !inner_rect.contains(cursor_pos) || !ui.is_enabled() {
                return;
            }

            let stroke = egui::Stroke { width: 1.0, color: ui.visuals().text_color() };
            ui.painter()
                .circle_stroke(cursor_pos, self.thickness, stroke);
            ui.output_mut(|w| w.cursor_icon = egui::CursorIcon::None);
            if ui.input(|i| i.pointer.primary_down()) {
                self.tx.send(EraseEvent::Start(cursor_pos)).unwrap();
            }
            if ui.input(|i| i.pointer.primary_released()) {
                self.last_pos = None;
                self.tx.send(EraseEvent::End).unwrap();
            }
        } else {
            self.last_pos = None;
            self.tx.send(EraseEvent::End).unwrap();
        }
    }
}
