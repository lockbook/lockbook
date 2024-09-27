mod rect;
mod scale;
mod translate;

use bezier_rs::Subpath;
use glam::{DAffine2, DMat2, DVec2};
use lb_rs::Uuid;
use resvg::usvg::Transform;

use crate::tab::svg_editor::selection::scale::snap_scale;

use self::{
    rect::SelectionRectContainer,
    scale::scale_group_from_center,
    translate::{detect_translation, end_translation},
};

use super::{
    history::History, parser, toolbar::ToolContext, util::is_multi_touch, Buffer, DeleteElement,
};

#[derive(Default)]
pub struct Selection {
    last_pos: Option<egui::Pos2>,
    pub selected_elements: Vec<SelectedElement>,
    candidate_selected_elements: Vec<SelectedElement>,
    selection_rect: Option<SelectionRectContainer>,
    laso_original_pos: Option<egui::Pos2>,
    laso_rect: Option<egui::Rect>,
    current_op: SelectionOperation,
}

#[derive(Clone, Debug)]
pub struct SelectedElement {
    pub id: Uuid,
    prev_pos: egui::Pos2,
    transform: Transform,
}

impl Selection {
    pub fn handle_input(&mut self, ui: &mut egui::Ui, selection_ctx: &mut ToolContext) {
        let pos = ui.ctx().pointer_hover_pos().or(ui.input(|r| {
            r.events.iter().find_map(|event| {
                if let egui::Event::Touch { device_id: _, id: _, phase: _, pos, force: _ } = event {
                    Some(*pos)
                } else {
                    None
                }
            })
        }));

        let working_rect = selection_ctx.painter.clip_rect();

        if is_multi_touch(ui) {
            *selection_ctx.allow_viewport_changes = true;

            self.selection_rect =
                SelectionRectContainer::new(&self.selected_elements, selection_ctx.buffer);
            if let Some(s) = &self.selection_rect {
                s.show(ui, selection_ctx.painter);
            }
            self.laso_original_pos = pos;
            return;
        }

        let mut maybe_selected_el = None;

        if let Some(selection_rect) = &self.selection_rect {
            if selection_rect.show_delete_btn(ui, selection_ctx.painter) {
                self.delete_selection(selection_ctx.buffer, selection_ctx.history);
                self.laso_original_pos = None;
                return;
            }
        }

        if matches!(self.current_op, SelectionOperation::Idle) && pos.is_some() {
            maybe_selected_el =
                detect_translation(selection_ctx.buffer, self.last_pos, pos.unwrap());

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

        let pos_is_inside_canvas =
            if let Some(p) = pos { selection_ctx.painter.clip_rect().contains(p) } else { false };

        if should_rebuild && pos_is_inside_canvas {
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
                    selection_ctx.painter.rect_filled(
                        rect,
                        egui::Rounding::ZERO,
                        ui.visuals().hyperlink_color.gamma_multiply(0.1),
                    );
                    // if the path bounding box intersects with the laso rect then it's a match
                    for (id, el) in selection_ctx.buffer.elements.iter() {
                        if el.deleted() {
                            continue;
                        }
                        let el_intersects_laso = match el {
                            parser::Element::Path(path) => {
                                let path_rect = path.bounding_box();
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
                                self.laso_rect
                                    .unwrap_or(egui::Rect::NOTHING)
                                    .contains_rect(img_bb)
                                    || self
                                        .laso_rect
                                        .unwrap_or(egui::Rect::NOTHING)
                                        .intersects(img_bb)
                            }
                            parser::Element::Text(_) => todo!(),
                        };

                        if el_intersects_laso {
                            self.candidate_selected_elements.push(SelectedElement {
                                id: *id,
                                prev_pos: pos,
                                transform: Transform::identity(),
                            });
                        }
                    }

                    self.selection_rect = SelectionRectContainer::new(
                        &self.candidate_selected_elements,
                        selection_ctx.buffer,
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
                SelectionRectContainer::new(&self.selected_elements, selection_ctx.buffer);
        }

        let mut intent = None;
        if let Some(r) = &self.selection_rect {
            r.show(ui, selection_ctx.painter);
            if let Some(p) = pos {
                if selection_ctx.painter.clip_rect().contains(p) {
                    intent = r.get_cursor_icon(p);
                }
            }
        }

        if ui.input(|r| r.pointer.primary_released()) {
            if let Some(p) = pos {
                end_translation(
                    selection_ctx.buffer,
                    selection_ctx.history,
                    &mut self.selected_elements,
                    p,
                    true,
                );
            }
            self.current_op = SelectionOperation::Idle;
        } else if ui.input(|r| r.pointer.primary_clicked()) && pos.is_some() {
            end_translation(
                selection_ctx.buffer,
                selection_ctx.history,
                &mut self.selected_elements,
                pos.unwrap(),
                false,
            );
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
                            if let Some(el) = selection_ctx.buffer.elements.get_mut(&selection.id) {
                                let transform = Transform::identity().post_translate(
                                    p.x - selection.prev_pos.x,
                                    p.y - selection.prev_pos.y,
                                );
                                selection.transform = selection.transform.post_concat(transform);
                                selection.transform.sx = 1.0;
                                selection.transform.sy = 1.0;

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
                            let icon = snap_scale(
                                p,
                                &mut self.selected_elements,
                                s_r,
                                selection_ctx.buffer,
                            );
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
                end_translation(
                    selection_ctx.buffer,
                    selection_ctx.history,
                    &mut self.selected_elements,
                    p,
                    true,
                );
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

        if is_scaling_down || is_scaling_up && self.selection_rect.is_some() {
            scale_group_from_center(
                factor,
                &mut self.selected_elements,
                self.selection_rect.as_ref().unwrap(),
                selection_ctx.buffer,
            );
            if let Some(p) = pos {
                end_translation(
                    selection_ctx.buffer,
                    selection_ctx.history,
                    &mut self.selected_elements,
                    p,
                    true,
                );
            }
        }

        if ui.input(|r| r.key_pressed(egui::Key::Backspace)) && !self.selected_elements.is_empty() {
            self.delete_selection(selection_ctx.buffer, selection_ctx.history);
        }
        if ui.input(|r| r.key_pressed(egui::Key::OpenBracket)) && !self.selected_elements.is_empty()
        {
            let index = if let Some((selected_el_index, _, el)) = selection_ctx
                .buffer
                .elements
                .get_full_mut(&self.selected_elements[0].id)
            {
                match el {
                    parser::Element::Path(path) => path.diff_state.data_changed = true,
                    parser::Element::Image(image) => image.diff_state.data_changed = true,
                    parser::Element::Text(_) => todo!(),
                }
                selected_el_index
            } else {
                0
            };
            selection_ctx.buffer.elements.swap_indices(0, index);
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
                    .find(|(&id, _el)| id.eq(&selection.id));
                DeleteElement { id: selection.id }
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
