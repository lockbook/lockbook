use std::collections::HashMap;

use egui::TouchPhase;
use resvg::usvg::Transform;
use tracing::trace;

use crate::tab::svg_editor::toolbar::MINI_MAP_WIDTH;

use super::element::BoundedElement;
use super::util::transform_rect;
use super::{SVGEditor, ViewportSettings};
use lb_rs::model::svg::buffer::u_transform_to_bezier;
use lb_rs::model::svg::element::Element;

use super::Buffer;
use super::toolbar::ToolContext;
pub const MIN_ZOOM_LEVEL: f32 = 0.1;

#[derive(Default)]
pub struct GestureHandler {
    current_gesture: Option<Gesture>,
    pub is_zoom_locked: bool,
    pub is_pan_x_locked: bool,
    pub is_pan_y_locked: bool,
}

#[derive(Clone, Debug)]
struct Gesture {
    /// min, max
    touch_infos: HashMap<u64, TouchInfo>,
}

#[derive(Clone, Copy, Debug, Default)]
struct TouchInfo {
    is_active: bool,
    lifetime_distance: f32,
    frame_delta: egui::Vec2,
    last_pos: Option<egui::Pos2>,
}

#[derive(PartialEq, Clone, Copy, Debug)]
enum Shortcut {
    Undo,
    Redo,
}

impl GestureHandler {
    pub fn handle_input(&mut self, ui: &mut egui::Ui, gesture_ctx: &mut ToolContext) {
        if !*gesture_ctx.allow_viewport_changes {
            self.current_gesture = None;
            return;
        }

        if let Some(current_gesture) = &mut self.current_gesture {
            for info in current_gesture.touch_infos.values_mut() {
                info.frame_delta = egui::vec2(0.0, 0.0);
            }
        }

        ui.input(|r| {
            // go through the touches and reset how much they moved in the last frame
            for e in r.events.iter() {
                self.handle_event(e, gesture_ctx)
            }
        });
        self.change_viewport(ui, gesture_ctx);
    }

    fn handle_event(&mut self, event: &egui::Event, gesture_ctx: &mut ToolContext) {
        if let egui::Event::Touch { device_id: _, id, phase, pos, force } = *event {
            if force.is_some() {
                return;
            }
            if let Some(current_gest) = &mut self.current_gesture {
                match phase {
                    TouchPhase::Start => {
                        // add this to the list of touches in the gesture
                        if current_gest
                            .touch_infos
                            .insert(
                                id.0,
                                TouchInfo {
                                    last_pos: Some(pos),
                                    is_active: true,
                                    ..Default::default()
                                },
                            )
                            .is_some()
                        {
                            trace!("duplicate start for a touch id")
                        }
                    }
                    TouchPhase::Move => {
                        if let Some(touch_info) = current_gest.touch_infos.get_mut(&id.0) {
                            if let Some(last_pos) = touch_info.last_pos {
                                touch_info.frame_delta = pos - last_pos;
                                touch_info.lifetime_distance += last_pos.distance(pos);
                            }
                            touch_info.last_pos = Some(pos);
                        }
                    }
                    TouchPhase::End => {
                        // mark this touch as inactive in touches hashmap
                        if let Some(touch_info) = current_gest.touch_infos.get_mut(&id.0) {
                            touch_info.is_active = false;
                        }

                        // if this is the last active touch then end the gesture
                        if current_gest
                            .touch_infos
                            .values()
                            .all(|info| !info.is_active)
                        {
                            self.apply_shortcut(gesture_ctx);
                            self.current_gesture = None;
                        }
                    }
                    TouchPhase::Cancel => {
                        // don't just mark as inactive insdies fof touches but completly remove it
                        if current_gest.touch_infos.remove(&id.0).is_none() {
                            trace!(?id, "tryed to cancel touch  which is not in current gesture",);
                        }
                    }
                }
            } else if phase == egui::TouchPhase::Start {
                let mut touch_infos = HashMap::new();
                touch_infos.insert(
                    id.0,
                    TouchInfo { last_pos: Some(pos), is_active: true, ..Default::default() },
                );

                self.current_gesture = Some(Gesture { touch_infos })
            } else if phase == egui::TouchPhase::Cancel {
                trace!(?id, "no gesture in progress but tried to cancel touch");
            }
        }
    }

