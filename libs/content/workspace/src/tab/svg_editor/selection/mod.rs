mod rect;
mod scale;
mod translate;

use bezier_rs::Subpath;
use glam::{DAffine2, DMat2, DVec2};
use resvg::usvg::Transform;

use crate::tab::svg_editor::selection::scale::snap_scale;

use self::{
    rect::SelectionRectContainer,
    scale::scale_group_from_center,
    translate::{detect_translation, end_translation},
};

use super::{history::History, parser, util::bb_to_rect, Buffer, DeleteElement};

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

#[derive(Clone, Debug)]
struct SelectedElement {
    id: String,
    prev_pos: egui::Pos2,
    transform: Transform,
}

impl Selection {
    pub fn handle_input(
        &mut self, ui: &mut egui::Ui, painter: &egui::Painter, buffer: &mut parser::Buffer,
        history: &mut History,
    ) {
        let pos = ui.ctx().pointer_hover_pos().or(ui.input(|r| {
            r.events.iter().find_map(|event| {
                if let egui::Event::Touch { device_id: _, id: _, phase: _, pos, force: _ } = event {
                    Some(*pos)
                } else {
                    None
                }
            })
        }));

        let working_rect = painter.clip_rect();
        let mut maybe_selected_el = None;

        if let Some(selection_rect) = &self.selection_rect {
            if selection_rect.show_delete_btn(ui, painter) {
                self.delete_selection(buffer, history);
                self.laso_original_pos = None;
                return;
            }
        }

        if matches!(self.current_op, SelectionOperation::Idle) && pos.is_some() {
            maybe_selected_el = detect_translation(buffer, self.last_pos, pos.unwrap());
            if maybe_selected_el.is_some() {
                ui.output_mut(|r| r.cursor_icon = egui::CursorIcon::Grab);
            }
        }

        // build up selected elements
        let should_rebuild = if cfg!(target_os = "ios") {
            pos.is_some()
                && ui.input(|i| {
                    i.events.iter().any(|e| {
                        if let egui::Event::Touch { device_id: _, id: _, phase, pos: _, force: _ } =
                            e
                        {
                            !phase.eq(&egui::TouchPhase::Move)
                        } else {
                            false
                        }
                    })
                })
        } else {
            ui.input(|r| r.pointer.primary_clicked())
        };

        if should_rebuild {
            // is cursor inside of a selected element?
            let pos_over_selected_el = if let Some(r) = &self.selection_rect {
                r.get_cursor_icon(pos.unwrap()).is_some()
            } else {
                false
            };

            // cursor is outside of a selected element, add elements
            if let Some(new_selected_el) = maybe_selected_el {
                if ui.input(|r| r.modifiers.shift) {
                    self.selected_elements.push(new_selected_el);
                } else if !pos_over_selected_el {
                    self.selected_elements = vec![new_selected_el]
                }
            } else if !pos_over_selected_el {
                self.selected_elements.clear();
                self.laso_original_pos = Some(pos.unwrap());
            }
        }

        if self.selected_elements.is_empty() {
            if ui.input(|r| r.pointer.primary_down())
                && pos.is_some()
                && self.laso_original_pos.is_some()
            {
                let pos = pos.unwrap();
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
                    painter.rect_filled(
                        rect,
                        egui::Rounding::ZERO,
                        ui.visuals().hyperlink_color.gamma_multiply(0.1),
                    );
                    // if the path bounding box intersects with the laso rect then it's a match
                    for (id, el) in buffer.elements.iter() {
                        if el.deleted() {
                            continue;
                        }
                        let el_intersects_laso = match el {
                            parser::Element::Path(path) => {
                                let bb = path.data.bounding_box().unwrap();
                                let path_rect = bb_to_rect(bb);
                                if self.laso_rect.unwrap().intersects(path_rect) {
                                    let laso_bb = Subpath::new_rect(
                                        glam::DVec2 {
                                            x: self.laso_rect.unwrap().min.x as f64,
                                            y: self.laso_rect.unwrap().min.y as f64,
                                        },
                                        glam::DVec2 {
                                            x: self.laso_rect.unwrap().max.x as f64,
                                            y: self.laso_rect.unwrap().max.y as f64,
                                        },
                                    );

                                    !path
                                        .data
                                        .subpath_intersections(&laso_bb, None, None)
                                        .is_empty()
                                        || self.laso_rect.unwrap().contains_rect(path_rect)
                                } else {
                                    false
                                }
                            }
                            parser::Element::Image(img) => {
                                let img_bb = img.bounding_box();
                                self.laso_rect.unwrap().contains_rect(img_bb)
                                    || self.laso_rect.unwrap().intersects(img_bb)
                            }
                            parser::Element::Text(_) => todo!(),
                        };

                        if el_intersects_laso {
                            self.candidate_selected_elements.push(SelectedElement {
                                id: id.to_owned(),
                                prev_pos: pos,
                                transform: Transform::identity(),
                            });
                        }
                    }

                    self.selection_rect =
                        SelectionRectContainer::new(&self.candidate_selected_elements, buffer);
                }
            } else if ui.input(|r| r.pointer.primary_released()) && self.laso_rect.is_some() {
                self.selected_elements = self.candidate_selected_elements.clone();
                self.laso_original_pos = None;
                self.laso_rect = None;
            }
        }

