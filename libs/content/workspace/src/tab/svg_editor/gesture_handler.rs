use std::time::{Duration, Instant};

use resvg::usvg::Transform;
use tracing::{field::debug, trace};

use crate::tab::svg_editor::util::is_multi_touch;

use super::{toolbar::ToolContext, util::get_touch_positions, Buffer};

#[derive(Default)]
pub struct GestureHandler {
    current_gesture: Option<Gesture>,
}

#[derive(Clone, Copy, Debug)]
struct Gesture {
    /// min, max
    potential_zoom: f32,
    potential_pan: egui::Vec2,
    last_applied_shortcut: Option<(Shortcut, Instant)>,
    total_applied_shortcuts: usize,
    num_touches: usize,
    start_time: Instant, // can't trust egui's time that's `Relative to whatever. Used for animation.`
}

impl Gesture {
    fn new(ui: &mut egui::Ui) -> Self {
        let zoom_delta = ui.input(|r| r.zoom_delta());
        let pan = get_pan(ui).unwrap_or_default();
        let res = Gesture {
            potential_zoom: zoom_delta,
            potential_pan: pan,
            start_time: Instant::now(),
            last_applied_shortcut: None,
            total_applied_shortcuts: 0,
            num_touches: if let Some(multi_touch) = ui.input(|r| r.multi_touch()) {
                multi_touch.num_touches
            } else {
                get_touch_positions(ui).len()
            },
        };
        trace!(res.num_touches, "initialed num touch");
        res
    }
}
#[derive(PartialEq, Clone, Copy, Debug)]
enum Shortcut {
    Undo,
    Redo,
}
const ZOOM_THRESH: f32 = 0.05;
const PAN_THRESH: egui::Vec2 = egui::vec2(10.0, 10.0);
const DECISION_TIME: Duration = Duration::from_millis(800);

impl GestureHandler {
    /// returns true if the viewport is changing and there was a zoom or pan event
    pub fn handle_input(&mut self, ui: &mut egui::Ui, gesture_ctx: &mut ToolContext) {
        if !*gesture_ctx.allow_viewport_changes && self.current_gesture.is_none() {
            return;
        }

        // populate gesture on first multi frame
        if is_multi_touch(ui) {
            if self.current_gesture.is_none() {
                trace!("setting new gesture");
                self.current_gesture = Some(Gesture::new(ui))
            }
        } else if let Some(prev_gesture) = &mut self.current_gesture {
            trace!("gesture released in the last frame");
            if !is_potential_viewport_change_higher_than_threshold(prev_gesture)
                && prev_gesture.total_applied_shortcuts == 0
            {
                trace!("starting shortcut handler");
                self.apply_shortcut(gesture_ctx);
                ui.ctx().request_repaint();
            }
            self.current_gesture = None;
        }

        if let Some(current_gesture) = self.current_gesture {
            if current_gesture.total_applied_shortcuts == 0 {
                self.change_viewport(ui, gesture_ctx);
            }
        } else {
            self.change_viewport(ui, gesture_ctx);
        }

        if let Some(multi_touch) = ui.input(|r| r.multi_touch()) {
            if let Some(current_touch) = &mut self.current_gesture {
                current_touch.num_touches = current_touch.num_touches.max(multi_touch.num_touches);
                trace!(current_touch.num_touches, "setting num touches");
            }
        }

        let current_gesture = match self.current_gesture {
            Some(ref mut val) => val,
            None => return,
        };

        if is_potential_viewport_change_higher_than_threshold(current_gesture) {
            return;
        }

        let elapsed_gesture_time = Instant::now() - current_gesture.start_time;
        if elapsed_gesture_time < DECISION_TIME {
            trace!("still collecting more potential viewport change");
            ui.ctx().request_repaint();
            return;
        }

        // // when was the last time the shortcut was applied.
        let should_apply_shortcut =
            if let Some(last_applied_shortcut) = &current_gesture.last_applied_shortcut {
                if current_gesture.total_applied_shortcuts == 0 {
                    trace!("this is first gesture, apply auto");
                    true
                } else {
                    let shortcut_cool_off = if current_gesture.total_applied_shortcuts > 4 {
                        Duration::from_millis(70)
                    } else {
                        Duration::from_millis(200)
                    };

                    let time_since_last_shortcut = Instant::now() - last_applied_shortcut.1;
                    trace!(?time_since_last_shortcut, "time frame between last shortcut");
                    time_since_last_shortcut > shortcut_cool_off
                }
            } else {
                trace!("there's no last gesture");
                true
            };

        if should_apply_shortcut {
            self.apply_shortcut(gesture_ctx);
        }
        ui.ctx().request_repaint();
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
            gesture_ctx.buffer.master_transform =
                gesture_ctx.buffer.master_transform.post_concat(t);

            for el in gesture_ctx.buffer.elements.values_mut() {
                el.transform(t);
            }
        }

        if let Some(current_gesture) = &mut self.current_gesture {
            current_gesture.potential_zoom *= (1.0 - zoom_delta).abs() + 1.0;
            current_gesture.potential_pan += pan.unwrap_or_default().abs();
        }
    }
    fn apply_shortcut(&mut self, gesture_ctx: &mut ToolContext<'_>) {
        let current_gesture = match self.current_gesture {
            Some(ref mut val) => val,
            None => return,
        };

        let intended_shortcut = if current_gesture.num_touches == 2 {
            Shortcut::Undo
        } else if current_gesture.num_touches == 3 {
            Shortcut::Redo
        } else {
            trace!(current_gesture.num_touches, "no configured for num touches");
            return;
        };

        if let Some(last_applied_shortcut) = &current_gesture.last_applied_shortcut {
            if last_applied_shortcut.0 == intended_shortcut {
                current_gesture.total_applied_shortcuts += 1;
            } else {
                current_gesture.total_applied_shortcuts = 1;
            }
        } else {
            current_gesture.total_applied_shortcuts += 1
        }
        match intended_shortcut {
            Shortcut::Undo => gesture_ctx.history.undo(gesture_ctx.buffer),
            Shortcut::Redo => gesture_ctx.history.redo(gesture_ctx.buffer),
        };
        current_gesture.last_applied_shortcut = Some((intended_shortcut, Instant::now()));
        trace!(current_gesture.num_touches, "applied gesture");
    }
}

fn is_potential_viewport_change_higher_than_threshold(current_gesture: &Gesture) -> bool {
    current_gesture.potential_pan.x.abs() > PAN_THRESH.x
        || current_gesture.potential_pan.y.abs() > PAN_THRESH.y
        || (current_gesture.potential_zoom - 1.0).abs() > ZOOM_THRESH
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