    fn change_viewport(&mut self, ui: &mut egui::Ui, gesture_ctx: &mut ToolContext<'_>) {
        let zoom_delta = ui.input(|r| r.zoom_delta());
        let is_zooming = zoom_delta != 1.0;
        let pan = self.get_pan(ui, gesture_ctx);

        let touch_positions = SVGEditor::get_touch_positions(ui);
        let pos_cardinality = touch_positions.len();
        let mut sum_pos = egui::Pos2::default();
        for pos in SVGEditor::get_touch_positions(ui).values() {
            sum_pos.x += pos.x;
            sum_pos.y += pos.y;
        }

        let maybe_pos = if pos_cardinality != 0 {
            Some(sum_pos / pos_cardinality as f32)
        } else {
            ui.ctx().pointer_hover_pos()
        };

        let container_rect_with_mini_map = if gesture_ctx.settings.show_mini_map {
            egui::Rect::from_min_size(
                gesture_ctx.viewport_settings.container_rect.min,
                egui::vec2(
                    gesture_ctx.viewport_settings.container_rect.width() - MINI_MAP_WIDTH,
                    gesture_ctx.viewport_settings.container_rect.height(),
                ),
            )
        } else {
            gesture_ctx.viewport_settings.container_rect
        };

        if maybe_pos.is_some() && !container_rect_with_mini_map.contains(maybe_pos.unwrap()) {
            return;
        }

        let mut t = Transform::identity();
        if let Some(p) = pan {
            t = t.post_translate(
                if !self.is_pan_x_locked { p.x } else { 0.0 },
                if !self.is_pan_y_locked { p.y } else { 0.0 },
            );
        }
        if is_zooming && !self.is_zoom_locked {
            // apply zoom
            t = t.post_scale(zoom_delta, zoom_delta);

            // correct the zoom to center
            if let Some(pos) = maybe_pos {
                t = t.post_translate((1.0 - zoom_delta) * pos.x, (1.0 - zoom_delta) * pos.y);
            }
        }

        if pan.is_some() || is_zooming {
            transform_canvas(gesture_ctx.buffer, gesture_ctx.viewport_settings, t);
        }
    }

    fn apply_shortcut(&mut self, gesture_ctx: &mut ToolContext<'_>) {
        let current_gesture = match self.current_gesture {
            Some(ref mut val) => val,
            None => return,
        };

        if current_gesture
            .touch_infos
            .values()
            .any(|info| info.lifetime_distance != 0.0)
        {
            return;
        };
        let num_touches = current_gesture.touch_infos.len();
        let intended_shortcut = if num_touches == 2 {
            Shortcut::Undo
        } else if num_touches == 3 {
            Shortcut::Redo
        } else {
            trace!(num_touches, "no configured for num touches");
            return;
        };

        match intended_shortcut {
            Shortcut::Undo => gesture_ctx.history.undo(gesture_ctx.buffer),
            Shortcut::Redo => gesture_ctx.history.redo(gesture_ctx.buffer),
        };
        trace!(num_touches, "applied gesture");
    }

    fn get_pan(&self, ui: &mut egui::Ui, gesture_ctx: &mut ToolContext) -> Option<egui::Vec2> {
        if let Some(current_gesture) = &self.current_gesture {
            let mut active_touches = current_gesture.touch_infos.values().filter(|v| v.is_active);

            if active_touches.clone().count() == 1 && gesture_ctx.settings.pencil_only_drawing {
                let touch = active_touches.next().unwrap();
                return Some(touch.frame_delta);
            }
        }
        ui.input(|r| {
            if r.raw_scroll_delta.x.abs() > 0.0 || r.raw_scroll_delta.y.abs() > 0.0 {
                Some(r.raw_scroll_delta)
            } else if let Some(touch_gesture) = r.multi_touch() {
                if touch_gesture.translation_delta.x.abs() > 0.0
                    || touch_gesture.translation_delta.y.abs() > 0.0
                {
                    Some(touch_gesture.translation_delta)
                } else {
                    None
                }
            } else {
                None
            }
        })
    }

    // todo: tech debt lol, should refactor the eraser instead of doing this
    pub fn is_locked_vw_pen_only_draw(&self) -> bool {
        if let Some(current_gesture) = &self.current_gesture {
            current_gesture
                .touch_infos
                .values()
                .filter(|v| v.is_active)
                .count()
                > 0
        } else {
            false
        }
    }
}

