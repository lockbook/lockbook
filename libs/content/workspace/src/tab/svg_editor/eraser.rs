use resvg::usvg::Visibility;
use std::collections::HashSet;
use std::sync::mpsc;

use super::history::History;
use super::{util::pointer_interests_path, Buffer, DeleteElement};

pub struct Eraser {
    pub rx: mpsc::Receiver<EraseEvent>,
    pub tx: mpsc::Sender<EraseEvent>,
    pub thickness: f32,
    paths_to_delete: HashSet<String>,
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

        Eraser { rx, tx, paths_to_delete: HashSet::default(), thickness: 10.0, last_pos: None }
    }

    pub fn handle_events(&mut self, event: EraseEvent, buffer: &mut Buffer, history: &mut History) {
        match event {
            EraseEvent::Start(pos) => {
                buffer.elements.iter_mut().for_each(|(id, el)| {
                    if self.paths_to_delete.contains(id) {
                        return;
                    }
                    match el {
                        super::parser::Element::Path(path) => {
                            if pointer_interests_path(
                                &path.data,
                                pos,
                                self.last_pos,
                                self.thickness as f64,
                            ) {
                                self.paths_to_delete.insert(id.clone());
                            }
                        }
                        _ => todo!(),
                    }
                });

                self.paths_to_delete.iter().for_each(|id| {
                    if let Some(super::parser::Element::Path(path)) = buffer.elements.get_mut(id) {
                        path.opacity = 0.3;
                    }
                });

                self.last_pos = Some(pos);
            }
            EraseEvent::End => {
                if self.paths_to_delete.is_empty() {
                    return;
                }

                self.paths_to_delete.iter().for_each(|id| {
                    if let Some(super::parser::Element::Path(path)) = buffer.elements.get_mut(id) {
                        path.opacity = 1.0;
                        path.visibility = Visibility::Hidden;
                    }
                });

                history.save(super::Event::Delete(
                    self.paths_to_delete
                        .iter()
                        .map(|id| DeleteElement { id: id.to_owned() })
                        .collect(),
                ));

                self.paths_to_delete.clear();
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
