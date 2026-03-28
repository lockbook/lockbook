use std::collections::HashMap;

use bezier_rs::Subpath;
use egui::UiBuilder;
use glam::DVec2;
use indexmap::IndexMap;
use lb_rs::Uuid;
use lb_rs::model::svg::buffer::{Buffer, get_pen_colors};
use lb_rs::model::svg::element::{Element, ManipulatorGroupId, Stroke, WeakImages};
use resvg::usvg::Transform;
use tracing::debug;

use lb_rs::model::svg::buffer::serialize_inner;

use crate::tab::svg_editor::clip::duplicate_elements;
use crate::tab::svg_editor::element::BoundedElement;
use crate::tab::svg_editor::history::{self, TransformElement};
use crate::tab::svg_editor::roger::RogerEvent;
use crate::tab::svg_editor::toolbar::{
    ToolContext, show_color_btn, show_opacity_slider, show_section_header, show_thickness_slider,
};
use crate::tab::svg_editor::tools::pen::DEFAULT_PEN_STROKE_WIDTH;
use crate::tab::svg_editor::tools::{RogerTool, selection};
use crate::tab::svg_editor::util::{pointer_intersects_element, transform_rect};
use crate::tab::svg_editor::{DeleteElement, Event};
use crate::theme::icons::Icon;
use crate::theme::palette::ThemePalette;
use crate::widgets::Button;

#[derive(Default)]
pub struct Selection {
    pub selected_elements: Vec<SelectedElement>,
    current_op: SelectionOperation,
    laso_rect: Option<egui::Rect>,
    pub layout: Layout,
    show_selection_popover: bool,
    pub selection_stroke_snashot: HashMap<Uuid, Stroke>,
    pub properties: Option<ElementEditableProperties>,
    selection_container: Option<egui::Rect>,
    selection_handles: Option<SelectionHandles>,
}

#[derive(Clone, Debug, Default)]
pub struct ElementEditableProperties {
    pub stroke: Option<Stroke>,
    pub opacity: f32,
}
struct SelectionHandles {
    handles: [(egui::Rect, SelectionOperation); 8],
}
impl Default for SelectionHandles {
    fn default() -> Self {
        Self {
            handles: [
                (egui::Rect::ZERO, SelectionOperation::NorthScale),
                (egui::Rect::ZERO, SelectionOperation::SouthScale),
                (egui::Rect::ZERO, SelectionOperation::EastScale),
                (egui::Rect::ZERO, SelectionOperation::WestScale),
                (egui::Rect::ZERO, SelectionOperation::NorthEastScale),
                (egui::Rect::ZERO, SelectionOperation::NorthWestScale),
                (egui::Rect::ZERO, SelectionOperation::SouthEastScale),
                (egui::Rect::ZERO, SelectionOperation::SouthWestScale),
            ],
        }
    }
}

impl SelectionHandles {
    fn from_corners(corners: [(egui::Pos2, SelectionOperation); 8]) -> Self {
        let mut selection = SelectionHandles::default();
        for (i, (pos, op)) in corners.iter().enumerate() {
            let handle_side_length = 8.0; // handle is a square
            let rect = egui::Rect {
                min: egui::pos2(pos.x - handle_side_length / 2.0, pos.y - handle_side_length / 2.0),
                max: egui::pos2(pos.x + handle_side_length / 2.0, pos.y + handle_side_length / 2.0),
            };
            selection.handles[i] = (rect, *op);
        }
        selection
    }

    fn show(&self, ui: &mut egui::Ui) {
        for (i, &(rect, scale_op)) in self.handles.iter().enumerate() {
            ui.painter().rect(
                rect,
                egui::CornerRadius::same(2),
                egui::Color32::WHITE,
                egui::Stroke { width: 1.0, color: ui.visuals().widgets.active.bg_fill },
                egui::epaint::StrokeKind::Inside,
            );
        }
    }
}

#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
enum SelectionOperation {
    Translation,
    EastScale,
    WestScale,
    NorthScale,
    SouthScale,
    NorthWestScale,
    NorthEastScale,
    SouthEastScale,
    SouthWestScale,
    LasoBuild(BuildPayload),
    #[default]
    Idle,
}

