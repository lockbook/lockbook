use bezier_rs::Subpath;
use glam::{DAffine2, DMat2, DVec2};
use lb_rs::Uuid;
use resvg::usvg::Transform;

use super::{
    history::{History, TransformElement},
    parser::ManipulatorGroupId,
    toolbar::ToolContext,
    util::{is_multi_touch, pointer_intersects_element},
    Buffer, DeleteElement, Event,
};

#[derive(Default)]
pub struct Selection {
    pub selected_elements: Vec<SelectedElement>,
    current_op: SelectionOperation,
    laso_rect: Option<egui::Rect>,
    layout: Layout,
}

#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
enum SelectionOperation {
    Translation,
    EastScale,
    WestScale,
    NorthScale,
    SouthScale,
    LasoBuild(BuildPayload),
    #[default]
    Idle,
}

#[derive(Clone, Debug)]
pub struct SelectedElement {
    pub id: Uuid,
    transform: Transform, // collection of all transforms that happend during a drag
}

#[derive(Default)]
struct Layout {
    container_tooltip: Option<egui::Rect>,
}

enum SelectionEvent {
    StartLaso(BuildPayload),
    LasoBuild(BuildPayload),
    EndLaso,
    SelectAll,
    StartTransform(egui::Pos2),
    Transform(egui::Pos2),
    EndTransform(egui::Pos2),
    Delete,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct BuildPayload {
    pos: egui::Pos2,
    modifiers: egui::Modifiers,
}

struct SelectionInputState {
    transform_occured: bool,
    delta: egui::Vec2,
    is_multi_touch: bool,
}

impl Selection {
    pub fn handle_input(&mut self, ui: &mut egui::Ui, selection_ctx: &mut ToolContext) {
        let is_multi_touch = is_multi_touch(ui);

        ui.input(|r| {
            let mut input_state = SelectionInputState {
                transform_occured: false,
                delta: r.pointer.delta(),
                is_multi_touch,
            };
            for e in r.events.iter() {
                if input_state.is_multi_touch {
                    break;
                }
                if let Some(selection_event) = self.map_ui_event(e, selection_ctx) {
                    self.handle_selection_event(selection_event, selection_ctx, &mut input_state);
                }
            }
        });

        if let Some(laso_rect) = self.laso_rect {
            ui.painter().rect_filled(
                laso_rect,
                egui::Rounding::ZERO,
                ui.visuals().hyperlink_color.gamma_multiply(0.1),
            );
        };

        if self.current_op != SelectionOperation::Translation {
            self.show_selection_rects(ui, selection_ctx.buffer);
        }
    }

    fn map_ui_event(
        &self, event: &egui::Event, selection_ctx: &mut ToolContext,
    ) -> Option<SelectionEvent> {
        match self.current_op {
            SelectionOperation::Idle => {
                match *event {
                    egui::Event::PointerButton { pos, button, pressed, modifiers } => {
                        if button != egui::PointerButton::Primary {
                            return None;
                        }
                        if pressed {
                            // if the pos is inside of the current selection rect + feathering, then this is the start of a new drag
                            if self.decide_transform_type(pos, selection_ctx)
                                != SelectionOperation::Idle
                            {
                                println!("start drag");
                                return Some(SelectionEvent::StartTransform(pos));
                            } else {
                                println!("build selection");
                                return Some(SelectionEvent::StartLaso(BuildPayload {
                                    pos,
                                    modifiers,
                                }));
                            }
                        }
                    }
                    egui::Event::PointerMoved(pos) => {
                        // see if we're hovering over any paths, if so set the according cursor.
                        if let Some(maybe_selection) =
                            detect_translation(selection_ctx.buffer, None, pos)
                        {
                            println!("change cursor");
                            // cursor = egui::CursorIcon::Grab;
                            // hovered_selection = Some(maybe_selection);
                        }
                    }
                    egui::Event::Key { key, physical_key: _, pressed, repeat, modifiers } => {
                        if key == egui::Key::A && modifiers.command && pressed && !repeat {
                            return Some(SelectionEvent::SelectAll);
                        }

                        if key == egui::Key::Delete
                            || key == egui::Key::Backspace
                            || key == egui::Key::Backspace
                        {
                            return Some(SelectionEvent::Delete);
                        }
                    }
                    _ => {}
                }
            }
            SelectionOperation::LasoBuild(origin_payload) => {
                match *event {
                    egui::Event::PointerMoved(pos) => {
                        return Some(SelectionEvent::LasoBuild(BuildPayload {
                            pos,
                            modifiers: egui::Modifiers::NONE,
                        }))
                    }
                    egui::Event::PointerButton { pos, button, pressed, modifiers } => {
                        if button != egui::PointerButton::Primary {
                            println!("no primary button");
                            return None;
                        }
                        if !pressed {
                            // if the pos is inside of the current selection rect + feathering, then this is the start of a new drag
                            if self.selected_elements.is_empty() {
                                return Some(SelectionEvent::EndLaso);
                            } else {
                                return Some(SelectionEvent::EndLaso);
                                // return Some(SelectionEvent::StartTransform(pos));
                            }
                        }
                    }
                    _ => {}
                }
            }
            _ => {
                match *event {
                    egui::Event::PointerMoved(pos2) => {
                        // what edge is the pos in, set the cursor icon accordingly
                        // issue transform command based on the position delta
                        return Some(SelectionEvent::Transform(pos2));
                    }
                    egui::Event::PointerButton { pos, button, pressed, modifiers } => {
                        if button != egui::PointerButton::Primary {
                            return None;
                        }
                        if !pressed {
                            // end the transform / save to history
                            return Some(SelectionEvent::EndTransform(pos));
                        }
                    }
                    _ => {}
                }
            }
        }

        None
    }

