use bezier_rs::{Bezier, Identifier, Subpath};
use eframe::egui;
use minidom::Element;
use resvg::tiny_skia::Point;
use resvg::usvg::{Node, NodeKind};
use std::collections::HashMap;
use std::sync::mpsc;

use super::util::node_by_id;

const ERASER_THICKNESS: f32 = 10.0;
pub struct Eraser {
    pub rx: mpsc::Receiver<EraseEvent>,
    pub tx: mpsc::Sender<EraseEvent>,
    path_bounds: HashMap<String, Subpath<ManipulatorGroupId>>,
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
                self.path_bounds.iter().for_each(|(id, path)| {
                    if self.paths_to_delete.contains(id) {
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

                    if path.is_point() {
                        let point = path.manipulator_groups().get(0).unwrap().anchor;
                        if delete_brush.contains_point(point) {
                            self.paths_to_delete.push(id.clone());
                        }
                    } else if !path
                        .subpath_intersections(&delete_brush, None, None)
                        .is_empty()
                    {
                        self.paths_to_delete.push(id.clone());
                    }
                });

                self.paths_to_delete.iter().for_each(|id| {
                    if let Some(node) = node_by_id(root, id.to_string()) {
                        node.set_attr("opacity", "0.5");
                    }
                });
                self.last_pos = Some(pos);
            }
            EraseEvent::End => {
                self.paths_to_delete.iter().for_each(|id| {
                    if let Some(node) = node_by_id(root, id.to_string()) {
                        node.set_attr("d", "");
                    }
                });
                self.paths_to_delete = vec![];
            }
        }
        let mut buffer = Vec::new();
        root.write_to(&mut buffer).unwrap();
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
                self.last_pos = None;
                self.tx.send(EraseEvent::End).unwrap();
            }
        }
    }

    pub fn index_rects(&mut self, utree: &Node) {
        for el in utree.children() {
            if let NodeKind::Path(ref p) = *el.borrow() {
                // todo: this optimization relies on history, index only when paths are modified
                // if self.path_bounds.contains_key(&p.id) {
                //     continue;
                // }
                self.path_bounds
                    .insert(p.id.clone(), convert_path_to_bezier(p.data.points()));
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
struct ManipulatorGroupId;

impl Identifier for ManipulatorGroupId {
    fn new() -> Self {
        ManipulatorGroupId
    }
}
