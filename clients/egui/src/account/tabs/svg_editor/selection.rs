use bezier_rs::Bezier;
use eframe::egui;

use super::{node_by_id, pointer_interests_path, Buffer};

pub struct Selection {
    last_pos: Option<egui::Pos2>,
    dragged_element: Option<DraggedElement>,
}

struct DraggedElement {
    id: String,
    original_pos: egui::Pos2,
    original_matrix: Option<[f64; 6]>,
}

impl Selection {
    pub fn new() -> Self {
        Selection { last_pos: None, dragged_element: None }
    }

    pub fn handle_input(
        &mut self, ui: &mut egui::Ui, working_rect: egui::Rect, buffer: &mut Buffer,
    ) {
        let pos = match ui.ctx().pointer_hover_pos() {
            Some(cp) => {
                if !working_rect.contains(cp) || !ui.is_enabled() {
                    return;
                }
                cp
            }
            None => return,
        };

        if let Some(de) = &mut self.dragged_element {
            ui.output_mut(|r| r.cursor_icon = egui::CursorIcon::Grab);

            if ui.input(|r| r.pointer.primary_released()) {
                self.dragged_element = None;
            } else if ui.input(|r| r.pointer.primary_down()) {
                let delta_x = pos.x - de.original_pos.x;
                let delta_y = pos.y - de.original_pos.y;
                if let Some(node) = node_by_id(&mut buffer.current, de.id.clone()) {
                    if let Some(transform) = node.attr("transform") {
                        if de.original_matrix.is_none() {
                            let transform = transform.to_owned();
                            // let a = "";
                            for segment in svgtypes::TransformListParser::from(transform.as_str()) {
                                let segment = match segment {
                                    Ok(v) => v,
                                    Err(_) => break,
                                };
                                match segment {
                                    svgtypes::TransformListToken::Matrix { a, b, c, d, e, f } => {
                                        de.original_matrix = Some([a, b, c, d, e, f]);
                                    }
                                    _ => {}
                                }
                            }
                        }
                    } 
                        println!("{:#?}", de.original_matrix);
                        node.set_attr(
                            "transform",
                            format!(
                                "matrix(1,0,0,1,{},{} )",
                                delta_x as f64 + de.original_matrix.unwrap_or_default()[4],
                                delta_y as f64 + de.original_matrix.unwrap_or_default()[5]
                            ),
                        );
                        // node.set_attr("transform", format!("matrix(1,0,0,1,{delta_x},{delta_y} )"));
                    
                    buffer.needs_path_map_update = true;
                }
            }
        } else {
            for (id, path) in buffer.paths.iter() {
                if pointer_interests_path(path, pos, self.last_pos, 10.0) {
                    ui.output_mut(|r| r.cursor_icon = egui::CursorIcon::Grab);
                    if ui.input(|r| r.pointer.primary_clicked()) {
                        self.dragged_element = Some(DraggedElement {
                            id: id.clone(),
                            original_pos: pos,
                            original_matrix: None,
                        });
                    }
                    break;
                }
            }
        }

        self.last_pos = Some(pos);
    }
}
