use bezier_rs::{Bezier, Identifier, Subpath};
use eframe::egui;
use minidom::Element;
use resvg::tiny_skia::Point;
use resvg::usvg::{Node, NodeKind};
use std::collections::HashMap;
use std::sync::mpsc;

const ERASER_THICKNESS: f32 = 5.0;
pub struct Eraser {
    pub rx: mpsc::Receiver<EraseEvent>,
    pub tx: mpsc::Sender<EraseEvent>,
    path_bounds: HashMap<String, (Subpath<ManipulatorGroupId>, f32)>,
    paths_to_delete: Vec<String>,
    last_pos: Option<egui::Pos2>,
}

pub enum EraseEvent {
    Start(egui::Pos2),
    End,
}
impl Eraser {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();

        Eraser { rx, tx, paths_to_delete: vec![], path_bounds: HashMap::default(), last_pos: None }
    }

    pub fn handle_events(&mut self, event: EraseEvent, root: &mut Element) -> String {
        match event {
            EraseEvent::Start(pos) => {
                self.path_bounds
                    .iter()
                    .for_each(|(id, (path, _path_thickness))| {
                        let bb = path.bounding_box();
                        if bb.is_none() {
                            return;
                        }
                        let bb = bb.unwrap();

                        let mut rect = egui::Rect::from_min_max(
                            egui::pos2(bb[0].x as f32, bb[0].y as f32),
                            egui::pos2(bb[1].x as f32, bb[1].y as f32),
                        );

                        // padding for small strokes
                        if rect.area() < 300.0 {
                            rect = rect.expand2(egui::vec2(20.0, 20.0));
                        }

                        // ui.painter().rect_stroke(rect, egui::Rounding::none(), egui::Stroke{ width: 1.0, color: egui::Color32::DEBUG_COLOR });
                        if !rect.contains(pos) {
                            return;
                        }

                        let last_pos = self.last_pos.unwrap_or(pos);

                        // todo: consider thickness using bezier rs::outline
                        let delete_brush = Subpath::new_line(
                            glam::DVec2 { x: last_pos.x as f64, y: last_pos.y as f64 },
                            glam::DVec2 { x: pos.x as f64, y: pos.y as f64 },
                        );

                        if !self.paths_to_delete.contains(id)
                            && !path
                                .subpath_intersections(&delete_brush, None, None)
                                .is_empty()
                        {
                            self.paths_to_delete.push(id.clone());
                            if let Some(node) = root.children_mut().find(|e| {
                                if let Some(id_attr) = e.attr("id") {
                                    id_attr == id
                                } else {
                                    false
                                }
                            }) {
                                node.set_attr("opacity", "0.5");
                            }
                        }
                    });
                self.last_pos = Some(pos);
            }
            EraseEvent::End => {
                self.paths_to_delete.iter().for_each(|id| {
                    if let Some(node) = root.children_mut().find(|e| {
                        if let Some(id_attr) = e.attr("id") {
                            id_attr == id
                        } else {
                            false
                        }
                    }) {
                        node.set_attr("d", "");
                    }
                });
                // actually remove them from the dom / content
            }
        }
        let mut buffer = Vec::new();
        root.write_to(&mut buffer).unwrap();
        // todo: handle unwrap
        std::str::from_utf8(&buffer)
            .unwrap()
            .replace("xmlns='' ", "")
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
                self.tx.send(EraseEvent::End).unwrap();
            }
        }
    }

    pub fn index_rects(&mut self, utree: &Node) {
        for el in utree.children() {
            if let NodeKind::Path(ref p) = *el.borrow() {
                // todo: this optimization relies on indexing only when paths are finished
                // if self.path_bounds.contains_key(&p.id) {
                //     continue;
                // }
                self.path_bounds
                    .insert(p.id.clone(), (convert_path_to_bezier(p.data.points()), 1.0));
            }
        }
    }
}

fn convert_path_to_bezier(data: &[Point]) -> Subpath<ManipulatorGroupId> {
    let mut bez = vec![];
    let mut i = 1;
    while i < data.len() - 2 {
        bez.push(Bezier::from_cubic_coordinates(
            data[i - 1].x as f64,
            data[i - 1].y as f64,
            data[i].x as f64,
            data[i].y as f64,
            data[i + 1].x as f64,
            data[i + 1].y as f64,
            data[i + 2].x as f64,
            data[i + 2].y as f64,
        ));
        i += 1;
    }
    Subpath::from_beziers(&bez, false)
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
struct ManipulatorGroupId; // Replace with your actual type

impl Identifier for ManipulatorGroupId {
    fn new() -> Self {
        ManipulatorGroupId
    }
}