    fn handle_selection_event(
        &mut self, selection_event: SelectionEvent, selection_ctx: &mut ToolContext,
        r: &mut SelectionInputState,
    ) {
        match selection_event {
            SelectionEvent::StartTransform(pos) => {
                self.current_op = self.decide_transform_type(pos, selection_ctx);
                println!("current transform type{:#?}", self.current_op);
            }
            SelectionEvent::Transform(pos) => {
                if r.transform_occured || r.is_multi_touch {
                    return;
                }
                let container_rect = self.get_container_rect(&selection_ctx.buffer);

                self.selected_elements.iter_mut().for_each(|s_el| {
                    // see what edge
                    let transform = match self.current_op {
                        SelectionOperation::Translation => {
                            Transform::identity().post_translate(r.delta.x, r.delta.y)
                        }
                        SelectionOperation::Idle => Transform::identity(),
                        _ => snap_scale(pos, container_rect, selection_ctx.buffer),
                    };

                    if let Some(el) = selection_ctx.buffer.elements.get_mut(&s_el.id) {
                        el.transform(transform);
                        s_el.transform = s_el.transform.post_concat(transform);
                    }
                });

                r.transform_occured = true;
            }
            SelectionEvent::EndTransform(pos2) => {
                println!("handling end transform");
                self.current_op = SelectionOperation::Idle;

                // save to history
                let events: Vec<TransformElement> = self
                    .selected_elements
                    .iter_mut()
                    .filter_map(|el| {
                        if el.transform.is_identity() {
                            return None;
                        }
                        if selection_ctx.buffer.elements.get_mut(&el.id).is_some() {
                            Some(TransformElement { id: el.id.to_owned(), transform: el.transform })
                        } else {
                            None
                        }
                    })
                    .collect();
                if !events.is_empty() {
                    selection_ctx.history.save(Event::Transform(events));
                }
            }
            SelectionEvent::StartLaso(build_payload) => {
                if let Some(maybe_new_selection) =
                    detect_translation(selection_ctx.buffer, None, build_payload.pos)
                {
                    if build_payload.modifiers.shift {
                        self.selected_elements.push(maybe_new_selection);
                    } else if build_payload.modifiers.alt {
                        if let Some(i) = self
                            .selected_elements
                            .iter()
                            .position(|s_el| s_el.id == maybe_new_selection.id)
                        {
                            self.selected_elements.remove(i);
                        }
                    } else {
                        self.selected_elements = vec![maybe_new_selection];
                    }
                    if !self.selected_elements.is_empty() {
                        self.current_op = SelectionOperation::Translation;
                    }
                } else {
                    self.current_op = SelectionOperation::LasoBuild(build_payload);
                    self.selected_elements.clear();
                }
                println!("selected count after build: {}", self.selected_elements.len());
            }
            SelectionEvent::LasoBuild(build_payload) => {
                if let SelectionOperation::LasoBuild(build_origin) = self.current_op {
                    let rect = get_laso_rect(build_payload.pos, build_origin.pos);
                    self.laso_rect = Some(rect);

                    self.selected_elements = self.get_laso_selected_els(selection_ctx);
                }
            }
            SelectionEvent::EndLaso => {
                self.current_op = SelectionOperation::Idle;
                self.laso_rect = None;
            }
            SelectionEvent::SelectAll => {
                println!("select all");
                self.selected_elements = selection_ctx
                    .buffer
                    .elements
                    .iter()
                    .filter_map(|(&id, el)| {
                        if el.deleted() {
                            return None;
                        }
                        Some(SelectedElement { id, transform: Transform::identity() })
                    })
                    .collect();
            }
            SelectionEvent::Delete => {
                let elements = self
                    .selected_elements
                    .iter()
                    .map(|selection| {
                        selection_ctx
                            .buffer
                            .elements
                            .iter()
                            .find(|(&id, _el)| id.eq(&selection.id));
                        DeleteElement { id: selection.id }
                    })
                    .collect();

                let delete_event = super::Event::Delete(elements);
                selection_ctx
                    .history
                    .apply_event(&delete_event, selection_ctx.buffer);
                selection_ctx.history.save(delete_event);
                self.selected_elements.clear();
            }
        }
    }