        if self.laso_rect.is_none() {
            self.selection_rect = SelectionRectContainer::new(&self.selected_elements, buffer);
        }

        let mut intent = None;
        if let Some(r) = &self.selection_rect {
            r.show(ui, painter);
            if let Some(p) = pos {
                if painter.clip_rect().contains(p) {
                    intent = r.get_cursor_icon(p);
                }
            }
        }

        if ui.input(|r| r.pointer.primary_released()) {
            if let Some(p) = pos {
                end_translation(buffer, history, &mut self.selected_elements, p, true);
            }
            self.current_op = SelectionOperation::Idle;
        } else if ui.input(|r| r.pointer.primary_clicked()) && pos.is_some() {
            end_translation(buffer, history, &mut self.selected_elements, pos.unwrap(), false);
        } else if ui.input(|r| r.pointer.primary_down()) {
            if matches!(self.current_op, SelectionOperation::Idle) {
                if let Some(r) = &mut intent {
                    self.current_op = r.current_op;
                    ui.output_mut(|w| w.cursor_icon = r.cursor_icon);
                }
            }
            if let Some(p) = pos {
                match self.current_op {
                    SelectionOperation::Translation => {
                        self.selected_elements.iter_mut().for_each(|selection| {
                            if let Some(el) = buffer.elements.get_mut(&selection.id) {
                                let transform = Transform::identity().post_translate(
                                    p.x - selection.prev_pos.x,
                                    p.y - selection.prev_pos.y,
                                );
                                selection.transform = selection.transform.post_concat(transform);
                                el.transform(transform);
                            }

                            selection.prev_pos = p;
                            ui.output_mut(|w| w.cursor_icon = egui::CursorIcon::Grabbing);
                        });
                    }
                    SelectionOperation::EastScale
                    | SelectionOperation::WestScale
                    | SelectionOperation::NorthScale
                    | SelectionOperation::SouthScale => {
                        if let Some(s_r) = self.selection_rect.as_ref() {
                            let icon = snap_scale(p, &mut self.selected_elements, s_r, buffer);
                            if let Some(c) = icon {
                                ui.output_mut(|w| w.cursor_icon = c);
                            }
                        }
                    }
                    SelectionOperation::Idle => {}
                }
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
            self.selected_elements
                .iter_mut()
                .for_each(|el| el.transform = Transform::identity().post_translate(d.x, d.y));
            if let Some(p) = pos {
                end_translation(buffer, history, &mut self.selected_elements, p, true);
            }
        }

        let is_scaling_up = ui.input(|r| r.key_pressed(egui::Key::Equals));
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
                self.selection_rect.as_ref().unwrap(), // todo: remove unwrap cus it can be none
                buffer,
            );
            if let Some(p) = pos {
                end_translation(buffer, history, &mut self.selected_elements, p, true);
            }
        }

        if ui.input(|r| r.key_pressed(egui::Key::Backspace)) && !self.selected_elements.is_empty() {
            self.delete_selection(buffer, history);
        }

        if let Some(p) = pos {
            self.last_pos = Some(p);
        }
    }

    fn delete_selection(&mut self, buffer: &mut Buffer, history: &mut History) {
        let elements = self
            .selected_elements
            .iter()
            .map(|selection| {
                buffer
                    .elements
                    .iter()
                    .find(|(id, _el)| id.to_owned().eq(&selection.id));
                DeleteElement { id: selection.id.clone() }
            })
            .collect();

        let delete_event = super::Event::Delete(elements);
        history.apply_event(&delete_event, buffer);
        history.save(delete_event);
        self.selected_elements.clear();
    }
}

#[derive(Default, Clone, Copy, Debug)]
enum SelectionOperation {
    Translation,
    EastScale,
    WestScale,
    NorthScale,
    SouthScale,
    #[default]
    Idle,
}

#[derive(Debug)]
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

/// converts a usvg transform into a bezier_rs transform
pub fn u_transform_to_bezier(src: &Transform) -> DAffine2 {
    glam::DAffine2 {
        matrix2: DMat2 {
            x_axis: DVec2 { x: src.sx.into(), y: src.ky.into() },
            y_axis: DVec2 { x: src.kx.into(), y: src.sy.into() },
        },
        translation: glam::DVec2 { x: src.tx.into(), y: src.ty.into() },
    }
}
