use std::collections::HashMap;

use egui::TouchPhase;
use resvg::usvg::Transform;
use tracing::trace;

use super::{toolbar::ToolContext, Buffer};

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
        ui.input(|r| {
            // go through the touches and reset how much they moved in the last frame
            for e in r.events.iter() {
                self.handle_event(e, gesture_ctx)
            }
        });
        self.change_viewport(ui, gesture_ctx);
    }

    fn handle_event(&mut self, event: &egui::Event, gesture_ctx: &mut ToolContext) {
        if let egui::Event::Touch { device_id: _, id, phase, pos, force: _ } = *event {
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
                touch_infos.insert(id.0, TouchInfo { last_pos: Some(pos), ..Default::default() });

                self.current_gesture = Some(Gesture { touch_infos })
            } else if phase == egui::TouchPhase::Cancel {
                trace!(?id, "no gesture in progress but tried to cancel touch");
            }
        }
    }

    fn change_viewport(&mut self, ui: &mut egui::Ui, gesture_ctx: &mut ToolContext<'_>) {
        let zoom_delta = ui.input(|r| r.zoom_delta());
        let is_zooming = zoom_delta != 1.0;
        let pan = get_pan(ui);

        let touch_positions = get_touch_positions(ui);
        let pos_cardinality = touch_positions.len();
        let mut sum_pos = egui::Pos2::default();
        for pos in get_touch_positions(ui).values() {
            sum_pos.x += pos.x;
            sum_pos.y += pos.y;
        }

        let pos = if pos_cardinality != 0 {
            sum_pos / pos_cardinality as f32
        } else {
            match ui.ctx().pointer_hover_pos() {
                Some(cp) => {
                    if gesture_ctx.painter.clip_rect().contains(cp) {
                        cp
                    } else {
                        return;
                    }
                }
                None => egui::Pos2::ZERO,
            }
        };

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
            t = t.post_translate((1.0 - zoom_delta) * pos.x, (1.0 - zoom_delta) * pos.y);
        }

        if pan.is_some() || is_zooming {
            transform_canvas(gesture_ctx.buffer, t);
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
            .any(|info| info.lifetime_distance > 50.0)
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
}

pub fn transform_canvas(buffer: &mut Buffer, t: Transform) {
    let new_transform = buffer.master_transform.post_concat(t);
    if new_transform.sx == 0.0 || new_transform.sy == 0.0 {
        return;
    }
    buffer.master_transform = new_transform;
    for el in buffer.elements.values_mut() {
        el.transform(t);
    }
}

pub fn get_zoom_fit_transform(buffer: &mut Buffer, ui: &mut egui::Ui) -> Option<Transform> {
    let elements_bound = match calc_elements_bounds(buffer) {
        Some(rect) => rect,
        None => return None,
    };
    let inner_rect = ui.painter().clip_rect();
    let is_width_smaller = elements_bound.width() < elements_bound.height();
    let padding_coeff = 0.7;
    let zoom_delta = if is_width_smaller {
        inner_rect.height() * padding_coeff / elements_bound.height()
    } else {
        inner_rect.width() * padding_coeff / elements_bound.width()
    };
    let center_x =
        inner_rect.center().x - zoom_delta * (elements_bound.left() + elements_bound.width() / 2.0);
    let center_y =
        inner_rect.center().y - zoom_delta * (elements_bound.top() + elements_bound.height() / 2.0);
    Some(
        Transform::identity()
            .post_scale(zoom_delta, zoom_delta)
            .post_translate(center_x, center_y),
    )
}

fn calc_elements_bounds(buffer: &mut Buffer) -> Option<egui::Rect> {
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
    if !dirty_bound {
        None
    } else {
        Some(elements_bound)
    }
}

fn get_pan(ui: &mut egui::Ui) -> Option<egui::Vec2> {
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

pub fn zoom_percentage_to_transform(
    zoom_percentage: f32, buffer: &mut Buffer, ui: &mut egui::Ui,
) -> Transform {
    let zoom_delta = (zoom_percentage) / (buffer.master_transform.sx * 100.0);
    return Transform::identity()
        .post_scale(zoom_delta, zoom_delta)
        .post_translate(
            (1.0 - zoom_delta) * ui.ctx().screen_rect().center().x,
            (1.0 - zoom_delta) * ui.ctx().screen_rect().center().y,
        );
}

fn get_touch_positions(ui: &mut egui::Ui) -> HashMap<u64, egui::Pos2> {
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