#[derive(Clone, Debug)]
pub struct SelectedElement {
    pub id: Uuid,
    pub transform: Transform, // collection of all transforms that happend during a drag
}

#[derive(Default)]
pub struct Layout {
    pub container_tooltip: Option<egui::Rect>,
    pub popover: Option<egui::Rect>,
}

#[derive(Debug)]
pub enum SelectionEvent {
    StartLaso(BuildPayload),
    LasoBuild(BuildPayload),
    EndLaso,
    SelectAll,
    StartTransform(SelectionOperation),
    Transform(egui::Pos2),
    EndTransform,
    Delete,
    Copy,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BuildPayload {
    pub pos: egui::Pos2,
    pub modifiers: egui::Modifiers,
}

impl RogerTool for Selection {
    type ToolEvent = SelectionEvent;

    fn roger_to_tool_event(&self, roger_event: RogerEvent) -> Option<Self::ToolEvent> {
        match roger_event {
            RogerEvent::ToolStart(payload) => {
                // we're hovering over an element
                let suggested_op = self.compute_suggested_op(payload.pos);
                debug!(?suggested_op, " tool start");

                if let Some(op) = suggested_op {
                    return Some(SelectionEvent::StartTransform(op));
                }

                Some(SelectionEvent::StartLaso(BuildPayload {
                    pos: payload.pos,
                    modifiers: egui::Modifiers::NONE, // todo: should add tool payload modifiers to roger event
                }))
            }
            RogerEvent::ToolRun(payload) => match self.current_op {
                SelectionOperation::LasoBuild(_) => Some(SelectionEvent::LasoBuild(BuildPayload {
                    pos: payload.pos,
                    modifiers: egui::Modifiers::NONE,
                })),
                _ => Some(SelectionEvent::Transform(payload.pos)),
            },
            RogerEvent::ToolEnd(_) => match self.current_op {
                SelectionOperation::LasoBuild(_) => Some(SelectionEvent::EndLaso),
                _ => Some(SelectionEvent::EndTransform),
            },
            _ => None,
        }
    }

