use eframe::egui;

use super::{
    node_by_id, pointer_interests_path, util::deserialize_transform, Buffer, DeleteElement,
    TransformElement,
};

pub struct Selection {
    last_pos: Option<egui::Pos2>,
    selected_elements: Vec<SelectedElement>,
    laso_original_pos: Option<egui::Pos2>,
    laso_rect: Option<egui::Rect>,
}

struct SelectedElement {
    id: String,
    original_pos: egui::Pos2,
    original_matrix: (String, [f64; 6]),
}

impl Selection {
    pub fn new() -> Self {
        Selection {
            last_pos: None,
            selected_elements: vec![],
            laso_original_pos: None,
            laso_rect: None,
        }
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
            let pos_over_selected_el = self.selected_elements.iter().any(|el| {
                let bb = buffer.paths.get(&el.id).unwrap().bounding_box().unwrap();
                let rect = egui::Rect {
                    min: egui::pos2(bb[0].x as f32, bb[0].y as f32),
                    max: egui::pos2(bb[1].x as f32, bb[1].y as f32),
                };
                rect.contains(pos)
            });

            // cursor is outside of a selected element, add elements
            if !pos_over_selected_el {
                if let Some(new_selected_el) = maybe_selected_el {
                    if ui.input(|r| r.modifiers.shift) {
                        self.selected_elements.push(new_selected_el);
                        // end_drag(buffer, &mut self.selected_elements, pos);
                    } else {
                        self.selected_elements = vec![new_selected_el]
                    }
                } else {
                    self.selected_elements.clear();
                    self.laso_original_pos = Some(pos);
                }
            }
        }

        if self.selected_elements.is_empty() && self.laso_original_pos.is_some() {
            if ui.input(|r| r.pointer.primary_down()) {
                let mut corners = [self.laso_original_pos.unwrap(), pos];
                corners.sort_by(|a, b| (a.x.total_cmp(&b.x)));
                let mut rect = egui::Rect { min: corners[0], max: corners[1] };
                if rect.height() < 0. {
                    std::mem::swap(&mut rect.min.y, &mut rect.max.y)
                }
                if rect.width() < 0. {
                    std::mem::swap(&mut rect.min.x, &mut rect.max.x)
                }

                rect.min.x = rect.min.x.max(working_rect.left());
                rect.min.y = rect.min.y.max(working_rect.top());

                rect.max.x = rect.max.x.min(working_rect.right());
                rect.max.y = rect.max.y.min(working_rect.bottom());

                self.laso_rect = Some(rect);
                ui.painter().rect_filled(
                    rect,
                    egui::Rounding::none(),
                    ui.visuals().hyperlink_color.gamma_multiply(0.1),
                )
            } else if ui.input(|r| r.pointer.primary_released()) {
                // if the path bounding box intersects with the laso rect then it's a match
                buffer.paths.iter().for_each(|(id, path)| {
                    let bb = path.bounding_box().unwrap();
                    let path_rect = egui::Rect {
                        min: egui::pos2(bb[0].x as f32, bb[0].y as f32),
                        max: egui::pos2(bb[1].x as f32, bb[1].y as f32),
                    };

                    if self.laso_rect.unwrap().intersects(path_rect) {
                        let transform = buffer
                            .current
                            .children()
                            .find(|el| el.attr("id").unwrap_or_default().eq(id))
                            .unwrap()
                            .attr("transform")
                            .unwrap_or_default();

                        self.selected_elements.push(SelectedElement {
                            id: id.to_owned(),
                            original_pos: pos,
                            original_matrix: (
                                transform.to_string(),
                                deserialize_transform(transform),
                            ),
                        });
                    }
                });
                self.laso_original_pos = None;
            }
        }

        let mut transform_origin_dirty = false;
        let mut history_dirty = false;

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
                transform_origin_dirty = true;
                history_dirty = true;
                break;
            } else if ui.input(|r| r.pointer.primary_clicked()) {
                transform_origin_dirty = true;
                break;
            } else if ui.input(|r| r.pointer.primary_down()) {
                let mut delta = egui::pos2(pos.x - el.original_pos.x, pos.y - el.original_pos.y);
                if let Some(transform) = buffer.current.attr("transform") {
                    let transform = deserialize_transform(transform);
                    delta.x /= transform[0] as f32;
                    delta.y /= transform[3] as f32;
                }
                drag(delta, el, buffer);
            }

            if ui.input(|r| r.key_pressed(egui::Key::ArrowDown))
                || ui.input(|r| r.key_pressed(egui::Key::ArrowUp))
                || ui.input(|r| r.key_pressed(egui::Key::ArrowLeft))
                || ui.input(|r| r.key_pressed(egui::Key::ArrowRight))
            {
                transform_origin_dirty = true;
                break;
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
                transform_origin_dirty = true;
                history_dirty = true;
                drag(d, el, buffer);
            }
        }

        if transform_origin_dirty {
            end_drag(buffer, &mut self.selected_elements, pos, history_dirty);
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

            let delete_event = super::Event::Delete(elements);
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
                    original_matrix: (transform.to_string(), deserialize_transform(transform)),
                });
            }
        }
        None
    }
}

fn end_drag(buffer: &mut Buffer, els: &mut [SelectedElement], pos: egui::Pos2, save_event: bool) {
    let events: Vec<TransformElement> = els
        .iter_mut()
        .filter_map(|el| {
            el.original_pos = pos;
            if let Some(node) = buffer
                .current
                .children()
                .find(|node| node.attr("id").map_or(false, |id| id.eq(&el.id)))
            {
                if let Some(new_transform) = node.attr("transform") {
                    let new_transform =
                        (new_transform.to_string(), deserialize_transform(new_transform));

                    let old_transform = el.original_matrix.clone();
                    let delta = egui::pos2(
                        (new_transform.1[4] - old_transform.1[4]).abs() as f32,
                        (new_transform.1[5] - old_transform.1[5]).abs() as f32,
                    );

                    el.original_matrix = new_transform.clone();

                    let history_threshold = 1.0;
                    if save_event && (delta.y > history_threshold || delta.x > history_threshold) {
                        Some(TransformElement {
                            id: el.id.to_owned(),
                            old_transform: old_transform.0,
                            new_transform: new_transform.0,
                        })
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect();
    if !events.is_empty() {
        buffer.save(super::Event::Transform(events));
    }
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