pub fn transform_canvas(
    buffer: &mut Buffer, viewport_settings: &mut ViewportSettings, t: Transform,
) {
    let new_transform = viewport_settings.master_transform.post_concat(t);

    // max allowed zoom level is 10%
    if viewport_settings.master_transform.sx < MIN_ZOOM_LEVEL
        && new_transform.sx < viewport_settings.master_transform.sx
    {
        return;
    }
    if new_transform.sx == 0.0 || new_transform.sy == 0.0 {
        return;
    }
    viewport_settings.master_transform = new_transform;
    buffer.master_transform_changed = true;

    for el in buffer.elements.values_mut() {
        match el {
            Element::Path(path) => {
                path.diff_state.transformed = Some(t);
                path.data.apply_transform(u_transform_to_bezier(&t));
            }
            Element::Image(image) => {
                if let Some(new_vbox) = image.view_box.transform(t) {
                    image.view_box = new_vbox;
                }
                image.diff_state.transformed = Some(t);
            }
            Element::Text(_) => todo!(),
        }
    }
    viewport_settings.bounded_rect = viewport_settings
        .bounded_rect
        .map(|rect| transform_rect(rect, t));
}

/// returns the fit transform in the non master transform plane
pub fn get_zoom_fit_transform(viewport_settings: &ViewportSettings) -> Option<Transform> {
    let elements_bound = viewport_settings.bounded_rect?;

    get_rect_identity_transform(
        viewport_settings.container_rect,
        elements_bound,
        0.7,
        viewport_settings.container_rect.center(),
    )
}

/// given two rects how to transform them such that they're both equal
pub fn get_rect_identity_transform(
    origin: egui::Rect, source: egui::Rect, padding_coeff: f32, anchor: egui::Pos2,
) -> Option<Transform> {
    let is_width_smaller = source.width() < source.height();
    let zoom_delta = if is_width_smaller {
        origin.height() * padding_coeff / source.height()
    } else {
        origin.width() * padding_coeff / source.width()
    };
    let center_x = anchor.x - zoom_delta * (source.left() + source.width() / 2.0);
    let center_y = anchor.y - zoom_delta * (source.top() + source.height() / 2.0);
    Some(
        Transform::identity()
            .post_scale(zoom_delta, zoom_delta)
            .post_translate(center_x, center_y),
    )
}

/// result is in absolute plane
pub fn calc_elements_bounds(buffer: &Buffer) -> Option<egui::Rect> {
    let mut elements_bound =
        egui::Rect { min: egui::pos2(f32::MAX, f32::MAX), max: egui::pos2(f32::MIN, f32::MIN) };
    let mut dirty_bound = false;
    for (_, el) in buffer.elements.iter() {
        if el.deleted() {
            continue;
        }

        let el_rect = el.bounding_box();
        dirty_bound = true;

        elements_bound.min.x = elements_bound.min.x.min(el_rect.min.x);
        elements_bound.min.y = elements_bound.min.y.min(el_rect.min.y);

        elements_bound.max.x = elements_bound.max.x.max(el_rect.max.x);
        elements_bound.max.y = elements_bound.max.y.max(el_rect.max.y);
    }
    if !dirty_bound { None } else { Some(elements_bound) }
}

pub fn zoom_percentage_to_transform(
    zoom_percentage: f32, viewport_settings: &ViewportSettings, ui: &mut egui::Ui,
) -> Transform {
    let zoom_delta = (zoom_percentage) / (viewport_settings.master_transform.sx * 100.0);
    Transform::identity()
        .post_scale(zoom_delta, zoom_delta)
        .post_translate(
            (1.0 - zoom_delta) * ui.ctx().screen_rect().center().x,
            (1.0 - zoom_delta) * ui.ctx().screen_rect().center().y,
        )
}

impl SVGEditor {
    pub fn get_touch_positions(ui: &mut egui::Ui) -> HashMap<u64, egui::Pos2> {
        ui.input(|r| {
            let mut touch_positions = HashMap::new();
            for e in r.events.iter() {
                if let egui::Event::Touch { device_id: _, id, phase, pos, force: _ } = *e {
                    if phase != egui::TouchPhase::Cancel {
                        touch_positions.insert(id.0, pos);
                    }
                }
            }

            touch_positions
        })
    }
}