    fn handle_tool_event(
        &mut self, ui: &mut egui::Ui, event: Self::ToolEvent, selection_ctx: &mut ToolContext,
    ) {
        match event {
            SelectionEvent::StartTransform(op) => {
                self.current_op = op;
            }
            SelectionEvent::Transform(pos) => {
                let container_rect = self.get_container_rect(selection_ctx.buffer);

                let min_allowed = egui::vec2(10.0, 10.0);

                for s_el in self.selected_elements.iter_mut() {
                    // see what edge
                    let transform = match self.current_op {
                        SelectionOperation::Translation => {
                            let pointer_delta = ui.input(|r| r.pointer.delta());
                            Transform::identity().post_translate(pointer_delta.x, pointer_delta.y)
                        }
                        SelectionOperation::Idle => Transform::identity(),
                        SelectionOperation::EastScale => {
                            let new_width =
                                container_rect.width() + (pos.x - container_rect.right());

                            let sx = new_width / container_rect.width();
                            let anchor = container_rect.min.x;

                            Transform::identity()
                                .post_scale(sx, 1.0)
                                .post_translate(anchor * (1. - sx), 0.0)
                        }
                        SelectionOperation::WestScale => {
                            let new_width =
                                container_rect.width() + (container_rect.left() - pos.x);

                            let sx = new_width / container_rect.width();
                            let anchor = container_rect.max.x;
                            Transform::identity()
                                .post_scale(sx, 1.0)
                                .post_translate(anchor * (1. - sx), 0.0)
                        }
                        SelectionOperation::NorthScale => {
                            let new_height =
                                container_rect.height() + (container_rect.top() - pos.y);

                            let sy = new_height / container_rect.height();
                            let anchor = container_rect.max.y;
                            Transform::identity()
                                .post_scale(1.0, sy)
                                .post_translate(0.0, anchor * (1. - sy))
                        }
                        SelectionOperation::SouthScale => {
                            let new_height =
                                container_rect.height() + (pos.y - container_rect.bottom());

                            let sy = new_height / container_rect.height();

                            let anchor = container_rect.min.y;
                            if new_height < 10.0 {
                                Transform::identity()
                            } else {
                                Transform::identity()
                                    .post_scale(1.0, sy)
                                    .post_translate(0.0, anchor * (1. - sy))
                            }
                        }
                        SelectionOperation::SouthWestScale => {
                            let new_height =
                                container_rect.height() + (pos.y - container_rect.bottom());
                            let new_width =
                                container_rect.width() + (container_rect.left() - pos.x);

                            let sy = new_height / container_rect.height();
                            let sx = new_width / container_rect.width();

                            let s_uniform = (sx + sy) / 2.0;

                            let anchor = container_rect.right_top();
                            Transform::identity()
                                .post_scale(s_uniform, s_uniform)
                                .post_translate(
                                    anchor.x * (1. - s_uniform),
                                    anchor.y * (1. - s_uniform),
                                )
                        }
                        SelectionOperation::NorthWestScale => {
                            let new_height =
                                container_rect.height() + (container_rect.top() - pos.y);
                            let new_width =
                                container_rect.width() + (container_rect.left() - pos.x);

                            let sy = new_height / container_rect.height();
                            let sx = new_width / container_rect.width();

                            let s_uniform = (sx + sy) / 2.0;

                            let anchor = container_rect.right_bottom();
                            Transform::identity()
                                .post_scale(s_uniform, s_uniform)
                                .post_translate(
                                    anchor.x * (1. - s_uniform),
                                    anchor.y * (1. - s_uniform),
                                )
                        }
                        SelectionOperation::NorthEastScale => {
                            let new_height =
                                container_rect.height() + (container_rect.top() - pos.y);
                            let new_width =
                                container_rect.width() + (pos.x - container_rect.right());

                            let sy = new_height / container_rect.height();
                            let sx = new_width / container_rect.width();

                            let s_uniform = (sx + sy) / 2.0;

                            let anchor = container_rect.left_bottom();
                            Transform::identity()
                                .post_scale(s_uniform, s_uniform)
                                .post_translate(
                                    anchor.x * (1. - s_uniform),
                                    anchor.y * (1. - s_uniform),
                                )
                        }
                        SelectionOperation::SouthEastScale => {
                            let new_height =
                                container_rect.height() + (pos.y - container_rect.bottom());
                            let new_width =
                                container_rect.width() + (pos.x - container_rect.right());

                            let sy = new_height / container_rect.height();
                            let sx = new_width / container_rect.width();

                            let s_uniform = (sx + sy) / 2.0;

                            let anchor = container_rect.left_top();
                            Transform::identity()
                                .post_scale(s_uniform, s_uniform)
                                .post_translate(
                                    anchor.x * (1. - s_uniform),
                                    anchor.y * (1. - s_uniform),
                                )
                        }
                        _ => snap_scale(pos, container_rect), // todod: figure out if this can be removed
                    };

                    let new_rect = transform_rect(container_rect, transform);
                    if new_rect.width() < min_allowed.x || new_rect.height() < min_allowed.y {
                        continue;
                    }

                    if let Some(el) = selection_ctx.buffer.elements.get_mut(&s_el.id) {
                        el.transform(transform);
                        s_el.transform = s_el.transform.post_concat(transform);
                    }
                }
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
                    selection_ctx
                        .history
                        .save(history::Event::Transform(events));
                }
            }
            SelectionEvent::StartLaso(build_payload) => {
                if build_payload.modifiers.alt {
                    let iter = self.selected_elements.iter().filter_map(|s_el| {
                        selection_ctx
                            .buffer
                            .elements
                            .get(&s_el.id)
                            .map(|el| (s_el.id, el.clone()))
                    });

                    let elements = IndexMap::from_iter(iter);

                    let new_ids = duplicate_elements(
                        elements,
                        selection_ctx.buffer.weak_path_pressures.clone(),
                        selection_ctx.buffer,
                        selection_ctx.history,
                        None,
                    );

                    let new_els = new_ids
                        .into_iter()
                        .map(|id| SelectedElement { id, transform: Transform::identity() })
                        .collect();

                    self.new_selection_els(new_els);
                    self.current_op = SelectionOperation::Translation;
                } else if let Some(maybe_new_selection) =
                    detect_translation(selection_ctx.buffer, None, build_payload.pos)
                {
                    if build_payload.modifiers.shift_only() {
                        self.push_selection_el(maybe_new_selection);
                    } else if build_payload.modifiers.command_only() {
                        if let Some(i) = self
                            .selected_elements
                            .iter()
                            .position(|s_el| s_el.id == maybe_new_selection.id)
                        {
                            self.remove_selection_el(i);
                        } else {
                            self.push_selection_el(maybe_new_selection);
                        }
                    } else {
                        self.new_selection_els(vec![maybe_new_selection]);
                    }
                    if !self.selected_elements.is_empty() {
                        self.current_op = SelectionOperation::Translation;
                    }
                } else {
                    self.current_op = SelectionOperation::LasoBuild(build_payload);
                    self.clear_selection_els();
                }
            }
            SelectionEvent::LasoBuild(build_payload) => {
                if let SelectionOperation::LasoBuild(build_origin) = self.current_op {
                    let rect = get_laso_rect(build_payload.pos, build_origin.pos);
                    self.laso_rect = Some(rect);

                    let new_selection_els = self.get_laso_selected_els(selection_ctx);
                    self.new_selection_els(new_selection_els);
                }
            }
            SelectionEvent::EndLaso => {
                self.current_op = SelectionOperation::Idle;
                self.laso_rect = None;
            }
            SelectionEvent::SelectAll => {
                let new_selection_els = selection_ctx
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

                self.new_selection_els(new_selection_els);
            }
            SelectionEvent::Delete => {
                self.delete_selection(selection_ctx);
            }
            SelectionEvent::Copy => self.copy_selection(ui, selection_ctx),
        }
    }

