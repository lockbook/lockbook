use std::time::{Duration, Instant};

use resvg::usvg::Transform;
use tracing::trace;

use super::{toolbar::ToolContext, util::get_touch_positions, Buffer};

#[derive(Default)]
pub struct GestureHandler {
    current_gesture: Option<Gesture>,
}

struct Gesture {
    potential_zoom: f32,
    potential_pan: egui::Vec2,
    last_applied_shortcut: Option<(Shortcut, Instant)>,
    total_applied_shortcuts: usize,
    start_time: Instant, // can't trust egui's time that's `Relative to whatever. Used for animation.`
}

#[derive(PartialEq)]
enum Shortcut {
    Undo,
    Redo,
}
const ZOOM_THRESH: f32 = 0.2;
const PAN_THRESH: egui::Vec2 = egui::vec2(2.0, 2.0);
const DECISION_TIME: Duration = Duration::from_millis(200);

impl GestureHandler {
    /// returns true if the viewport is changing and there was a zoom or pan event
    pub fn handle_input(&mut self, ui: &mut egui::Ui, gesture_ctx: &mut ToolContext) {
        if !*gesture_ctx.allow_viewport_changes {
            return;
        }
        ui.ctx().request_repaint();
        // populate gesture on first multi frame
        if ui.input(|r| r.multi_touch()).is_some() {
            if self.current_gesture.is_none() {
                trace!("setting new gesture");
                self.current_gesture = Some(Gesture {
                    potential_zoom: 1.0,
                    potential_pan: egui::vec2(0.0, 0.0),
                    start_time: Instant::now(),
                    last_applied_shortcut: None,
                    total_applied_shortcuts: 0,
                })
            }
        } else {
            trace!("resetting current gesture");
            self.current_gesture = None;
        }

        let mut default_gesture = Gesture {
            potential_zoom: ui.input(|r| r.zoom_delta()),
            potential_pan: get_pan(ui).unwrap_or_default(),
            start_time: Instant::now(),
            last_applied_shortcut: None,
            total_applied_shortcuts: 0,
        };

        let current_gesture = match &mut self.current_gesture {
            Some(val) => val,
            None => &mut default_gesture,
        };

        let is_potential_viewport_change_higher_than_threshold =
            current_gesture.potential_pan.x.abs() > PAN_THRESH.x
                || current_gesture.potential_pan.y.abs() > PAN_THRESH.y
                || (current_gesture.potential_zoom - 1.0).abs() > ZOOM_THRESH;

        if is_potential_viewport_change_higher_than_threshold || !gesture_ctx.is_touch_frame {
            trace!("potential viewport change is higher than threshold");
            change_viewport(ui, gesture_ctx);
            return;
        }

        let current_multi_touch = match ui.input(|r| r.multi_touch()) {
            Some(val) => val,
            None => return,
        };

        let elapsed_gesture_time = Instant::now() - current_gesture.start_time;

        if elapsed_gesture_time < DECISION_TIME {
            trace!("still collecting more potential viewport change");
            current_gesture.potential_zoom *= current_multi_touch.zoom_delta;
            if let Some(current_pan) = get_pan(ui) {
                current_gesture.potential_pan += current_pan
            }
        } else {
            trace!("start the shortcut handler");

            let intended_shortcut = if current_multi_touch.num_touches == 2 {
                Shortcut::Undo
            } else if current_multi_touch.num_touches == 3 {
                Shortcut::Redo
            } else {
                return;
            };

            // when was the last time the shortcut was applied.
            let should_apply_shortcut =
                if let Some(last_applied_shortcut) = &current_gesture.last_applied_shortcut {
                    if current_gesture.total_applied_shortcuts == 0 {
                        true
                    } else {
                        let shortcut_cool_off = if current_gesture.total_applied_shortcuts > 8 {
                            Duration::from_millis(50)
                        } else {
                            Duration::from_millis(200)
                        };

                        Instant::now() - last_applied_shortcut.1 > shortcut_cool_off
                    }
                } else {
                    true
                };
            trace!(should_apply_shortcut, "should apply shortcut?");

            if should_apply_shortcut {
                if let Some(last_applied_shortcut) = &current_gesture.last_applied_shortcut {
                    if last_applied_shortcut.0 == intended_shortcut {
                        current_gesture.total_applied_shortcuts += 1;
                    } else {
                        current_gesture.total_applied_shortcuts = 1;
                    }
                }

                // decide what the shortcut should be. is it undo? redo?
                match intended_shortcut {
                    Shortcut::Undo => gesture_ctx.history.undo(gesture_ctx.buffer),
                    Shortcut::Redo => gesture_ctx.history.redo(gesture_ctx.buffer),
                };
                current_gesture.last_applied_shortcut = Some((intended_shortcut, Instant::now()));
            }
        }
    }
}

fn change_viewport(ui: &mut egui::Ui, gesture_ctx: &mut ToolContext<'_>) {
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
                    return; // todo: check this doesn't break zoom on touch devices
                }
            }
            None => egui::Pos2::ZERO,
        }
    };

    let mut t = Transform::identity();
    if let Some(p) = pan {
        t = t.post_translate(p.x, p.y);
    }
    if is_zooming {
        // apply zoom
        t = t.post_scale(zoom_delta, zoom_delta);

        // correct the zoom to center
        t = t.post_translate((1.0 - zoom_delta) * pos.x, (1.0 - zoom_delta) * pos.y);
    }

    if pan.is_some() || is_zooming {
        gesture_ctx.buffer.master_transform = gesture_ctx.buffer.master_transform.post_concat(t);

        for el in gesture_ctx.buffer.elements.values_mut() {
            el.transform(t);
        }
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
