use minidom::Element;
use std::collections::HashMap;
use std::sync::mpsc;

use super::{util, util::pointer_interests_path, Buffer, DeleteElement};

const ERASER_THICKNESS: f32 = 10.0;
pub struct Eraser {
    pub rx: mpsc::Receiver<EraseEvent>,
    pub tx: mpsc::Sender<EraseEvent>,
    paths_to_delete: HashMap<String, Element>,
    last_pos: Option<egui::Pos2>,
}

pub enum EraseEvent {
    Start(egui::Pos2),
    End,
}
impl Eraser {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();

        Eraser { rx, tx, paths_to_delete: HashMap::default(), last_pos: None }
    }

    pub fn handle_events(&mut self, event: EraseEvent, buffer: &mut Buffer) {
        match event {
            EraseEvent::Start(pos) => {
                buffer.paths.iter().for_each(|(id, path)| {
                    if self.paths_to_delete.contains_key(id) {
                        return;
                    }

                    if pointer_interests_path(path, pos, self.last_pos, ERASER_THICKNESS as f64) {
                        if let Some(n) = buffer
                            .current
                            .children()
                            .find(|e| e.attr("id").unwrap_or_default().eq(&id.to_string()))
                        {
                            self.paths_to_delete.insert(id.clone(), n.clone());
                        }

                        if let Some(n) = util::node_by_id(&mut buffer.current, id.to_string()) {
                            n.set_attr("opacity", "0.3");
                        }
                    }
                });

                self.last_pos = Some(pos);
            }
            EraseEvent::End => {
                if self.paths_to_delete.is_empty() {
                    return;
                }

                self.paths_to_delete.iter().for_each(|(id, _)| {
                    if let Some(n) = util::node_by_id(&mut buffer.current, id.to_string()) {
                        n.set_attr("opacity", "1");
                    }
                });

                buffer.save(super::Event::Delete(
                    self.paths_to_delete
                        .iter()
                        .map(|(id, path_el)| DeleteElement {
                            id: id.to_owned(),
                            element: path_el.clone(),
                        })
                        .collect(),
                ));

                self.paths_to_delete.iter().for_each(|(id, _)| {
                    buffer.current.remove_child(id);
                });

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
                .circle_stroke(cursor_pos, ERASER_THICKNESS, stroke);
            ui.output_mut(|w| w.cursor_icon = egui::CursorIcon::None);

            if ui.input(|i| i.pointer.primary_down()) {
                self.tx.send(EraseEvent::Start(cursor_pos)).unwrap();
            }
            if ui.input(|i| i.pointer.primary_released()) {
                self.last_pos = None;
                self.tx.send(EraseEvent::End).unwrap();
            }
        }
    }
}