    fn show_hover_point(&self, _: &mut egui::Ui, _: egui::Pos2, _: &mut ToolContext<'_>) {}

    fn show_tool_ui(&mut self, ui: &mut egui::Ui, selection_ctx: &mut ToolContext) {
        ui.scope_builder(
            UiBuilder::new()
                .layer_id(egui::LayerId {
                    order: egui::Order::Foreground,
                    id: "selection_overlay".into(),
                })
                .max_rect(selection_ctx.viewport_settings.container_rect),
            |ui| {
                if let Some(laso_rect) = self.laso_rect {
                    ui.painter().rect_filled(
                        laso_rect,
                        0.0,
                        ui.visuals().widgets.active.bg_fill.linear_multiply(0.1),
                    );
                };

                self.show_selection_rects(ui, selection_ctx);
            },
        );
    }
}
impl Selection {
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

        let delete_event = Event::Delete(elements);
        selection_ctx
            .history
            .apply_event(&delete_event, selection_ctx.buffer);
        selection_ctx.history.save(delete_event);
        self.clear_selection_els();
    }

    fn compute_suggested_op(&self, pos: egui::Pos2) -> Option<SelectionOperation> {
        if let Some(handles) = &self.selection_handles {
            for (rect, op) in handles.handles.iter() {
                let rect = rect.expand(10.0);
                if rect.contains(pos) {
                    return Some(*op);
                }
            }
        }

        if let Some(container) = self.selection_container {
            let container = container.expand(5.0);
            if container.contains(pos) {
                return Some(SelectionOperation::Translation);
            }
        }

        None
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

    fn show_selection_rects(&mut self, ui: &mut egui::Ui, selection_ctx: &mut ToolContext) {
        if self.selected_elements.is_empty() {
            self.selection_container = None;
            self.selection_handles = None;
            return;
        }
        let container = self.get_container_rect(selection_ctx.buffer);

        if self.current_op != SelectionOperation::Translation
            && self.selection_stroke_snashot.is_empty()
        {
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

        ui.visuals_mut().window_corner_radius = egui::CornerRadius::same(7);
        ui.style_mut()
            .text_styles
            .insert(egui::TextStyle::Body, egui::FontId::new(15.0, egui::FontFamily::Proportional));
        ui.style_mut().text_styles.insert(
            egui::TextStyle::Button,
            egui::FontId::new(15.0, egui::FontFamily::Proportional),
        );
        ui.visuals_mut().window_shadow = egui::Shadow::NONE;

        if ui.visuals().dark_mode {
            ui.visuals_mut().window_stroke = egui::Stroke::NONE;
            ui.visuals_mut().window_fill = egui::Color32::from_rgb(42, 42, 42);
        }

        if let SelectionOperation::LasoBuild(_) = self.current_op {
            return;
        }

        let opacity = if self.current_op == SelectionOperation::Idle { 1.0 } else { 0.0 };

        ui.set_opacity(opacity);

        let gap_between_btn_and_rect = 15.0;

        // minimizes layout shifts
        let approx_container_tooltip =
            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(250.0, 0.0));

        let top_left_min = container.min
            - egui::vec2(
                0.0,
                self.layout
                    .container_tooltip
                    .unwrap_or(approx_container_tooltip)
                    .height()
                    + gap_between_btn_and_rect,
            );
        let bottom_left_min = container.left_bottom() + egui::vec2(0.0, gap_between_btn_and_rect);

        let bottom_screen_overflow = selection_ctx.viewport_settings.container_rect.bottom()
            < approx_container_tooltip.height() + bottom_left_min.y;

        let min = if bottom_screen_overflow { top_left_min } else { bottom_left_min };

        let tooltip_rect = egui::Rect { min, max: min };
        ui.scope_builder(
            UiBuilder::new().layer_id(egui::LayerId {
                order: egui::Order::Tooltip,
                id: "selection_tooltip".into(),
            }),
            |ui| {
                let res = ui.scope_builder(UiBuilder::new().max_rect(tooltip_rect), |ui| {
                    ui.style_mut().spacing.window_margin = egui::Margin::symmetric(5, 0);

                    egui::Frame::window(ui.style())
                        .show(ui, |ui| ui.horizontal(|ui| self.show_tooltip(ui, selection_ctx)))
                });
                let show_popover_toggled = res.inner.inner.inner;

                self.layout.container_tooltip = Some(res.response.rect);

                let popover_min = res.response.rect.right_top() + egui::vec2(5.0, 0.0);
                let rect = egui::Rect::from_min_size(popover_min, egui::vec2(0.0, 0.0));
                let res = ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
                    if self.show_selection_popover && !show_popover_toggled {
                        egui::Frame::window(ui.style()).show(ui, |ui| {
                            ui.vertical(|ui| {
                                self.show_selection_popover(ui, selection_ctx);
                            });
                        });
                    }
                });

                self.layout.popover = Some(res.response.rect)
            },
        );

