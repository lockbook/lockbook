use bezier_rs::{Bezier, Identifier, Subpath};
use eframe::egui;
use minidom::Element;
use resvg::tiny_skia::{PathBuilder, Point};
use resvg::usvg::{Node, NodeKind, Tree};
use std::collections::HashMap;
use std::sync::mpsc;

pub struct Eraser {
    pub rx: mpsc::Receiver<EraseEvent>,
    pub tx: mpsc::Sender<EraseEvent>,
    path_bounds: HashMap<String, Subpath<ManipulatorGroupId>>,
    paths_to_delete: Vec<String>,
}

pub enum EraseEvent {
    Start(egui::Pos2),
    End,
}
impl Eraser {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();

        Eraser { rx, tx, paths_to_delete: vec![], path_bounds: HashMap::default() }
    }

    pub fn handle_events(
        &mut self, event: EraseEvent, root: &mut Element, ui: &mut egui::Ui,
    ) -> String {
        match event {
            EraseEvent::Start(pos) => {
                self.path_bounds.iter().for_each(|(id, path)| {
                    if let Some(bb) = path.bounding_box() {
                        let mut rect = egui::Rect::from_min_max(
                            egui::pos2(bb[0].x as f32, bb[0].y as f32),
                            egui::pos2(bb[1].x as f32, bb[1].y as f32),
                        );
                        
                        if rect.width() < 10. {
                            rect = rect.expand2(egui::vec2(10.0, 0.0));
                        }
                        if rect.height() < 10. {
                            rect = rect.expand2(egui::vec2(0.0, 10.0));
                        }
                        // ui.painter().rect_stroke(rect, egui::Rounding::none(), egui::Stroke{ width: 1.0, color: egui::Color32::DEBUG_COLOR });
                        println!("w: {}, h: {}", rect.width(), rect.height());
                        if !rect.contains(pos) {
                            return;
                        }

                        // can use subpath new_rect instead of this
                        let mut pb = PathBuilder::new();
                        pb.push_circle(pos.x, pos.y, 5.0);
                        let delete_brush = convert_path_to_bezier(pb.finish().unwrap().points());
                        

                        if !path
                            .subpath_intersections(&delete_brush, None, None)
                            .is_empty()
                        {
                            self.paths_to_delete.push(id.clone());
                            if let Some(node) = root.children_mut().find(|e| {
                                if let Some(id_attr) = e.attr("id") {
                                    id_attr == id.to_string()
                                } else {
                                    false
                                }
                            }) {
                                node.set_attr("opacity", "0.5");
                            }
                        }
                    }
                })
                // check if the cursor falls in any of the rects
                // if yes
                // use the second pass of an intersection algo
                // if yes add to the to delete array and redraw the path gray
            }
            EraseEvent::End => {
                self.paths_to_delete.iter().for_each(|id| {
                    if let Some(node) = root.children_mut().find(|e| {
                        if let Some(id_attr) = e.attr("id") {
                            id_attr == id.to_string()
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
            ui.painter().circle_stroke(cursor_pos, 5.0, stroke);
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
struct ManipulatorGroupId; // Replace with your actual type

impl Identifier for ManipulatorGroupId {
    fn new() -> Self {
        ManipulatorGroupId
    }
}