    fn get_laso_selected_els(
        &mut self, selection_ctx: &mut ToolContext<'_>,
    ) -> Vec<SelectedElement> {
        let mut laso_selected_elements = Vec::with_capacity(self.selected_elements.capacity());
        for (id, el) in selection_ctx.buffer.elements.iter() {
            if el.deleted() {
                continue;
            }
            if self.el_intersects_laso(el) {
                laso_selected_elements
                    .push(SelectedElement { id: *id, transform: Transform::identity() });
            }
        }
        laso_selected_elements
    }

    fn el_intersects_laso(&mut self, el: &super::parser::Element) -> bool {
        match el {
            super::parser::Element::Path(path) => {
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
            super::parser::Element::Image(img) => {
                let img_bb = img.bounding_box();
                self.laso_rect
                    .unwrap_or(egui::Rect::NOTHING)
                    .contains_rect(img_bb)
                    || self
                        .laso_rect
                        .unwrap_or(egui::Rect::NOTHING)
                        .intersects(img_bb)
            }
            super::parser::Element::Text(_) => todo!(),
        }
    }

    fn decide_transform_type(
        &self, cursor_pos: egui::Pos2, selection_ctx: &mut ToolContext,
    ) -> SelectionOperation {
        let rect = self.get_container_rect(&selection_ctx.buffer);

        let edge_stroke = egui::Stroke {
            width: 10.0,
            color: egui::Color32::DEBUG_COLOR, // we will never show the stroke, just use it to calc bounds
        };

        let left = egui::Shape::LineSegment {
            points: [rect.min, rect.min + egui::vec2(0.0, rect.height())],
            stroke: edge_stroke.into(),
        }
        .visual_bounding_rect();

        let right = egui::Shape::LineSegment {
            points: [rect.max, rect.max - egui::vec2(0.0, rect.height())],
            stroke: edge_stroke.into(),
        }
        .visual_bounding_rect();

        let top = egui::Shape::LineSegment {
            points: [rect.min, rect.min + egui::vec2(rect.width(), 0.0)],
            stroke: edge_stroke.into(),
        }
        .visual_bounding_rect();

        let bottom = egui::Shape::LineSegment {
            points: [rect.max, rect.max - egui::vec2(rect.width(), 0.0)],
            stroke: edge_stroke.into(),
        }
        .visual_bounding_rect();

        if left.contains(cursor_pos) {
            return SelectionOperation::WestScale;
        }
        if right.contains(cursor_pos) {
            return SelectionOperation::EastScale;
        }

        if top.contains(cursor_pos) {
            return SelectionOperation::NorthScale;
        }
        if bottom.contains(cursor_pos) {
            return SelectionOperation::SouthScale;
        }

        if rect.expand(10.0).contains(cursor_pos) {
            return SelectionOperation::Translation;
        }

        SelectionOperation::Idle
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

    pub fn show_selection_rects(&mut self, ui: &mut egui::Ui, buffer: &Buffer) {
        if self.selected_elements.is_empty() {
            return;
        }

        let container = self.get_container_rect(buffer);
        for el in self.selected_elements.iter() {
            let child = match buffer.elements.get(&el.id) {
                Some(el) => el.bounding_box(),
                None => continue,
            };
            if self.selected_elements.len() != 1 {
                self.show_child_selection_rect(ui, child);
            }
        }

        self.show_selection_container(ui, container);

        let mut ui = ui.child_ui(ui.clip_rect(), egui::Layout::default(), None);

        let gap_between_btn_and_rect = 15.0;

        let min = container.min
            - egui::vec2(
                0.0,
                self.layout
                    .container_tooltip
                    .unwrap_or(egui::Rect::ZERO)
                    .height()
                    + gap_between_btn_and_rect,
            );
        let tooltip_rect = egui::Rect { min, max: min };

        let res = ui.allocate_ui_at_rect(tooltip_rect, |ui| {
            egui::Frame::window(ui.style()).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.button("sup");
                    ui.add_space(10.0);
                    ui.label("fuuuu");
                })
            })
        });
        self.layout.container_tooltip = Some(res.response.rect);
    }

    pub fn get_container_rect(&self, buffer: &Buffer) -> egui::Rect {
        let mut container = egui::Rect::NOTHING;
        for el in self.selected_elements.iter() {
            let child = match buffer.elements.get(&el.id) {
                Some(el) => el.bounding_box(),
                None => continue,
            };

            container.min.x = container.min.x.min(child.min.x);
            container.min.y = container.min.y.min(child.min.y);

            container.max.x = container.max.x.max(child.max.x);
            container.max.y = container.max.y.max(child.max.y);
        }
        container
    }

    fn show_child_selection_rect(&self, ui: &mut egui::Ui, rect: egui::Rect) {
        ui.painter().rect_stroke(
            rect,
            egui::Rounding::ZERO,
            egui::Stroke { width: 1.0, color: ui.visuals().hyperlink_color.gamma_multiply(0.4) },
        );
    }

    fn show_selection_container(&self, ui: &mut egui::Ui, rect: egui::Rect) {
        for corner in [
            rect.min,
            rect.max,
            rect.min + egui::vec2(rect.width(), 0.0),
            rect.min + egui::vec2(0.0, rect.height()),
        ] {
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
                egui::Rounding::ZERO,
                egui::Color32::WHITE,
                egui::Stroke { width: 1.0, color: ui.visuals().hyperlink_color },
            );
        }

        ui.painter().rect_stroke(
            rect,
            egui::Rounding::ZERO,
            egui::Stroke { width: 1.0, color: ui.visuals().hyperlink_color },
        );
    }
}

