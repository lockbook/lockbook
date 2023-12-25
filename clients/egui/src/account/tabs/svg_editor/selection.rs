use eframe::egui;

use super::{node_by_id, pointer_interests_path, Buffer, DeleteElement, TransformElement};

pub struct Selection {
    last_pos: Option<egui::Pos2>,
    selected_elements: Vec<SelectedElement>,
}

// i need to keep track of selected, but not dragging | selected and dragging |
struct SelectedElement {
    id: String,
    original_pos: egui::Pos2,
    original_matrix: (String, [f64; 6]),
}

/**
 * Todo:
 * - i need to save transports as a history event
 * - reach: allow copy paste selection
 * - reach lasso tool
 */

impl Selection {
    pub fn new() -> Self {
        Selection { last_pos: None, selected_elements: vec![] }
    }

    pub fn handle_input(
        &mut self, ui: &mut egui::Ui, working_rect: egui::Rect, buffer: &mut Buffer,
    ) {
        let pos = match ui.ctx().pointer_hover_pos() {
            Some(cp) => {
                if ui.is_enabled() {
                    cp
                } else {
                    return;
                }
            }
            None => egui::Pos2::ZERO,
        };

        let maybe_selected_el = self.detect_drag(buffer, pos, ui);
        if maybe_selected_el.is_some() {
            ui.output_mut(|r| r.cursor_icon = egui::CursorIcon::Grab);
        }

        // build up selected elements
        if ui.input(|r| r.pointer.primary_clicked()) {
            // is cursor inside of a selected element?
            let pos_over_selected_el = self
                .selected_elements
                .iter()
                .find(|el| {
                    let bb = buffer.paths.get(&el.id).unwrap().bounding_box().unwrap();
                    let rect = egui::Rect {
                        min: egui::pos2(bb[0].x as f32, bb[0].y as f32),
                        max: egui::pos2(bb[1].x as f32, bb[1].y as f32),
                    };
                    rect.contains(pos)
                })
                .is_some();

            // cursor is outside of a selected element, add elements
            if !pos_over_selected_el {
                if let Some(new_selected_el) = maybe_selected_el {
                    if ui.input(|r| r.modifiers.shift) {
                        self.selected_elements.push(new_selected_el);
                        self.selected_elements.iter_mut().for_each(|el| {
                            end_drag(buffer, el, pos);
                        })
                    } else {
                        self.selected_elements = vec![new_selected_el]
                    }
                } else {
                    self.selected_elements.clear();
                }
            }
        }

        for el in self.selected_elements.iter_mut() {
            let path = buffer.paths.get(&el.id).unwrap();
            let bb = path.bounding_box().unwrap();
            let rect = egui::Rect {
                min: egui::pos2(bb[0].x as f32, bb[0].y as f32),
                max: egui::pos2(bb[1].x as f32, bb[1].y as f32),
            };

            if rect.contains(pos) {
                ui.output_mut(|r| r.cursor_icon = egui::CursorIcon::Grab);
            }

            show_bb_rect(ui, bb, working_rect);

            if ui.input(|r| r.pointer.primary_released()) {
                end_drag(buffer, el, pos);
                println!("end");
            } else if ui.input(|r| r.pointer.primary_clicked()) {
                println!("start");
                el.original_pos = pos;
            } else if ui.input(|r| r.pointer.primary_down()) {
                let delta = egui::pos2(pos.x - el.original_pos.x, pos.y - el.original_pos.y);
                drag(delta, el, buffer);
            }

            let step_size = if ui.input(|r| r.modifiers.shift) { 7.0 } else { 2.0 };
            let delta = if ui.input(|r| r.key_down(egui::Key::ArrowDown)) {
                Some(egui::pos2(0.0, step_size))
            } else if ui.input(|r| r.key_down(egui::Key::ArrowLeft)) {
                Some(egui::pos2(-step_size, 0.0))
            } else if ui.input(|r| r.key_down(egui::Key::ArrowRight)) {
                Some(egui::pos2(step_size, 0.0))
            } else if ui.input(|r| r.key_down(egui::Key::ArrowUp)) {
                Some(egui::pos2(0.0, -step_size))
            } else {
                None
            };

            if let Some(d) = delta {
                end_drag(buffer, el, pos);
                drag(d, el, buffer);
            }
        }

        if ui.input(|r| r.key_pressed(egui::Key::Delete)) && !self.selected_elements.is_empty() {
            let elements = self
                .selected_elements
                .iter()
                .map(|el| {
                    let element = buffer
                        .current
                        .children()
                        .find(|node| node.attr("id").map_or(false, |id| id.eq(&el.id)))
                        .unwrap()
                        .clone();
                    DeleteElement { id: el.id.clone(), element }
                })
                .collect();

            let delete_event = super::Event::DeleteElements(elements);
            buffer.apply_event(&delete_event);
            buffer.save(delete_event);
            self.selected_elements.clear();
        }

        self.last_pos = Some(pos);
    }

