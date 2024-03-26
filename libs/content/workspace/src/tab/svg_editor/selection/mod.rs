mod rect;
mod scale;
mod translate;

use bezier_rs::Subpath;

use self::{
    rect::SelectionRectContainer,
    scale::{scale_group_from_center, snap_scale},
    translate::{detect_translation, end_translation, save_translate, save_translates},
};

use super::{
    util::{bb_to_rect, deserialize_transform},
    Buffer, DeleteElement,
};

#[derive(Default)]
pub struct Selection {
    last_pos: Option<egui::Pos2>,
    selected_elements: Vec<SelectedElement>,
    candidate_selected_elements: Vec<SelectedElement>,
    selection_rect: Option<SelectionRectContainer>,
    laso_original_pos: Option<egui::Pos2>,
    laso_rect: Option<egui::Rect>,
    current_op: SelectionOperation,
}

#[derive(Clone)]
struct SelectedElement {
    id: String,
    original_pos: egui::Pos2,
    original_matrix: (String, [f64; 6]),
}

impl Selection {
    pub fn handle_input(
        &mut self, ui: &mut egui::Ui, working_rect: egui::Rect, buffer: &mut Buffer,
    ) {
        let pos = match ui.ctx().pointer_hover_pos() {
            Some(cp) => {
                if ui.is_enabled() {
                    cp
                } else {
                    egui::Pos2::ZERO
                }
            }
            None => egui::Pos2::ZERO,
        };

        let mut maybe_selected_el = None;

        if let Some(selection_rect) = &self.selection_rect {
            if selection_rect.show_delete_btn(buffer, ui, working_rect) {
                self.delete_selection(buffer);
                return;
            }
        }

        if matches!(self.current_op, SelectionOperation::Idle) {
            maybe_selected_el = detect_translation(buffer, self.last_pos, pos);
            if maybe_selected_el.is_some() {
                ui.output_mut(|r| r.cursor_icon = egui::CursorIcon::Grab);
            }
        }

        // build up selected elements
        if ui.input(|r| r.pointer.primary_clicked()) {
            // is cursor inside of a selected element?
            let pos_over_selected_el = if let Some(r) = &self.selection_rect {
                r.get_cursor_icon(pos).is_some()
            } else {
                false
            };

            // cursor is outside of a selected element, add elements
            if let Some(new_selected_el) = maybe_selected_el {
                if ui.input(|r| r.modifiers.shift) {
                    self.selected_elements.push(new_selected_el);
                    // end_translation(buffer, &mut self.selected_elements, pos);
                } else if !pos_over_selected_el {
                    self.selected_elements = vec![new_selected_el]
                }
                self.current_op = SelectionOperation::Translation;
            } else if !pos_over_selected_el {
                self.selected_elements.clear();
                self.laso_original_pos = Some(pos);
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

                if rect.area() > 10.0 {
                    self.candidate_selected_elements.clear();

                    self.laso_rect = Some(rect);
                    ui.painter().rect_filled(
                        rect,
                        egui::Rounding::none(),
                        ui.visuals().hyperlink_color.gamma_multiply(0.1),
                    );
                    // if the path bounding box intersects with the laso rect then it's a match
                    buffer.paths.iter().for_each(|(id, path)| {
                        let bb = path.bounding_box().unwrap();

                        let path_rect = bb_to_rect(bb);

                        if self.laso_rect.unwrap().intersects(path_rect) {
                            let bb_subpath = Subpath::new_rect(
                                glam::DVec2 {
                                    x: self.laso_rect.unwrap().min.x as f64,
                                    y: self.laso_rect.unwrap().min.y as f64,
                                },
                                glam::DVec2 {
                                    x: self.laso_rect.unwrap().max.x as f64,
                                    y: self.laso_rect.unwrap().max.y as f64,
                                },
                            );

                            if !path
                                .subpath_intersections(&bb_subpath, None, None)
                                .is_empty()
                                || self.laso_rect.unwrap().contains_rect(path_rect)
                            {
                                let transform = buffer
                                    .current
                                    .children()
                                    .find(|el| el.attr("id").unwrap_or_default().eq(id))
                                    .unwrap()
                                    .attr("transform")
                                    .unwrap_or_default();

                                self.candidate_selected_elements.push(SelectedElement {
                                    id: id.to_owned(),
                                    original_pos: pos,
                                    original_matrix: (
                                        transform.to_string(),
                                        deserialize_transform(transform),
                                    ),
                                });
                            }
                        }
                    });

                    self.selection_rect = SelectionRectContainer::new(
                        &self.candidate_selected_elements,
                        working_rect,
                        buffer,
                    );
                }
            } else if ui.input(|r| r.pointer.primary_released()) && self.laso_rect.is_some() {
                self.selected_elements = self.candidate_selected_elements.clone();
                self.laso_original_pos = None;
                self.laso_rect = None;
            }
        }

        if self.laso_rect.is_none() {
            self.selection_rect =
                SelectionRectContainer::new(&self.selected_elements, working_rect, buffer);
        }

        let mut intent = None;
        if let Some(r) = &self.selection_rect {
            r.show(ui);
            intent = r.get_cursor_icon(pos);
        }

        if ui.input(|r| r.pointer.primary_released()) {
            end_translation(buffer, &mut self.selected_elements, pos, true);
            self.current_op = SelectionOperation::Idle;
        } else if ui.input(|r| r.pointer.primary_clicked()) {
            end_translation(buffer, &mut self.selected_elements, pos, false);
        } else if ui.input(|r| r.pointer.primary_down()) {
            if matches!(self.current_op, SelectionOperation::Idle) {
                if let Some(r) = &mut intent {
                    self.current_op = r.current_op;
                    ui.output_mut(|w| w.cursor_icon = r.cursor_icon);
                }
            }

            match self.current_op {
                SelectionOperation::Translation => {
                    self.selected_elements.iter_mut().for_each(|el| {
                        let mut delta =
                            egui::pos2(pos.x - el.original_pos.x, pos.y - el.original_pos.y);
                        if let Some(transform) = buffer.current.attr("transform") {
                            let transform = deserialize_transform(transform);
                            delta.x /= transform[0] as f32;
                            delta.y /= transform[3] as f32;
                        }
                        save_translate(delta, el, buffer);
                        ui.output_mut(|w| w.cursor_icon = egui::CursorIcon::Grabbing);
                    });
                }
                SelectionOperation::EastScale
                | SelectionOperation::WestScale
                | SelectionOperation::NorthScale
                | SelectionOperation::SouthScale => {
                    if let Some(s_r) = self.selection_rect.as_ref() {
                        let icon = snap_scale(pos, &mut self.selected_elements, s_r, buffer);
                        if let Some(c) = icon {
                            ui.output_mut(|w| w.cursor_icon = c);
                        }
                    }
                }
                SelectionOperation::Idle => {}
            }
        } else if let Some(r) = intent {
            ui.output_mut(|w| w.cursor_icon = r.cursor_icon);
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
            save_translates(d, &mut self.selected_elements, buffer);
            end_translation(buffer, &mut self.selected_elements, pos, true);
        }

        let is_scaling_up = ui.input(|r| r.key_pressed(egui::Key::PlusEquals));
        let is_scaling_down = ui.input(|r| r.key_pressed(egui::Key::Minus));

        let factor = if is_scaling_up {
            1.1
        } else if is_scaling_down {
            0.9
        } else {
            1.0
        };

        if is_scaling_down || is_scaling_up {
            scale_group_from_center(
                factor,
                &mut self.selected_elements,
                self.selection_rect.as_ref().unwrap(),
                buffer,
            );
            end_translation(buffer, &mut self.selected_elements, pos, true);
        }

        if ui.input(|r| r.key_pressed(egui::Key::Backspace)) && !self.selected_elements.is_empty() {
            self.delete_selection(buffer);
        }

        self.last_pos = Some(pos);
    }

    fn delete_selection(&mut self, buffer: &mut Buffer) {
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
}

#[derive(Default, Clone, Copy)]
enum SelectionOperation {
    Translation,
    EastScale,
    WestScale,
    NorthScale,
    SouthScale,
    #[default]
    Idle,
}

struct SelectionResponse {
    current_op: SelectionOperation,
    cursor_icon: egui::CursorIcon,
}

impl SelectionResponse {
    fn new(current_op: SelectionOperation) -> Self {
        let cursor_icon = match current_op {
            SelectionOperation::Translation => egui::CursorIcon::Grab,
            SelectionOperation::EastScale => egui::CursorIcon::ResizeEast,
            SelectionOperation::WestScale => egui::CursorIcon::ResizeWest,
            SelectionOperation::NorthScale => egui::CursorIcon::ResizeNorth,
            SelectionOperation::SouthScale => egui::CursorIcon::ResizeSouth,
            SelectionOperation::Idle => egui::CursorIcon::Default,
        };

        Self { current_op, cursor_icon }
    }
}