        if opacity == 0.0 {
            self.layout.container_tooltip = None;
        }
    }

    fn show_tooltip(&mut self, ui: &mut egui::Ui, selection_ctx: &mut ToolContext) -> bool {
        let btn_margin = egui::vec2(5.0, 2.0);
        if ui.visuals().dark_mode {
            ui.visuals_mut().window_stroke =
                egui::Stroke { width: 1.5, color: egui::Color32::from_rgb(240, 240, 240) };
        } else {
            ui.visuals_mut().widgets.noninteractive.bg_stroke = ui.visuals().window_stroke;
            ui.visuals_mut().widgets.noninteractive.bg_stroke.width = 1.0;
        }

        let chevron_size = 13.0;

        if self.show_selection_popover {
            if Button::default()
                .icon(&Icon::CHEVRON_LEFT.size(chevron_size))
                .margin(egui::vec2(0.0, btn_margin.y))
                .show(ui)
                .clicked()
            {
                self.show_selection_popover = !self.show_selection_popover;
            }

            return false;
        }

        if Button::default()
            .text("Copy")
            .margin(btn_margin)
            .show(ui)
            .clicked()
        {
            self.copy_selection(ui, selection_ctx);
            self.clear_selection_els();
        }

        ui.separator();

        if Button::default()
            .text("Delete")
            .margin(btn_margin)
            .show(ui)
            .clicked()
        {
            self.delete_selection(selection_ctx);
        }

        ui.separator();

        if Button::default()
            .icon(&Icon::CHEVRON_RIGHT.size(chevron_size))
            .show(ui)
            .clicked()
        {
            self.show_selection_popover = !self.show_selection_popover;
            return true;
        }
        false
    }

    fn cut_selection(&mut self, ui: &mut egui::Ui, selection_ctx: &mut ToolContext<'_>) {
        self.copy_selection(ui, selection_ctx);
        self.delete_selection(selection_ctx);
        self.clear_selection_els();
    }

    fn copy_selection(&mut self, ui: &mut egui::Ui, selection_ctx: &mut ToolContext<'_>) {
        let id_map = &selection_ctx.buffer.id_map;
        let elements: IndexMap<Uuid, Element> = self
            .selected_elements
            .iter()
            .map(|el| (el.id, selection_ctx.buffer.elements.get(&el.id).unwrap().clone()))
            .collect();

        let serialized_selection = serialize_inner(
            id_map,
            &elements,
            &selection_ctx.buffer.weak_viewport_settings,
            &WeakImages::default(),
            &selection_ctx.buffer.weak_path_pressures,
        );

        ui.ctx().copy_text(serialized_selection);
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
            egui::CornerRadius::ZERO,
            egui::Stroke {
                width: 1.0,
                color: ui.visuals().widgets.active.bg_fill.linear_multiply(0.4),
            },
            egui::epaint::StrokeKind::Inside,
        );
    }

    fn show_selection_container(&mut self, ui: &mut egui::Ui, rect: egui::Rect) {
        let corners = [
            (rect.min, SelectionOperation::NorthWestScale),
            (rect.max, SelectionOperation::SouthEastScale),
            (rect.right_top(), SelectionOperation::NorthEastScale),
            (rect.left_bottom(), SelectionOperation::SouthWestScale),
            (rect.center_top(), SelectionOperation::NorthScale),
            (rect.center_bottom(), SelectionOperation::SouthScale),
            (rect.left_center(), SelectionOperation::WestScale),
            (rect.right_center(), SelectionOperation::EastScale),
        ];

        let selection_handles = SelectionHandles::from_corners(corners);
        selection_handles.show(ui);
        self.selection_handles = Some(selection_handles);

        ui.painter().rect_stroke(
            rect,
            egui::CornerRadius::ZERO,
            egui::Stroke { width: 1.0, color: ui.visuals().widgets.active.bg_fill },
            egui::epaint::StrokeKind::Inside,
        );
        self.selection_container = Some(rect);
    }

    fn push_selection_el(&mut self, el: SelectedElement) {
        self.selected_elements.push(el);
        self.properties = None;
    }

    fn remove_selection_el(&mut self, i: usize) {
        self.selected_elements.remove(i);
        self.properties = None;
    }

    fn clear_selection_els(&mut self) {
        self.selected_elements.clear();
        self.show_selection_popover = false;
        self.properties = None;
    }

    fn new_selection_els(&mut self, new_slection_els: Vec<SelectedElement>) {
        self.selected_elements = new_slection_els;
        self.properties = None;
    }

    fn show_selection_popover(
        &mut self, ui: &mut egui::Ui, selection_ctx: &mut ToolContext,
    ) -> bool {
        let width = 200.0;
        ui.style_mut().spacing.slider_width = width;
        ui.set_width(width);

        let mut buffer_changed = false;
        ui.add_space(10.0);
        show_section_header(ui, "stroke");
        ui.add_space(10.0);

        if let Some(properties) = &mut self.properties {
            self.selected_elements.iter().for_each(|s_el| {
                if let Some(el) = selection_ctx.buffer.elements.get(&s_el.id) {
                    properties.opacity = el.opacity();
                    properties.stroke = el.stroke();
                }
            });

            if let Some(stroke) = &mut properties.stroke {
                let colors = get_pen_colors();
                ui.horizontal_wrapped(|ui| {
                    colors.iter().for_each(|&c| {
                        let color = ThemePalette::resolve_dynamic_color(c, ui.visuals().dark_mode);
                        let active_color = ThemePalette::resolve_dynamic_color(
                            stroke.color,
                            ui.visuals().dark_mode,
                        );

                        let color_btn = show_color_btn(ui, color, active_color, None);
                        if color_btn.clicked() || color_btn.drag_started() {
                            let event = history::Event::StrokeChange(
                                self.selected_elements
                                    .iter()
                                    .filter_map(|s_el: &selection::SelectedElement| {
                                        if let Some(el) =
                                            selection_ctx.buffer.elements.get_mut(&s_el.id)
                                        {
                                            let old_stroke = el.stroke();
                                            stroke.color = c;
                                            if let Some(mut el_stroke) = el.stroke() {
                                                el_stroke.color = c;
                                                el.set_stroke(*stroke);
                                                buffer_changed = true;
                                                Some(history::StrokeChangeElement {
                                                    id: s_el.id,
                                                    old_stroke,
                                                    new_stroke: Some(el_stroke),
                                                })
                                            } else {
                                                None
                                            }
                                        } else {
                                            None
                                        }
                                    })
                                    .collect(),
                            );
                            selection_ctx.history.save(event);
                        }
                    });
                });
                ui.add_space(10.0);
                let slider_res = show_opacity_slider(ui, &mut stroke.opacity, &stroke.color);

                if slider_res.drag_started() || slider_res.clicked() {
                    // let's store the inital stroke of each selected el
                    self.selection_stroke_snashot = self
                        .selected_elements
                        .iter()
                        .filter_map(|s_el| {
                            if let Some(el) = selection_ctx.buffer.elements.get(&s_el.id) {
                                el.stroke().map(|stroke| (s_el.id, stroke))
                            } else {
                                None
                            }
                        })
                        .collect();
                }
                if slider_res.dragged() || slider_res.clicked() {
                    self.selected_elements.iter().for_each(|s_el| {
                        if let Some(el) = selection_ctx.buffer.elements.get_mut(&s_el.id) {
                            if let Some(mut el_stroke) = el.stroke() {
                                el_stroke.opacity = stroke.opacity;
                                el.set_stroke(el_stroke);
                                buffer_changed = true;
                            }
                        }
                    });
                }
                if slider_res.drag_stopped() || slider_res.clicked() {
                    let event = history::Event::StrokeChange(
                        self.selected_elements
                            .iter()
                            .filter_map(|s_el: &selection::SelectedElement| {
                                if let Some(el) = selection_ctx.buffer.elements.get_mut(&s_el.id) {
                                    Some(history::StrokeChangeElement {
                                        id: s_el.id,
                                        old_stroke: self
                                            .selection_stroke_snashot
                                            .get(&s_el.id)
                                            .copied(),
                                        new_stroke: el.stroke(),
                                    })
                                } else {
                                    None
                                }
                            })
                            .collect(),
                    );
                    self.selection_stroke_snashot.clear();
                    selection_ctx.history.save(event);
                }

                ui.add_space(25.0);

                let range = DEFAULT_PEN_STROKE_WIDTH..=10.0;
                let slider_res = show_thickness_slider(ui, &mut stroke.width, range, 0.0);

                if slider_res.drag_started() || slider_res.clicked() {
                    // let's store the inital stroke of each selected el
                    self.selection_stroke_snashot = self
                        .selected_elements
                        .iter()
                        .filter_map(|s_el| {
                            if let Some(el) = selection_ctx.buffer.elements.get(&s_el.id) {
                                el.stroke().map(|stroke| (s_el.id, stroke))
                            } else {
                                None
                            }
                        })
                        .collect();
                }
                if slider_res.dragged() || slider_res.clicked() {
                    self.selected_elements.iter().for_each(|s_el| {
                        if let Some(el) = selection_ctx.buffer.elements.get_mut(&s_el.id) {
                            if let Some(mut el_stroke) = el.stroke() {
                                el_stroke.width = stroke.width;
                                el.set_stroke(el_stroke);
                                buffer_changed = true;
                            }
                        }
                    });
                }
                if slider_res.drag_stopped() || slider_res.clicked() {
                    let event = history::Event::StrokeChange(
                        self.selected_elements
                            .iter()
                            .filter_map(|s_el: &selection::SelectedElement| {
                                if let Some(el) = selection_ctx.buffer.elements.get_mut(&s_el.id) {
                                    Some(history::StrokeChangeElement {
                                        id: s_el.id,
                                        old_stroke: self
                                            .selection_stroke_snashot
                                            .get(&s_el.id)
                                            .copied(),
                                        new_stroke: el.stroke(),
                                    })
                                } else {
                                    None
                                }
                            })
                            .collect(),
                    );
                    self.selection_stroke_snashot.clear();
                    selection_ctx.history.save(event);
                }

                ui.add_space(20.0);

                show_section_header(ui, "layer");
                ui.add_space(5.0);
                ui.horizontal(|ui| {
                    self.show_layer_controls(selection_ctx, ui);
                });

                ui.add_space(7.5);
                ui.horizontal(|ui| {
                    self.show_action_controls(selection_ctx, ui);
                });
                ui.add_space(10.0);
            }
        } else {
            let mut properties = ElementEditableProperties::default();

            self.selected_elements.iter().for_each(|s_el| {
                if let Some(el) = selection_ctx.buffer.elements.get(&s_el.id) {
                    properties.opacity = el.opacity();
                    properties.stroke = el.stroke();
                }
            });

            self.properties = Some(properties);
        }

        buffer_changed
    }

    fn show_layer_controls(&mut self, selection_ctx: &mut ToolContext<'_>, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let btn_rounding = 5.0;
            let icon_size = 14.0;

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
                .icon(&Icon::BRING_TO_BACK.size(icon_size).color(
                    if max_current_index == selection_ctx.buffer.elements.len() - 1 {
                        ui.visuals().text_color().linear_multiply(0.4)
                    } else {
                        ui.visuals().text_color()
                    },
                ))
                .frame(true)
                .margin(egui::vec2(3.0, 0.0))
                .rounding(btn_rounding)
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
                .icon(&Icon::CHEVRON_LEFT.size(icon_size).color(
                    if max_current_index == selection_ctx.buffer.elements.len() - 1 {
                        ui.visuals().text_color().linear_multiply(0.4)
                    } else {
                        ui.visuals().text_color()
                    },
                ))
                .frame(true)
                .margin(egui::vec2(3.0, 0.0))
                .rounding(btn_rounding)
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
                .icon(
                    &Icon::CHEVRON_RIGHT
                        .size(icon_size)
                        .color(if min_cureent_index == 0 {
                            ui.visuals().text_color().linear_multiply(0.4)
                        } else {
                            ui.visuals().text_color()
                        }),
                )
                .frame(true)
                .margin(egui::vec2(3.0, 0.0))
                .rounding(btn_rounding)
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
                .icon(
                    &Icon::BRING_TO_FRONT
                        .size(icon_size)
                        .color(if min_cureent_index == 0 {
                            ui.visuals().text_color().linear_multiply(0.4)
                        } else {
                            ui.visuals().text_color()
                        }),
                )
                .frame(true)
                .margin(egui::vec2(3.0, 0.0))
                .rounding(btn_rounding)
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
        });
    }

    fn show_action_controls(&mut self, selection_ctx: &mut ToolContext, ui: &mut egui::Ui) {
        let btn_rounding = 5.0;
        ui.horizontal(|ui| {
            if Button::default()
                .icon(&Icon::CONTENT_COPY)
                .frame(true)
                .margin(egui::vec2(3.0, 0.0))
                .rounding(btn_rounding)
                .show(ui)
                .clicked()
            {
                self.copy_selection(ui, selection_ctx);
                self.clear_selection_els();
            }
            if Button::default()
                .icon(&Icon::CONTENT_CUT)
                .frame(true)
                .margin(egui::vec2(3.0, 0.0))
                .rounding(btn_rounding)
                .show(ui)
                .clicked()
            {
                self.cut_selection(ui, selection_ctx);
            }
        });
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