fn get_laso_rect(current: egui::Pos2, drag_origin: egui::Pos2) -> egui::Rect {
    let mut corners = [drag_origin, current];
    corners.sort_by(|a, b| (a.x.total_cmp(&b.x)));
    let mut laso_rect = egui::Rect { min: corners[0], max: corners[1] };
    if laso_rect.height() < 0. {
        std::mem::swap(&mut laso_rect.min.y, &mut laso_rect.max.y)
    }
    if laso_rect.width() < 0. {
        std::mem::swap(&mut laso_rect.min.x, &mut laso_rect.max.x)
    }
    laso_rect
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

pub fn detect_translation(
    buffer: &mut Buffer, last_pos: Option<egui::Pos2>, current_pos: egui::Pos2,
) -> Option<SelectedElement> {
    for (id, el) in buffer.elements.iter() {
        if el.deleted() {
            continue;
        }
        if pointer_intersects_element(el, current_pos, last_pos, 10.0) {
            return Some(SelectedElement { id: *id, transform: Transform::identity() });
        }
    }
    None
}

pub fn scale_from_center(factor: f32, selected_rect: egui::Rect, buffer: &mut Buffer) -> Transform {
    let path: Subpath<ManipulatorGroupId> = Subpath::new_rect(
        DVec2 { x: selected_rect.min.x as f64, y: selected_rect.min.y as f64 },
        DVec2 { x: selected_rect.max.x as f64, y: selected_rect.max.y as f64 },
    );

    let bb = match path.bounding_box() {
        Some(val) => val,
        None => return Transform::identity(),
    };

    let element_rect = egui::Rect {
        min: egui::pos2(bb[0].x as f32, bb[0].y as f32),
        max: egui::pos2(bb[1].x as f32, bb[1].y as f32),
    };

    Transform::identity()
        .post_scale(factor, factor)
        .post_translate(
            -(1. - factor) * (element_rect.width() / 2. - element_rect.right()),
            -(1. - factor) * (element_rect.height() / 2. - element_rect.bottom()),
        )
}

pub fn snap_scale(pos: egui::Pos2, selected_rect: egui::Rect, buffer: &mut Buffer) -> Transform {
    let top_distance = pos.y - selected_rect.min.y;
    let bottom_distance = selected_rect.max.y - pos.y;
    let left_distance = pos.x - selected_rect.min.x;
    let right_distance = selected_rect.max.x - pos.x;

    let min_distance =
        f32::min(f32::min(top_distance, bottom_distance), f32::min(left_distance, right_distance));

    let factor = if min_distance == top_distance {
        (selected_rect.bottom() - pos.y) / selected_rect.height().abs()
    } else if min_distance == bottom_distance {
        (pos.y - selected_rect.top()) / selected_rect.height().abs()
    } else if min_distance == right_distance {
        (pos.x - selected_rect.left()) / selected_rect.width().abs()
    } else {
        (selected_rect.right() - pos.x) / selected_rect.width().abs()
    };

    scale_from_center(factor, selected_rect, buffer)
}