    fn detect_drag(
        &mut self, buffer: &mut Buffer, pos: egui::Pos2, ui: &mut egui::Ui,
    ) -> Option<SelectedElement> {
        for (id, path) in buffer.paths.iter() {
            if pointer_interests_path(path, pos, self.last_pos, 10.0) {
                ui.output_mut(|r| r.cursor_icon = egui::CursorIcon::Grab);
                let transform = buffer
                    .current
                    .children()
                    .find(|el| el.attr("id").unwrap_or_default().eq(id))
                    .unwrap()
                    .attr("transform")
                    .unwrap_or_default();
                return Some(SelectedElement {
                    id: id.clone(),
                    original_pos: pos,
                    original_matrix: parse_transform(transform.to_string()),
                });
            }
        }
        None
    }
}

fn end_drag(buffer: &mut Buffer, el: &mut SelectedElement, pos: egui::Pos2) {
    el.original_pos = pos;
    if let Some(node) = buffer
        .current
        .children()
        .find(|node| node.attr("id").map_or(false, |id| id.eq(&el.id)))
    {
        if let Some(transform) = node.attr("transform") {
            let transform = transform.to_owned();
            buffer.save(super::Event::TransformElements(vec![TransformElement {
                id: el.id.to_owned(),
                old_transform: el.original_matrix.clone().0,
                new_transform: transform.clone(),
            }]));

            parse_transform(transform);
        }
    }
}

fn parse_transform(transform: String) -> (String, [f64; 6]) {
    for segment in svgtypes::TransformListParser::from(transform.as_str()) {
        let segment = match segment {
            Ok(v) => v,
            Err(_) => break,
        };
        match segment {
            svgtypes::TransformListToken::Matrix { a, b, c, d, e, f } => {
                return (transform.clone(), [a, b, c, d, e, f]);
            }
            _ => {}
        }
    }
    let identity_matrix = [0, 1, 1, 0, 0, 0].map(|f| f as f64);
    ("".to_string(), identity_matrix)
}

fn show_bb_rect(ui: &mut egui::Ui, mut bb: [glam::DVec2; 2], working_rect: egui::Rect) {
    bb[0].x = bb[0].x.max(working_rect.left() as f64);
    bb[0].y = bb[0].y.max(working_rect.top() as f64);

    bb[1].x = bb[1].x.min(working_rect.right() as f64);
    bb[1].y = bb[1].y.min(working_rect.bottom() as f64);

    if bb[1].x < bb[0].x || bb[1].y < bb[0].y {
        return;
    }

    let line_segments = [
        [egui::pos2(bb[0].x as f32, bb[0].y as f32), egui::pos2(bb[1].x as f32, bb[0].y as f32)],
        [egui::pos2(bb[0].x as f32, bb[1].y as f32), egui::pos2(bb[1].x as f32, bb[1].y as f32)],
        [egui::pos2(bb[0].x as f32, bb[0].y as f32), egui::pos2(bb[0].x as f32, bb[1].y as f32)],
        [egui::pos2(bb[1].x as f32, bb[0].y as f32), egui::pos2(bb[1].x as f32, bb[1].y as f32)],
    ];

    line_segments.iter().for_each(|line_segment| {
        ui.painter().add(egui::Shape::dashed_line(
            line_segment,
            egui::Stroke { width: 1.0, color: ui.visuals().hyperlink_color },
            3.,
            6.,
        ));
    });
}

fn drag(delta: egui::Pos2, de: &mut SelectedElement, buffer: &mut Buffer) {
    println!("delta: {:#?}", delta);
    println!("original: {}, {}", de.original_matrix.clone().1[4], de.original_matrix.clone().1[5]);

    if let Some(node) = node_by_id(&mut buffer.current, de.id.clone()) {
        node.set_attr(
            "transform",
            format!(
                "matrix(1,0,0,1,{},{} )",
                delta.x as f64 + de.original_matrix.clone().1[4],
                delta.y as f64 + de.original_matrix.clone().1[5]
            ),
        );
        buffer.needs_path_map_update = true;
    }
}
