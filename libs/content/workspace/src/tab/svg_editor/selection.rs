<<<<<<< HEAD:libs/content/workspace/src/tab/svg_editor/selection.rs
use super::{
    history::TransformElement,
    node_by_id,
    util::{deserialize_transform, pointer_interests_path, serialize_transform},
    Buffer, DeleteElement,
=======
use bezier_rs::Subpath;
use eframe::egui;
use glam::{DAffine2, DMat2, DVec2};

use super::{
    history::ManipulatorGroupId,
    node_by_id, pointer_interests_path,
    util::{deserialize_transform, serialize_transform},
    Buffer, DeleteElement, TransformElement,
>>>>>>> 5ce97131 (Mouse based element scaling):clients/egui/src/account/tabs/svg_editor/selection.rs
};

// todo: consider making this value dynamic depending on the scale of the element
const SCALE_BRUSH_SIZE: f64 = 10.0;

pub struct Selection {
    last_pos: Option<egui::Pos2>,
    selected_elements: Vec<SelectedElement>,
    laso_original_pos: Option<egui::Pos2>,
    laso_rect: Option<egui::Rect>,
    current_op: SelectionOperation,
}

struct SelectedElement {
    id: String,
    original_pos: egui::Pos2,
    original_matrix: (String, [f64; 6]),
}

impl Default for Selection {
    fn default() -> Self {
        Self::new()
    }
}

impl Selection {
    pub fn new() -> Self {
        Selection {
            last_pos: None,
            selected_elements: vec![],
            laso_original_pos: None,
            laso_rect: None,
            current_op: SelectionOperation::Idle,
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

        let mut maybe_selected_el = None;

        if matches!(self.current_op, SelectionOperation::Idle) {
            maybe_selected_el = self.detect_drag(buffer, pos);
            if maybe_selected_el.is_some() {
                ui.output_mut(|r| r.cursor_icon = egui::CursorIcon::Grab);
            }
        }

        // build up selected elements
        if ui.input(|r| r.pointer.primary_clicked()) {
            // is cursor inside of a selected element?
            let pos_over_selected_el = self.selected_elements.iter().any(|el| {
                let bb = buffer.paths.get(&el.id).unwrap().bounding_box().unwrap();
                let rect = egui::Rect {
                    min: egui::pos2(bb[0].x as f32, bb[0].y as f32),
                    max: egui::pos2(bb[1].x as f32, bb[1].y as f32),
                }
                // account for the fact that scaling operation can start with the
                // cursor slightly outside of the bounding box
                .expand(SCALE_BRUSH_SIZE as f32);

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
            let res = show_bb_rect(ui, &path, working_rect, pos);

            if ui.input(|r| r.pointer.primary_released()) {
                transform_origin_dirty = true;
                history_dirty = true;
                self.current_op = SelectionOperation::Idle;
            } else if ui.input(|r| r.pointer.primary_clicked()) {
                transform_origin_dirty = true;
            } else if ui.input(|r| r.pointer.primary_down()) {
                if matches!(self.current_op, SelectionOperation::Idle) {
                    if let Some(r) = res {
                        self.current_op = r.current_op;
                        ui.output_mut(|w| w.cursor_icon = r.cursor_icon);
                    }
                }

                match self.current_op {
                    SelectionOperation::Drag => {
                        let mut delta =
                            egui::pos2(pos.x - el.original_pos.x, pos.y - el.original_pos.y);
                        if let Some(transform) = buffer.current.attr("transform") {
                            let transform = deserialize_transform(transform);
                            delta.x /= transform[0] as f32;
                            delta.y /= transform[3] as f32;
                        }
                        drag(delta, el, buffer);
                        ui.output_mut(|w| w.cursor_icon = egui::CursorIcon::Grabbing);
                    }
                    SelectionOperation::HorizontalScale | SelectionOperation::VerticalScale => {
                        zoom_to_pos(pos, el, buffer, &self.current_op);
                        ui.output_mut(|w| w.cursor_icon = egui::CursorIcon::None);
                    }
                    SelectionOperation::Idle => {}
                }
            } else {
                if let Some(r) = res {
                    ui.output_mut(|w| w.cursor_icon = r.cursor_icon);
                }
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

            let is_scaling_up = ui.input(|r| r.key_pressed(egui::Key::PlusEquals));
            let is_scaling_down = ui.input(|r| r.key_pressed(egui::Key::Minus));

            let scale_factor = if is_scaling_up {
                1.1
            } else if is_scaling_down {
                0.9
            } else {
                1.0
            };

            if is_scaling_down || is_scaling_down {
                zoom_from_center(scale_factor, el, buffer);
                transform_origin_dirty = true;
                history_dirty = true;
            }
        }

        if transform_origin_dirty {
            end_drag(buffer, &mut self.selected_elements, pos, history_dirty);
        }

        if ui.input(|r| r.key_pressed(egui::Key::Backspace)) && !self.selected_elements.is_empty() {
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

    fn detect_drag(&mut self, buffer: &mut Buffer, pos: egui::Pos2) -> Option<SelectedElement> {
        for (id, path) in buffer.paths.iter() {
            if pointer_interests_path(path, pos, self.last_pos, 10.0) {
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

#[derive(Default)]
struct SelectionRect {
    left: Option<Subpath<ManipulatorGroupId>>,
    right: Option<Subpath<ManipulatorGroupId>>,
    top: Option<Subpath<ManipulatorGroupId>>,
    bottom: Option<Subpath<ManipulatorGroupId>>,
}

impl SelectionRect {
    fn show(&self, ui: &mut egui::Ui) {
        if let Some(top_path) = &self.top {
            self.show_subpath(&top_path, ui);
        };
        if let Some(bottom_path) = &self.bottom {
            self.show_subpath(&bottom_path, ui);
        };

        if let Some(left_path) = &self.left {
            self.show_subpath(&left_path, ui);

            if let Some(_) = &self.top {
                let corner = left_path.get_segment(0).unwrap().start();
                self.show_corner(corner, ui);
            }
            if let Some(_) = &self.bottom {
                let corner = left_path.get_segment(0).unwrap().end();
                self.show_corner(corner, ui);
            }
        };
        if let Some(right_path) = &self.right {
            self.show_subpath(&right_path, ui);

            if let Some(_) = &self.top {
                let corner = right_path.get_segment(0).unwrap().start();
                self.show_corner(corner, ui);
            }
            if let Some(_) = &self.bottom {
                let corner = right_path.get_segment(0).unwrap().end();
                self.show_corner(corner, ui);
            }
        };
    }

    fn show_subpath(&self, path: &Subpath<ManipulatorGroupId>, ui: &mut egui::Ui) {
        let line_segment = path.get_segment(0).unwrap();
        let line_segment = [
            egui::pos2(line_segment.start().x as f32, line_segment.start().y as f32),
            egui::pos2(line_segment.end().x as f32, line_segment.end().y as f32),
        ];
        ui.painter().line_segment(
            line_segment,
            egui::Stroke { width: 1.0, color: ui.visuals().hyperlink_color },
        );
    }

    fn show_corner(&self, corner: DVec2, ui: &mut egui::Ui) {
        let handle_side_length = 8.0; // handle is a square
        let corner = egui::pos2(corner.x as f32, corner.y as f32);
        let rect = egui::Rect {
            min: egui::pos2(
                corner.x - handle_side_length / 2.0,
                corner.y - handle_side_length / 2.0,
            ),
            max: egui::pos2(
                corner.x + handle_side_length / 2.0,
                corner.y + handle_side_length / 2.0,
            ),
        };
        ui.painter().rect(
            rect,
            egui::Rounding::none(),
            egui::Color32::WHITE,
            egui::Stroke { width: 1.0, color: ui.visuals().hyperlink_color },
        )
    }
}

enum SelectionOperation {
    Drag,
    HorizontalScale,
    VerticalScale,
    Idle,
}

struct SelectionResponse {
    current_op: SelectionOperation,
    cursor_icon: egui::CursorIcon,
}

fn show_bb_rect(
    ui: &mut egui::Ui, path: &Subpath<ManipulatorGroupId>, working_rect: egui::Rect,
    cursor_pos: egui::Pos2,
) -> Option<SelectionResponse> {
    let bb = match path.bounding_box() {
        Some(b) => b,
        None => {
            println!("none");
            return None;
        }
    };
    let mut clipped_bb = bb.clone();
    clipped_bb[0].x = clipped_bb[0].x.max(working_rect.left() as f64);
    clipped_bb[0].y = clipped_bb[0].y.max(working_rect.top() as f64);

    clipped_bb[1].x = clipped_bb[1].x.min(working_rect.right() as f64);
    clipped_bb[1].y = clipped_bb[1].y.min(working_rect.bottom() as f64);

    let selection_rect = SelectionRect {
        left: if clipped_bb[0].x == bb[0].x {
            Some(Subpath::from_anchors(
                [
                    DVec2 { x: clipped_bb[0].x, y: clipped_bb[0].y },
                    DVec2 { x: clipped_bb[0].x, y: clipped_bb[1].y },
                ],
                false,
            ))
        } else {
            None
        },
        right: if clipped_bb[1].x == bb[1].x {
            Some(Subpath::from_anchors(
                [
                    DVec2 { x: clipped_bb[1].x, y: clipped_bb[0].y },
                    DVec2 { x: clipped_bb[1].x, y: clipped_bb[1].y },
                ],
                false,
            ))
        } else {
            None
        },
        top: if clipped_bb[0].y == bb[0].y {
            Some(Subpath::from_anchors(
                [
                    DVec2 { x: clipped_bb[0].x, y: clipped_bb[0].y },
                    DVec2 { x: clipped_bb[1].x, y: clipped_bb[0].y },
                ],
                false,
            ))
        } else {
            None
        },
        bottom: if clipped_bb[1].y == bb[1].y {
            Some(Subpath::from_anchors(
                [
                    DVec2 { x: clipped_bb[0].x, y: clipped_bb[1].y },
                    DVec2 { x: clipped_bb[1].x, y: clipped_bb[1].y },
                ],
                false,
            ))
        } else {
            None
        },
    };

    selection_rect.show(ui);
    get_cursor_icon(selection_rect, cursor_pos, bb)
}

fn get_cursor_icon(
    selection_rect: SelectionRect, cursor_pos: egui::Pos2, bb: [DVec2; 2],
) -> Option<SelectionResponse> {
    let rect = egui::Rect {
        min: egui::pos2(bb[0].x as f32, bb[0].y as f32),
        max: egui::pos2(bb[1].x as f32, bb[1].y as f32),
    };

    let mut res = SelectionResponse {
        current_op: SelectionOperation::HorizontalScale,
        cursor_icon: egui::CursorIcon::ResizeColumn,
    };

    if let Some(left_path) = &selection_rect.left {
        if pointer_interests_path(left_path, cursor_pos, None, SCALE_BRUSH_SIZE) {
            return Some(res);
        }
    };
    if let Some(right_path) = &selection_rect.right {
        if pointer_interests_path(right_path, cursor_pos, None, SCALE_BRUSH_SIZE) {
            return Some(res);
        }
    };

    res.cursor_icon = egui::CursorIcon::ResizeRow;
    res.current_op = SelectionOperation::VerticalScale;
    if let Some(top_path) = &selection_rect.top {
        if pointer_interests_path(top_path, cursor_pos, None, SCALE_BRUSH_SIZE) {
            return Some(res);
        }
    };
    if let Some(bottom_path) = &selection_rect.bottom {
        if pointer_interests_path(bottom_path, cursor_pos, None, SCALE_BRUSH_SIZE) {
            return Some(res);
        }
    };

    res.cursor_icon = egui::CursorIcon::Grab;
    res.current_op = SelectionOperation::Drag;
    if rect.contains(cursor_pos) {
        return Some(res);
    }
    None
}

fn drag(delta: egui::Pos2, de: &mut SelectedElement, buffer: &mut Buffer) {
    if let Some(node) = node_by_id(&mut buffer.current, de.id.clone()) {
        node.set_attr(
            "transform",
            format!(
                "matrix({},0,0,{},{},{} )",
                de.original_matrix.1[0],
                de.original_matrix.1[3],
                delta.x as f64 + de.original_matrix.clone().1[4],
                delta.y as f64 + de.original_matrix.clone().1[5]
            ),
        );
        buffer.needs_path_map_update = true;
    }
}

fn zoom_from_center(factor: f64, de: &mut SelectedElement, buffer: &mut Buffer) {
    let path = match buffer.paths.get_mut(&de.id) {
        None => return,
        Some(p) => p,
    };

    // the inverse of the master transform will get the location of the
    // path's in terms of the svg viewport instead of the default egui
    // viewport. those cords are used for center based scaling.
    if let Some(transform) = buffer.current.attr("transform") {
        let [a, b, c, d, e, f] = deserialize_transform(transform);
        path.apply_transform(
            DAffine2 {
                matrix2: DMat2 { x_axis: DVec2 { x: a, y: b }, y_axis: DVec2 { x: c, y: d } },
                translation: DVec2 { x: e, y: f },
            }
            .inverse(),
        );
    }

    let bb = path.bounding_box().unwrap();
    let element_rect = egui::Rect {
        min: egui::pos2(bb[0].x as f32, bb[0].y as f32),
        max: egui::pos2(bb[1].x as f32, bb[1].y as f32),
    };

    if let Some(node) = node_by_id(&mut buffer.current, de.id.clone()) {
        let mut scaled_matrix = de.original_matrix.1.clone();
        scaled_matrix = scaled_matrix.map(|n| n * factor);

        // after scaling the matrix, a corrective translate is applied
        // to ensure that it's scaled from the center
        scaled_matrix[4] -=
            (1. - factor) * (element_rect.width() / 2. - element_rect.right()) as f64;
        scaled_matrix[5] -=
            (1. - factor) * (element_rect.height() / 2. - element_rect.bottom()) as f64;

        node.set_attr("transform", serialize_transform(&scaled_matrix));
        buffer.needs_path_map_update = true;
    }
}

fn zoom_to_pos(
    pos: egui::Pos2, de: &mut SelectedElement, buffer: &mut Buffer, current_op: &SelectionOperation,
) {
    let factor = match current_op {
        SelectionOperation::HorizontalScale => pos.x / de.original_pos.x,
        SelectionOperation::VerticalScale => pos.y / de.original_pos.y,
        _ => return,
    } as f64;

    zoom_from_center(factor, de, buffer);
}
