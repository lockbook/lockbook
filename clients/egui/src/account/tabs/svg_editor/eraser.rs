use bezier_rs::Bezier;
use eframe::egui;
use minidom::Element;
use std::collections::HashMap;
use std::sync::mpsc;

use super::{util, Buffer};

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

                    // first pass: check if the path bounding box contain the cursor.
                    // padding to account for low sampling rate scenarios and flat
                    // lines with empty bounding boxes
                    let padding = 50.0;
                    let bb = match path.bounding_box() {
                        Some(bb) => egui::Rect {
                            min: egui::pos2(bb[0].x as f32, bb[0].y as f32),
                            max: egui::pos2(bb[1].x as f32, bb[1].y as f32),
                        }
                        .expand(padding),
                        None => return,
                    };
                    let last_pos = self.last_pos.unwrap_or(pos.round());
                    if !(bb.contains(pos) || bb.contains(last_pos)) {
                        return;
                    }

                    // second more rigorous pass
                    let delete_brush = Bezier::from_linear_dvec2(
                        glam::dvec2(last_pos.x as f64, last_pos.y as f64),
                        glam::dvec2(pos.x as f64, pos.y as f64),
                    )
                    .outline(ERASER_THICKNESS as f64, bezier_rs::Cap::Round);

                    let is_inside_delete_brush = path.is_point()
                        && delete_brush
                            .contains_point(path.manipulator_groups().get(0).unwrap().anchor);
                    let intersects_delete_brush = !path
                        .subpath_intersections(&delete_brush, None, None)
                        .is_empty();

                    if intersects_delete_brush || is_inside_delete_brush {
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
                self.paths_to_delete.iter().for_each(|(id, _)| {
                    if let Some(n) = util::node_by_id(&mut buffer.current, id.to_string()) {
                        n.set_attr("opacity", "1");
                    }
                });

                buffer.save(super::Event::DeleteElements(super::DeleteElements {
                    elements: self.paths_to_delete.clone(),
                }));

                self.paths_to_delete.iter().for_each(|(id, _)| {
                    if let Some(node) = util::node_by_id(&mut buffer.current, id.to_string()) {
                        node.set_attr("d", "");
                    }
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
