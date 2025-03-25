use bezier_rs::Subpath;
use egui_animation::{animate_bool_eased, easing};
use glam::DVec2;
use lb_rs::{
    model::svg::element::{Element, ManipulatorGroupId},
    Uuid,
};
use resvg::usvg::Transform;

use super::element::BoundedElement;

use crate::{theme::icons::Icon, widgets::Button};

use super::{
    history::TransformElement,
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
    EndTransform,
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
                if input_state.is_multi_touch
                    || (selection_ctx.settings.pencil_only_drawing
                        && selection_ctx.is_locked_vw_pen_only)
                {
                    break;
                }
                if let Some(pos) = r.pointer.interact_pos() {
                    if self
                        .layout
                        .container_tooltip
                        .unwrap_or(egui::Rect::NOTHING)
                        .contains(pos)
                    {
                        break;
                    }
                }

                if let Some(selection_event) = self.map_ui_event(e, selection_ctx) {
                    self.handle_selection_event(selection_event, selection_ctx, &mut input_state);
                }
            }
        });

        let mut ui = ui.child_ui(ui.clip_rect(), egui::Layout::default(), None);
        ui.with_layer_id(
            egui::LayerId { order: egui::Order::PanelResizeLine, id: "selection_overlay".into() },
            |ui| {
                if let Some(laso_rect) = self.laso_rect {
                    ui.painter().rect_filled(
                        laso_rect,
                        egui::Rounding::ZERO,
                        ui.visuals().hyperlink_color.linear_multiply(0.1),
                    );
                };

                self.show_selection_rects(ui, selection_ctx);
            },
        );
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
                                return Some(SelectionEvent::StartTransform(pos));
                            } else {
                                return Some(SelectionEvent::StartLaso(BuildPayload {
                                    pos,
                                    modifiers,
                                }));
                            }
                        }
                    }
                    egui::Event::Key { key, physical_key: _, pressed, repeat, modifiers } => {
                        if key == egui::Key::A && modifiers.command && pressed && !repeat {
                            return Some(SelectionEvent::SelectAll);
                        }

                        if key == egui::Key::Delete || key == egui::Key::Backspace {
                            return Some(SelectionEvent::Delete);
                        }
                    }
                    _ => {}
                }
            }
            SelectionOperation::LasoBuild(_) => {
                match *event {
                    egui::Event::PointerMoved(pos) => {
                        return Some(SelectionEvent::LasoBuild(BuildPayload {
                            pos,
                            modifiers: egui::Modifiers::NONE,
                        }))
                    }
                    egui::Event::PointerButton { pos: _, button, pressed, modifiers: _ } => {
                        if button != egui::PointerButton::Primary {
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
                    egui::Event::PointerButton { pos: _, button, pressed, modifiers: _ } => {
                        if button != egui::PointerButton::Primary {
                            return None;
                        }
                        if !pressed {
                            // end the transform / save to history
                            return Some(SelectionEvent::EndTransform);
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
            }
            SelectionEvent::Transform(pos) => {
                if r.transform_occured || r.is_multi_touch {
                    return;
                }
                let container_rect = self.get_container_rect(selection_ctx.buffer);

                self.selected_elements.iter_mut().for_each(|s_el| {
                    // see what edge
                    let transform = match self.current_op {
                        SelectionOperation::Translation => {
                            Transform::identity().post_translate(r.delta.x, r.delta.y)
                        }
                        SelectionOperation::Idle => Transform::identity(),
                        _ => snap_scale(pos, container_rect),
                    };

                    if let Some(el) = selection_ctx.buffer.elements.get_mut(&s_el.id) {
                        el.transform(transform);
                        s_el.transform = s_el.transform.post_concat(transform);
                    }
                });

                r.transform_occured = true;
            }
            SelectionEvent::EndTransform => {
                self.current_op = SelectionOperation::Idle;

                // save to history
                let events: Vec<TransformElement> = self
                    .selected_elements
                    .iter_mut()
                    .filter_map(|el| {
                        if el.transform.is_identity() {
                            return None;
                        }

                        let transform_elapsed = el.transform;
                        el.transform = Transform::identity();

                        if selection_ctx.buffer.elements.get_mut(&el.id).is_some() {
                            Some(TransformElement {
                                id: el.id.to_owned(),
                                transform: transform_elapsed,
                            })
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
                self.delete_selection(selection_ctx);
            }
        }
    }

    fn delete_selection(&mut self, selection_ctx: &mut ToolContext) {
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

    fn el_intersects_laso(&mut self, el: &Element) -> bool {
        let laso_rect = match self.laso_rect {
            Some(val) => val,
            None => return false,
        };
        match el {
            Element::Path(path) => {
                let path_rect = path.bounding_box();
                if laso_rect.intersects(path_rect) {
                    let laso_bb = Subpath::new_rect(
                        glam::DVec2 { x: laso_rect.min.x as f64, y: laso_rect.min.y as f64 },
                        glam::DVec2 { x: laso_rect.max.x as f64, y: laso_rect.max.y as f64 },
                    );

                    !path
                        .data
                        .subpath_intersections(&laso_bb, None, None)
                        .is_empty()
                        || laso_rect.contains_rect(path_rect)
                } else {
                    false
                }
            }
            Element::Image(img) => {
                let img_bb = img.bounding_box();
                laso_rect.contains_rect(img_bb) || laso_rect.intersects(img_bb)
            }
            Element::Text(_) => todo!(),
        }
    }

    fn decide_transform_type(
        &self, cursor_pos: egui::Pos2, selection_ctx: &mut ToolContext,
    ) -> SelectionOperation {
        let rect = self.get_container_rect(selection_ctx.buffer);

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

    pub fn show_selection_rects(&mut self, ui: &mut egui::Ui, selection_ctx: &mut ToolContext) {
        if self.selected_elements.is_empty() {
            return;
        }
        let container = self.get_container_rect(selection_ctx.buffer);
        if self.current_op != SelectionOperation::Translation {
            for el in self.selected_elements.iter() {
                let child = match selection_ctx.buffer.elements.get(&el.id) {
                    Some(el) => el.bounding_box(),
                    None => continue,
                };
                if self.selected_elements.len() != 1 {
                    self.show_child_selection_rect(ui, child);
                }
            }

            self.show_selection_container(ui, container);
        }

        ui.visuals_mut().window_rounding = egui::Rounding::same(10.0);
        ui.style_mut().spacing.window_margin = egui::Margin::symmetric(7.0, 3.0);
        ui.style_mut()
            .text_styles
            .insert(egui::TextStyle::Body, egui::FontId::new(13.0, egui::FontFamily::Proportional));
        ui.style_mut().text_styles.insert(
            egui::TextStyle::Button,
            egui::FontId::new(15.0, egui::FontFamily::Proportional),
        );
        ui.visuals_mut().window_shadow = egui::Shadow::NONE;

        if ui.visuals().dark_mode {
            ui.visuals_mut().window_stroke = egui::Stroke::NONE;
            ui.visuals_mut().window_fill = egui::Color32::from_rgba_unmultiplied(20, 20, 20, 247);
        } else {
            ui.visuals_mut().window_stroke =
                egui::Stroke::new(0.5, egui::Color32::from_rgb(240, 240, 240));
            ui.visuals_mut().window_shadow = egui::Shadow {
                offset: egui::vec2(1.0, 8.0),
                blur: 20.0,
                spread: 0.0,
                color: egui::Color32::from_black_alpha(5),
            };
            ui.visuals_mut().window_fill = ui.visuals().extreme_bg_color;
        }

        if let SelectionOperation::LasoBuild(_) = self.current_op {
            return;
        }

        let opacity = animate_bool_eased(
            ui.ctx(),
            "selection_tooltip",
            self.current_op == SelectionOperation::Idle,
            easing::cubic_out,
            0.2,
        );

        ui.set_opacity(opacity);

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
            egui::Frame::window(ui.style())
                .show(ui, |ui| ui.horizontal(|ui| self.show_tooltip(ui, selection_ctx)))
        });
        self.layout.container_tooltip = Some(res.response.rect);

        if opacity == 0.0 {
            self.layout.container_tooltip = None;
        }
    }

    fn show_tooltip(&mut self, ui: &mut egui::Ui, selection_ctx: &mut ToolContext) {
        let mut max_current_index = 0;
        let mut min_cureent_index = usize::MAX;
        self.selected_elements.iter().for_each(|selected_element| {
            if let Some((el_id, _, _)) =
                selection_ctx.buffer.elements.get_full(&selected_element.id)
            {
                max_current_index = el_id.max(max_current_index);
                min_cureent_index = el_id.min(min_cureent_index);
            }
        });

        if Button::default()
            .icon(&Icon::BRING_TO_BACK.color(
                if max_current_index == selection_ctx.buffer.elements.len() - 1 {
                    ui.visuals().text_color().linear_multiply(0.4)
                } else {
                    ui.visuals().text_color()
                },
            ))
            .show(ui)
            .clicked()
            && max_current_index != selection_ctx.buffer.elements.len() - 1
        {
            self.selected_elements.iter().for_each(|selected_element| {
                if let Some((el_id, _, _)) =
                    selection_ctx.buffer.elements.get_full(&selected_element.id)
                {
                    selection_ctx
                        .buffer
                        .elements
                        .move_index(el_id, selection_ctx.buffer.elements.len() - 1);
                }
            });
        }

        if Button::default()
            .icon(&Icon::BRING_BACK.color(
                if max_current_index == selection_ctx.buffer.elements.len() - 1 {
                    ui.visuals().text_color().linear_multiply(0.4)
                } else {
                    ui.visuals().text_color()
                },
            ))
            .show(ui)
            .clicked()
            && max_current_index != selection_ctx.buffer.elements.len() - 1
        {
            self.selected_elements.iter().for_each(|selected_element| {
                if let Some((el_id, _, _)) =
                    selection_ctx.buffer.elements.get_full(&selected_element.id)
                {
                    if el_id < selection_ctx.buffer.elements.len() - 1 {
                        selection_ctx.buffer.elements.swap_indices(el_id, el_id + 1);
                    }
                }
            });
        }

        if Button::default()
            .icon(&Icon::BRING_FRONT.color(if min_cureent_index == 0 {
                ui.visuals().text_color().linear_multiply(0.4)
            } else {
                ui.visuals().text_color()
            }))
            .show(ui)
            .clicked()
            && min_cureent_index != 0
        {
            self.selected_elements.iter().for_each(|selected_element| {
                if let Some((el_id, _, _)) =
                    selection_ctx.buffer.elements.get_full(&selected_element.id)
                {
                    if el_id > 0 {
                        selection_ctx.buffer.elements.swap_indices(el_id, el_id - 1);
                    }
                }
            });
        }

        if Button::default()
            .icon(&Icon::BRING_TO_FRONT.color(if min_cureent_index == 0 {
                ui.visuals().text_color().linear_multiply(0.4)
            } else {
                ui.visuals().text_color()
            }))
            .show(ui)
            .clicked()
            && min_cureent_index != 0
        {
            self.selected_elements.iter().for_each(|selected_element| {
                if let Some((el_id, _, _)) =
                    selection_ctx.buffer.elements.get_full(&selected_element.id)
                {
                    selection_ctx.buffer.elements.move_index(el_id, 0);
                }
            });
        }

        ui.visuals_mut().widgets.noninteractive.bg_stroke =
            egui::Stroke { width: 3.0, color: ui.visuals().extreme_bg_color };

        ui.add(egui::Separator::default().vertical().grow(3.0));

        ui.add_space(4.0);
        if ui
            .add(
                egui::Button::new(
                    egui::RichText::new("Delete")
                        .color(ui.visuals().error_fg_color.linear_multiply(0.8)),
                )
                .fill(egui::Color32::TRANSPARENT)
                .stroke(egui::Stroke::NONE),
            )
            .clicked()
        {
            self.delete_selection(selection_ctx);
        }
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
            egui::Stroke { width: 1.0, color: ui.visuals().hyperlink_color.linear_multiply(0.4) },
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
            let corner = egui::pos2(corner.x, corner.y);
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

pub fn scale_from_center(factor: f32, selected_rect: egui::Rect) -> Transform {
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

pub fn snap_scale(pos: egui::Pos2, selected_rect: egui::Rect) -> Transform {
    let top_distance = pos.y - selected_rect.min.y;
    let bottom_distance = selected_rect.max.y - pos.y;
    let left_distance = pos.x - selected_rect.min.x;
    let right_distance = selected_rect.max.x - pos.x;

    let min_distance =
        f32::min(f32::min(top_distance, bottom_distance), f32::min(left_distance, right_distance));

    let factor = if min_distance == top_distance {
        (selected_rect.bottom() - pos.y).abs() / selected_rect.height().abs()
    } else if min_distance == bottom_distance {
        (pos.y - selected_rect.top()).abs() / selected_rect.height().abs()
    } else if min_distance == right_distance {
        (pos.x - selected_rect.left()).abs() / selected_rect.width().abs()
    } else {
        (selected_rect.right() - pos.x).abs() / selected_rect.width().abs()
    };

    scale_from_center(factor, selected_rect)
}
