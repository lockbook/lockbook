use core::f32;
use std::{collections::HashMap, slice::Iter};

use egui::{Pos2, TouchDeviceId, TouchId, TouchPhase};
use resvg::usvg::Transform;
use time::Duration;
use web_time::Instant;

use crate::{
    Event,
    tab::{
        ExtendedInput,
        svg_editor::{CanvasSettings, toolbar::ToolContext, tools::DynRogerTool},
    },
};

#[derive(Debug)]
pub struct Roger {
    touches: Vec<TouchInfo>,
    buttons: HashMap<MouseProps, (Instant, egui::Pos2)>, // track the start pos
    tool_running: Option<Instant>,
    mouse_hover_pos: Option<egui::Pos2>,
    tool_hover_pos: (egui::Pos2, Instant),
    tool_start_touch: Option<TouchId>, // keep track of the touch id that started a touch, to inform tool end
    viewport_changing: Option<Instant>,
    config: RogerConfig,
    is_touch_frame: bool, // as we traverse the input event stream do we see touch events.
    response: RogerResponse,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct RogerResponse {
    hide_overlay: bool,
}

#[derive(Debug, Default)]
pub struct RogerConfig {
    pencil_only_drawing: bool,
    is_read_only: bool,
}

impl RogerConfig {
    pub fn new(pencil_only_drawing: bool, is_read_only: bool) -> Self {
        Self { pencil_only_drawing, is_read_only }
    }
}

#[derive(Clone, Copy, Debug)]
struct TouchInfo {
    id: egui::TouchId,
    start: Instant,
    is_active: bool,
    has_force: bool,
    lifetime_distance: f32,
    frame_delta: egui::Vec2,
    last_pos: egui::Pos2,
}

impl TouchInfo {
    fn new(id: TouchId, pos: egui::Pos2, force: Option<f32>) -> Self {
        Self {
            id,
            start: Instant::now(),
            is_active: true,
            has_force: force.is_some(),
            lifetime_distance: 0.0,
            frame_delta: egui::Vec2::ZERO,
            last_pos: pos,
        }
    }
}

#[derive(Eq, Hash, PartialEq, Clone, Copy, Debug)]
struct MouseProps {
    button: ButtonType,
    modifiers: egui::Modifiers,
}

#[derive(PartialEq, Eq, Hash, Clone, Copy, Debug)]
enum ButtonType {
    Primary,
    Secondary,
    Middle,
    Extra1,
    Extra2,
}

#[derive(PartialEq, Debug, Clone, Copy)]
pub enum RogerEvent {
    ToolStart(ToolPayload),
    ToolRun(ToolPayload),
    ToolPredictedRun(egui::Pos2, Option<f32>),
    ToolEnd(ToolPayload),
    ToolCancel,
    ToolHover(ToolPayload),
    ViewportChange(Transform),
    Gesture(usize), // number of fingers in the gesture, ex: two finger undo
    ViewportChangeWithToolCancel,
}

struct ViewportChange;
impl ViewportChange {
    #[allow(clippy::new_ret_no_self)]
    fn new(roger: &Roger, event: &egui::Event, cancel_tool: bool) -> RogerEvent {
        if cancel_tool {
            return RogerEvent::ViewportChangeWithToolCancel;
        }
        let transform = match *event {
            egui::Event::Zoom(factor) => {
                let transform = Transform::identity().post_scale(factor, factor);

                let origin_pos = if let Some(pos) = roger.mouse_hover_pos {
                    pos
                } else {
                    let (sum, count) = roger
                        .touches
                        .iter()
                        .filter_map(|t| if t.is_active { Some(t.last_pos) } else { None })
                        .fold((Pos2::default(), 0), |(sum, count), pos| {
                            (egui::pos2(sum.x + pos.x, sum.y + pos.y), count + 1)
                        });
                    if count != 0 { sum / count as f32 } else { egui::Pos2::ZERO }
                };

                transform
                    .post_translate((1.0 - factor) * origin_pos.x, (1.0 - factor) * origin_pos.y)
            }
            egui::Event::MouseWheel { unit: _, delta, modifiers: _ } => {
                Transform::identity().post_translate(delta.x, delta.y)
            }
            _ => Transform::identity(),
        };
        RogerEvent::ViewportChange(transform)
    }

    #[cfg(test)]
    fn identity() -> RogerEvent {
        RogerEvent::ViewportChange(Transform::identity())
    }
}

#[derive(PartialEq, Debug, Clone, Copy)]
pub struct ToolPayload {
    pub pos: egui::Pos2,
    pub force: Option<f32>,
    pub id: Option<TouchId>,
}

impl From<egui::PointerButton> for ButtonType {
    fn from(button: egui::PointerButton) -> Self {
        match button {
            egui::PointerButton::Primary => ButtonType::Primary,
            egui::PointerButton::Secondary => ButtonType::Secondary,
            egui::PointerButton::Extra1 => ButtonType::Extra1,
            egui::PointerButton::Extra2 => ButtonType::Extra2,
            egui::PointerButton::Middle => ButtonType::Middle,
        }
    }
}

/**
 * drawing_rect, (the rect where you can draw on, according to the egui plane)
 * overlay_rects, rects where tool runs can pass through, but can't start in   
 */
#[derive(Debug)]
pub struct LayoutContext {
    draw_area: egui::Rect,
    overlay_areas: Vec<egui::Rect>,
}

impl LayoutContext {
    pub fn new(draw_area: egui::Rect, overlay_areas: Vec<egui::Rect>) -> Self {
        Self { draw_area, overlay_areas }
    }
}

impl Default for LayoutContext {
    fn default() -> Self {
        Self { draw_area: egui::Rect::EVERYTHING, overlay_areas: vec![] }
    }
}

impl Roger {
    pub fn new(config: RogerConfig) -> Self {
        Self {
            touches: Vec::new(),
            buttons: HashMap::new(),
            tool_running: None,
            viewport_changing: None,
            tool_start_touch: None,
            config,
            is_touch_frame: false,
            mouse_hover_pos: None,
            response: Default::default(),
            tool_hover_pos: (egui::Pos2::ZERO, past_instant()),
        }
    }

    pub fn process(&mut self, ui: &mut egui::Ui, layout: &LayoutContext) -> Vec<RogerEvent> {
        let mut roger_events = ui.input(|r| self.process_events(r.events.iter(), layout));
        let extended_roger_events = self.process_extended_events(ui.ctx().read_events(), layout);

        roger_events.extend(extended_roger_events);

        // coalesce all transform events into one.
        let mut transform_sum = Transform::identity();
        roger_events = roger_events
            .into_iter()
            .filter(|e| {
                if let RogerEvent::ViewportChange(t) = e {
                    transform_sum = transform_sum.post_concat(*t);
                    false
                } else {
                    true
                }
            })
            .collect::<Vec<RogerEvent>>();

        if !transform_sum.is_identity() {
            roger_events.push(RogerEvent::ViewportChange(transform_sum));
        }

        roger_events
    }

    pub fn process_extended_events(
        &mut self, events: Vec<Event>, ctx: &LayoutContext,
    ) -> Vec<RogerEvent> {
        events
            .iter()
            .filter_map(|event| self.extended_to_roger_event(event, ctx))
            .collect()
    }

    pub fn extended_to_roger_event(
        &mut self, event: &Event, ctx: &LayoutContext,
    ) -> Option<RogerEvent> {
        match event {
            Event::PredictedTouch { id, force, pos } => {
                if let Some(start_touch) = self.tool_start_touch {
                    if start_touch.eq(id) {
                        self.response.hide_overlay = pos_collides_with_layout(*pos, ctx);

                        return Some(RogerEvent::ToolPredictedRun(*pos, *force));
                    }
                };
                None
            }
            Event::MultiTouchGesture {
                rotation_delta: _,
                translation_delta,
                zoom_factor,
                start_positions,
                center_pos,
            } => {
                let invalid_touch_positions = start_positions.iter().any(|pos| {
                    pos_collides_with_layout(*pos, ctx) || !ctx.draw_area.contains(*pos)
                });

                if invalid_touch_positions {
                    return None;
                }
                let transform = Transform::identity()
                    .post_translate(translation_delta.x, translation_delta.y)
                    .post_scale(*zoom_factor, *zoom_factor)
                    .post_translate(
                        (1.0 - zoom_factor) * center_pos.x,
                        (1.0 - zoom_factor) * center_pos.y,
                    );

                if self.tool_running.is_none() {
                    self.viewport_changing = Some(Instant::now());
                    return Some(RogerEvent::ViewportChange(transform));
                }
                None
            }
            _ => None,
        }
    }

    pub fn process_events(
        &mut self, events: Iter<egui::Event>, layout: &LayoutContext,
    ) -> Vec<RogerEvent> {
        self.is_touch_frame = false;
        let result: Vec<RogerEvent> = events
            .filter_map(|event| {
                let roger_event = self.ui_to_roger_event(event, layout);

                if self.config.is_read_only
                    && !matches!(roger_event, Some(RogerEvent::ViewportChange(_)))
                {
                    return None;
                }

                if let Some(event) = roger_event {
                    if matches!(event, RogerEvent::ViewportChange(_))
                        || matches!(event, RogerEvent::ViewportChangeWithToolCancel)
                        || matches!(event, RogerEvent::ToolCancel)
                        || matches!(event, RogerEvent::ToolEnd(_))
                    {
                        self.response.hide_overlay = false;
                    }
                }

                if let Some(RogerEvent::ToolRun(..)) = roger_event {
                    self.tool_hover_pos.0 = egui::pos2(f32::NEG_INFINITY, f32::NEG_INFINITY);
                }

                roger_event
            })
            .collect();

        // dedupe hover events to only keep the last one. apple
        // sends a bunch of pen hover events and if you  draw a tool
        // hover over all of them, it will resemble a stroke
        let last_hover = result
            .iter()
            .rposition(|e| matches!(e, RogerEvent::ToolHover(_)));

        let result: Vec<RogerEvent> = result
            .into_iter()
            .enumerate()
            .filter(|(i, e)| !matches!(e, RogerEvent::ToolHover(_)) || last_hover == Some(*i))
            .map(|(_, e)| e)
            .collect();

        result
    }

    fn ui_to_roger_event(
        &mut self, event: &egui::Event, ctx: &LayoutContext,
    ) -> Option<RogerEvent> {
        let run_button =
            &MouseProps { button: ButtonType::Primary, modifiers: egui::Modifiers::NONE };

        match *event {
            egui::Event::PointerButton { pos, button, pressed, modifiers } => {
                if !self.touches.is_empty() || self.is_touch_frame {
                    return None;
                }

                let payload = ToolPayload { pos, force: None, id: None };
                let button = MouseProps { button: button.into(), modifiers };
                if pressed && !pos_collides_with_layout(pos, ctx) && ctx.draw_area.contains(pos) {
                    self.buttons.insert(button, (Instant::now(), pos));

                    if button == *run_button {
                        self.viewport_changing = None;
                        self.tool_running = Some(Instant::now());

                        return Some(RogerEvent::ToolStart(payload));
                    }
                } else if button == *run_button {
                    self.tool_running = None;
                    return Some(RogerEvent::ToolEnd(payload));
                }
                None
            }
            egui::Event::PointerMoved(pos) => {
                if !self.touches.is_empty() || self.is_touch_frame {
                    return None;
                }
                let payload = ToolPayload { pos, force: None, id: None };

                // so this is used by the transform centering logic, should it also
                // be used to display the pen hover and the eraser hover.

                if self.buttons.contains_key(run_button) && self.tool_running.is_some() {
                    self.update_hover_pos(ctx, pos, past_instant());

                    self.mouse_hover_pos = None;

                    self.response.hide_overlay = pos_collides_with_layout(pos, ctx);
                    return Some(RogerEvent::ToolRun(payload));
                }

                self.mouse_hover_pos = Some(pos);
                self.update_hover_pos(ctx, pos, Instant::now());

                if self.viewport_changing.is_none() {
                    return Some(RogerEvent::ToolHover(payload));
                }
                // todo: what happens when pointer moves outside of the canvas? do nothing or end.
                // for selection do nothing makes sense, we still wanna drag things
                // for pen tool, you wanna end.
                None
            }
            egui::Event::PointerGone => None,
            egui::Event::MouseWheel { .. } => {
                if self.tool_running.is_none() {
                    self.viewport_changing = Some(Instant::now());
                    return Some(ViewportChange::new(self, event, false));
                }

                None
            }
            egui::Event::Touch { device_id, id, phase, pos, force } => {
                self.is_touch_frame = true;
                self.touch_to_roger_event(device_id, id, pos, phase, force, ctx)
            }
            egui::Event::Zoom(..) => {
                if self.tool_running.is_none() {
                    self.viewport_changing = Some(Instant::now());
                    return Some(ViewportChange::new(self, event, false));
                }
                None
            }
            egui::Event::WindowFocused(gained_focus) => {
                if !self.touches.is_empty() || self.is_touch_frame || gained_focus {
                    return None;
                }

                // we've lost window focus
                if self.tool_running.is_some() {
                    self.reset_state();
                    return Some(RogerEvent::ToolCancel);
                }

                self.reset_state();
                None
            }
            _ => None,
        }
    }

    fn update_hover_pos(&mut self, ctx: &LayoutContext, pos: Pos2, instant: Instant) {
        if !pos_collides_with_layout(pos, ctx) {
            self.tool_hover_pos = (pos, instant);
        } else {
            self.tool_hover_pos.0 = egui::pos2(f32::NEG_INFINITY, f32::NEG_INFINITY);
        }
    }

    fn reset_state(&mut self) {
        self.touches.clear();
        self.buttons.clear();
        self.tool_running = None;
        self.viewport_changing = None;
        self.tool_start_touch = None;
    }

    fn touch_to_roger_event(
        &mut self, device_id: TouchDeviceId, id: TouchId, pos: egui::Pos2, phase: TouchPhase,
        force: Option<f32>, ctx: &LayoutContext,
    ) -> Option<RogerEvent> {
        let curr_touch_id = id;
        let is_curr_touch_pen = force.is_some();
        let event = &egui::Event::Touch { device_id, id, phase, pos, force };
        let payload = ToolPayload { pos, force, id: Some(id) };

        match phase {
            egui::TouchPhase::Start => {
                let last_touches_have_pen = self.touches.iter().any(|t| t.has_force);
                let collides = pos_collides_with_layout(pos, ctx);
                self.touches.push(TouchInfo::new(curr_touch_id, pos, force));
                if collides || !ctx.draw_area.contains(pos) {
                    return None;
                }

                if let Some(last_touch) = self.touches.iter().rev().nth(1) {
                    if !last_touches_have_pen && is_curr_touch_pen {
                        if self.tool_running.is_some() {
                            // this is non pen only mode, let the finger continue to run the tool,
                            // and ignore the pen input if the pen comes in after the touch
                            return None;
                        }
                        self.viewport_changing = None;
                        self.tool_start_touch = Some(curr_touch_id);
                        self.tool_running = Some(Instant::now());
                        return Some(RogerEvent::ToolStart(payload));
                    }

                    if !last_touches_have_pen && !is_curr_touch_pen {
                        if self.config.pencil_only_drawing {
                            // one finger touch trigers a gesture, and so does two fingers
                            self.viewport_changing = Some(Instant::now());
                            return Some(ViewportChange::new(self, event, false));
                        } else {
                            let elapsed = Instant::now() - last_touch.start;
                            // todo: source constant from pen impl
                            if elapsed < Duration::milliseconds(200) {
                                // if the two touch starts are temporaly close then it's a viewport and not
                                // a tool run. cancel the tool run and change viewpoort.
                                // for ex: cleanup the dot in the pen
                                self.viewport_changing = Some(Instant::now());
                                self.tool_start_touch = None;
                                self.tool_running = None;
                                return Some(ViewportChange::new(self, event, true));
                            } else if Some(last_touch.id) == self.tool_start_touch {
                                // else: the two fingers are spaced out. the first one runs the tool
                                // the second one will be ignored
                                return None;
                            }
                        }
                    }

                    if last_touches_have_pen {
                        return None;
                    }
                }

                if self.config.pencil_only_drawing {
                    if !is_curr_touch_pen {
                        self.viewport_changing = Some(Instant::now());
                        return Some(ViewportChange::new(self, event, false));
                    } else if self.tool_running.is_none() {
                        self.tool_running = Some(Instant::now());
                        self.tool_start_touch = Some(curr_touch_id);
                        return Some(RogerEvent::ToolStart(payload));
                    }
                } else if self.tool_running.is_none() {
                    self.tool_running = Some(Instant::now());
                    self.tool_start_touch = Some(curr_touch_id);
                    return Some(RogerEvent::ToolStart(payload));
                }
            }
            egui::TouchPhase::Move => {
                // update touch info with movement data.
                if let Some(i) = self.touches.iter().position(|&t| t.id.eq(&curr_touch_id)) {
                    let last_pos = self.touches[i].last_pos;

                    self.touches[i].frame_delta = pos - last_pos;
                    self.touches[i].lifetime_distance += last_pos.distance(pos);

                    self.touches[i].last_pos = pos;
                    if force.is_some() {
                        self.touches[i].has_force = true;
                    }
                } else {
                    return None; // maybe this touch isn't found because it failed layout check
                }

                if let Some(start_touch) = self.tool_start_touch {
                    if start_touch.eq(&curr_touch_id) {
                        self.response.hide_overlay = pos_collides_with_layout(pos, ctx);

                        return Some(RogerEvent::ToolRun(payload));
                    }
                };

                if self.viewport_changing.is_some() {
                    self.viewport_changing = Some(Instant::now());
                    return Some(ViewportChange::new(self, event, false));
                }
            }
            egui::TouchPhase::End => {
                if let Some(i) = self.touches.iter().position(|&t| t.id.eq(&curr_touch_id)) {
                    if let Some(start_touch) = self.tool_start_touch {
                        if start_touch == curr_touch_id {
                            self.tool_running = None;
                            self.tool_start_touch = None;
                            // the touch that started the tool ended, a gesture won't fire, so let's end all other touches.
                            self.touches.clear();
                            return Some(RogerEvent::ToolEnd(payload));
                        } else {
                            self.touches[i].is_active = false;
                        }
                    } else {
                        self.touches[i].is_active = false;
                    }
                }

                if self.touches.iter().all(|t| !t.is_active) {
                    // not sure if this is needed
                    self.viewport_changing = None;

                    let total_distance: f32 =
                        self.touches.iter().map(|t| t.lifetime_distance).sum();

                    let touch_count = self.touches.len();
                    self.touches.clear();

                    if total_distance < 5.0 {
                        return Some(RogerEvent::Gesture(touch_count));
                    }
                }

                let active_touches = self.touches.iter().filter(|t| t.is_active).count();
                if self.config.pencil_only_drawing && active_touches == 0
                    || !self.config.pencil_only_drawing && active_touches <= 1
                {
                    self.viewport_changing = None;
                }

                if active_touches == 0 {
                    self.touches.clear();
                }
            }
            egui::TouchPhase::Cancel => {
                if let Some(i) = self.touches.iter().position(|&t| t.id.eq(&curr_touch_id)) {
                    self.touches.remove(i);
                    if let Some(start_touch) = self.tool_start_touch {
                        if start_touch.eq(&curr_touch_id) {
                            self.tool_running = None;
                            self.tool_start_touch = None;
                            return Some(RogerEvent::ToolCancel);
                        }
                    }
                    // the touch that got canceld caused a viewport change. let's cancel the viewport change
                    if self.viewport_changing.is_some() {
                        self.viewport_changing = None;
                    }
                }
            }
        }
        None
    }

    pub fn show_hover_indicator<T: DynRogerTool + ?Sized>(
        &self, ui: &mut egui::Ui, ctx: &mut ToolContext, tool: &mut T,
    ) {
        ui.scope(|ui| {
            let old_layer = ctx.painter.layer_id();

            ctx.painter.set_layer_id(egui::LayerId {
                order: egui::Order::Middle, // todo: check if this is right
                id: "pen_overlay".into(),
            });

            let elpased = Instant::now() - self.tool_hover_pos.1;
            let target_opacity = if elpased > Duration::milliseconds(300) { 0.0 } else { 1.0 };

            let opacity = ui.ctx().animate_value_with_time(
                egui::Id::new("tool_hover_indicator_ui"),
                target_opacity,
                0.3,
            );
            ctx.painter.set_opacity(opacity);
            tool.show_hover_point(ui, self.tool_hover_pos.0, ctx);

            ctx.painter.set_layer_id(old_layer);
        });
    }

    pub fn should_hide_overlay(&self) -> bool {
        self.response.hide_overlay
    }

    pub fn sync_canvas_settings(&mut self, settings: &CanvasSettings) {
        self.config.pencil_only_drawing = settings.pencil_only_drawing;
    }
}

fn pos_collides_with_layout(pos: egui::Pos2, ctx: &LayoutContext) -> bool {
    ctx.overlay_areas.iter().any(|area| area.contains(pos))
}

// get an instant that's far in the past
fn past_instant() -> Instant {
    Instant::now() - Duration::days(1) // 1 day ago
}

#[cfg(test)]
mod tests {
    use std::vec;

    use egui::PointerButton;

    use super::*;

    trait ProcessEvents {
        fn process(&self, roger: &mut Roger, layout: &LayoutContext) -> Vec<RogerEvent>;
    }

    impl ProcessEvents for Vec<egui::Event> {
        fn process(&self, roger: &mut Roger, layout: &LayoutContext) -> Vec<RogerEvent> {
            roger.process_events(self.iter(), layout)
        }
    }

    impl ProcessEvents for Vec<Event> {
        fn process(&self, roger: &mut Roger, layout: &LayoutContext) -> Vec<RogerEvent> {
            roger.process_extended_events(self.to_owned(), layout)
        }
    }

    struct RogerTestFrame {
        events: Box<dyn ProcessEvents>,
        want: Vec<RogerEvent>,
    }

    impl RogerTestFrame {
        fn new(events: impl ProcessEvents + 'static, roger_events: Vec<RogerEvent>) -> Self {
            Self { events: Box::new(events), want: roger_events }
        }

        fn eval(&self, roger: &mut Roger, layout: &LayoutContext) {
            let got = self.events.process(roger, layout);
            for (i, w) in self.want.iter().enumerate() {
                assert_eq!(
                    w,
                    &got[i],
                    "event at index {i} mismatch. wanted {w:?} got {:?}.\nfull got: {got:?},\nfull want: {want:?}",
                    got[i],
                    want = self.want,
                );
            }
        }
    }

    struct RogerTestRunner {
        scenario: Vec<RogerTestFrame>,
    }

    #[cfg(test)]
    impl RogerTestRunner {
        fn new(scenario: Vec<RogerTestFrame>) -> Self {
            Self { scenario }
        }

        fn eval(&self, roger: &mut Roger, layout: &LayoutContext) {
            for frame in &self.scenario {
                frame.eval(roger, layout);
            }
        }
    }

    fn start_touch(id: u64, pos: egui::Pos2, force: Option<f32>) -> Vec<egui::Event> {
        vec![
            egui::Event::Touch {
                device_id: TouchDeviceId(0),
                id: TouchId(id),
                phase: TouchPhase::Start,
                pos,
                force,
            },
            egui::Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ]
    }

    fn end_touch(id: u64, pos: egui::Pos2, force: Option<f32>) -> Vec<egui::Event> {
        vec![
            egui::Event::Touch {
                device_id: TouchDeviceId(0),
                id: TouchId(id),
                phase: TouchPhase::End,
                pos,
                force,
            },
            egui::Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            },
            egui::Event::PointerGone,
        ]
    }

    fn cancel_touch(id: u64, pos: egui::Pos2, force: Option<f32>) -> Vec<egui::Event> {
        vec![
            egui::Event::Touch {
                device_id: TouchDeviceId(0),
                id: TouchId(id),
                phase: TouchPhase::Cancel,
                pos,
                force,
            },
            egui::Event::PointerGone,
        ]
    }

    fn primary_button(pos: egui::Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        }
    }

    fn mousewheel(delta: egui::Vec2) -> egui::Event {
        egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Line,
            delta,
            modifiers: egui::Modifiers::NONE,
        }
    }

    fn move_touch(id: u64, pos: egui::Pos2, force: Option<f32>) -> Vec<egui::Event> {
        vec![
            egui::Event::Touch {
                device_id: TouchDeviceId(0),
                id: TouchId(id),
                phase: TouchPhase::Move,
                pos,
                force,
            },
            egui::Event::PointerMoved(pos),
        ]
    }

    fn pointer_moved(pos: egui::Pos2) -> egui::Event {
        egui::Event::PointerMoved(pos)
    }

    #[test]
    fn button_then_mousewheel() {
        let mut roger = Roger::new(RogerConfig::default());

        let payload = ToolPayload { pos: egui::Pos2::ZERO, force: None, id: None };

        let test = RogerTestRunner::new(vec![RogerTestFrame::new(
            vec![
                primary_button(egui::Pos2::ZERO, true),
                egui::Event::PointerMoved(egui::Pos2::ZERO),
                egui::Event::PointerMoved(egui::Pos2::ZERO),
                mousewheel(egui::Vec2::ZERO), // pan doesn't do anything while tool is running
                primary_button(egui::Pos2::ZERO, false),
                egui::Event::PointerMoved(egui::Pos2::ZERO),
                mousewheel(egui::Vec2::ZERO), // pan now works because tool stopped running
            ],
            vec![
                RogerEvent::ToolStart(payload),
                RogerEvent::ToolRun(payload),
                RogerEvent::ToolRun(payload),
                RogerEvent::ToolEnd(payload),
                RogerEvent::ToolHover(payload),
                ViewportChange::identity(),
            ],
        )]);

        test.eval(&mut roger, &LayoutContext::default());
    }

    #[test]
    fn mousewheel_then_button() {
        let mut roger = Roger::new(RogerConfig::default());
        let payload = ToolPayload { pos: egui::Pos2::ZERO, force: None, id: None };

        let test = RogerTestRunner::new(vec![
            RogerTestFrame::new(
                vec![mousewheel(egui::Vec2::ZERO)],
                vec![ViewportChange::identity()],
            ),
            RogerTestFrame::new(
                vec![primary_button(egui::Pos2::ZERO, true)],
                vec![RogerEvent::ToolStart(payload)],
            ),
            RogerTestFrame::new(
                vec![egui::Event::PointerMoved(egui::Pos2::ZERO)],
                vec![RogerEvent::ToolRun(payload)],
            ),
            RogerTestFrame::new(
                vec![egui::Event::PointerMoved(egui::Pos2::ZERO)],
                vec![RogerEvent::ToolRun(payload)],
            ),
            RogerTestFrame::new(
                vec![primary_button(egui::Pos2::ZERO, false)],
                vec![RogerEvent::ToolEnd(payload)],
            ),
            RogerTestFrame::new(
                vec![egui::Event::PointerMoved(egui::Pos2::ZERO)],
                vec![RogerEvent::ToolHover(payload)],
            ),
        ]);

        test.eval(&mut roger, &LayoutContext::default());
    }
    #[test]
    fn single_pen_touch() {
        let mut roger = Roger::new(RogerConfig::default());
        let pos = egui::Pos2::new(10.0, 10.0);
        let force = Some(0.5);
        let payload = ToolPayload { pos, force, id: Some(TouchId(1)) };

        let test = RogerTestRunner::new(vec![
            RogerTestFrame::new(start_touch(1, pos, force), vec![RogerEvent::ToolStart(payload)]),
            RogerTestFrame::new(move_touch(1, pos, force), vec![RogerEvent::ToolRun(payload)]),
            RogerTestFrame::new(end_touch(1, pos, force), vec![RogerEvent::ToolEnd(payload)]),
        ]);

        test.eval(&mut roger, &LayoutContext::default());
    }

    #[test]
    fn single_finger_touch_default_mode() {
        let mut roger = Roger::new(RogerConfig::default());
        let pos = egui::Pos2::new(10.0, 10.0);
        let payload = ToolPayload { pos, force: None, id: Some(TouchId(1)) };

        let test = RogerTestRunner::new(vec![
            RogerTestFrame::new(start_touch(1, pos, None), vec![RogerEvent::ToolStart(payload)]),
            RogerTestFrame::new(move_touch(1, pos, None), vec![RogerEvent::ToolRun(payload)]),
            RogerTestFrame::new(end_touch(1, pos, None), vec![RogerEvent::ToolEnd(payload)]),
        ]);

        test.eval(&mut roger, &LayoutContext::default());
    }

    #[test]
    fn single_finger_touch_pencil_only_mode_big_movement() {
        let mut roger = Roger::new(RogerConfig::new(true, false));
        let pos = egui::Pos2::new(10.0, 10.0);

        let test = RogerTestRunner::new(vec![
            RogerTestFrame::new(start_touch(1, pos, None), vec![ViewportChange::identity()]),
            RogerTestFrame::new(
                move_touch(1, pos + egui::vec2(10.0, 10.0), None), // big movement will not trigger a gesture
                vec![ViewportChange::identity()],
            ),
            RogerTestFrame::new(end_touch(1, pos, None), vec![]),
        ]);

        test.eval(&mut roger, &LayoutContext::default());
    }

    #[test]
    fn single_finger_touch_pencil_only_mode_small_movement() {
        let mut roger = Roger::new(RogerConfig::new(true, false));
        let pos = egui::Pos2::new(10.0, 10.0);

        let test = RogerTestRunner::new(vec![
            RogerTestFrame::new(start_touch(1, pos, None), vec![ViewportChange::identity()]),
            RogerTestFrame::new(
                move_touch(1, pos + egui::vec2(0.0, 3.0), None), // small movement will trigger a gesture
                vec![ViewportChange::identity()],
            ),
            RogerTestFrame::new(end_touch(1, pos, None), vec![RogerEvent::Gesture(1)]),
        ]);

        test.eval(&mut roger, &LayoutContext::default());
    }
    #[test]
    fn two_finger_gesture() {
        let mut roger = Roger::new(RogerConfig::default());
        let pos1 = egui::Pos2::new(10.0, 10.0);
        let pos2 = egui::Pos2::new(20.0, 20.0);

        let test = RogerTestRunner::new(vec![
            RogerTestFrame::new(
                start_touch(1, pos1, None),
                vec![RogerEvent::ToolStart(ToolPayload {
                    pos: pos1,
                    force: None,
                    id: Some(TouchId(1)),
                })],
            ),
            RogerTestFrame::new(
                start_touch(2, pos2, None), // Second finger within 200ms
                vec![ViewportChange::new(&roger, &start_touch(2, pos2, None)[0], true)],
            ),
            RogerTestFrame::new(move_touch(1, pos1, None), vec![ViewportChange::identity()]),
            RogerTestFrame::new(move_touch(2, pos2, None), vec![ViewportChange::identity()]),
            RogerTestFrame::new(end_touch(1, pos1, None), vec![]),
            RogerTestFrame::new(
                end_touch(2, pos2, None),
                vec![RogerEvent::Gesture(2)], // Two finger tap gesture
            ),
        ]);

        test.eval(&mut roger, &LayoutContext::default());
    }

    #[test]
    fn two_finger_viewport_change_then_pen() {
        let mut roger = Roger::new(RogerConfig::default());
        let pos1 = egui::Pos2::new(10.0, 10.0);
        let pos2 = egui::Pos2::new(20.0, 20.0);
        let pos3 = egui::Pos2::new(30.0, 30.0);

        let pen_payload = ToolPayload { pos: pos3, force: Some(0.5), id: Some(TouchId(3)) };

        let test = RogerTestRunner::new(vec![
            RogerTestFrame::new(
                start_touch(1, pos1, None),
                vec![RogerEvent::ToolStart(ToolPayload {
                    pos: pos1,
                    force: None,
                    id: Some(TouchId(1)),
                })],
            ),
            RogerTestFrame::new(
                start_touch(2, pos2, None), // Second finger within 200ms
                vec![ViewportChange::new(&roger, &start_touch(2, pos2, None)[0], true)],
            ),
            RogerTestFrame::new(
                move_touch(1, pos1 + egui::vec2(10.0, 10.0), None),
                vec![ViewportChange::identity()],
            ),
            RogerTestFrame::new(
                move_touch(2, pos2 + egui::vec2(10.0, 10.0), None),
                vec![ViewportChange::identity()],
            ),
            RogerTestFrame::new(end_touch(1, pos1, None), vec![]),
            RogerTestFrame::new(end_touch(2, pos2, None), vec![]),
            RogerTestFrame::new(
                start_touch(3, pen_payload.pos, pen_payload.force),
                vec![RogerEvent::ToolStart(pen_payload)],
            ),
            RogerTestFrame::new(
                move_touch(3, pen_payload.pos, pen_payload.force),
                vec![RogerEvent::ToolRun(pen_payload)],
            ),
        ]);

        test.eval(&mut roger, &LayoutContext::default());
    }

    #[test]
    fn pen_then_finger_ignores_finger() {
        let mut roger = Roger::new(RogerConfig::default());
        let pos1 = egui::Pos2::new(10.0, 10.0);
        let pos2 = egui::Pos2::new(20.0, 20.0);
        let force = Some(0.5);
        let pen_payload = ToolPayload { pos: pos1, force, id: Some(TouchId(1)) };

        let test = RogerTestRunner::new(vec![
            RogerTestFrame::new(
                start_touch(1, pos1, force), // Pen starts
                vec![RogerEvent::ToolStart(pen_payload)],
            ),
            RogerTestFrame::new(
                start_touch(2, pos2, None), // Finger starts - should be ignored
                vec![],
            ),
            RogerTestFrame::new(move_touch(1, pos1, force), vec![RogerEvent::ToolRun(pen_payload)]),
            RogerTestFrame::new(end_touch(1, pos1, force), vec![RogerEvent::ToolEnd(pen_payload)]),
        ]);

        test.eval(&mut roger, &LayoutContext::default());
    }

    #[test]
    fn pen_then_two_fingers_ignores_fingers() {
        let mut roger = Roger::new(RogerConfig::default());
        let pos1 = egui::Pos2::new(10.0, 10.0);
        let pos2 = egui::Pos2::new(20.0, 20.0);
        let pos3 = egui::Pos2::new(30.0, 30.0);
        let pos4 = egui::Pos2::new(40.0, 40.0);

        let force = Some(0.5);
        let pen_1_payload = ToolPayload { pos: pos1, force, id: Some(TouchId(1)) };
        let pen_2_payload = ToolPayload { pos: pos4, force, id: Some(TouchId(4)) };

        let test = RogerTestRunner::new(vec![
            RogerTestFrame::new(
                start_touch(1, pos1, force), // Pen starts
                vec![RogerEvent::ToolStart(pen_1_payload)],
            ),
            RogerTestFrame::new(
                start_touch(2, pos2, None), // Finger starts - should be ignored
                vec![],
            ),
            RogerTestFrame::new(
                start_touch(3, pos3, None), // Finger 2 starts - should be ignored
                vec![],
            ),
            RogerTestFrame::new(
                move_touch(1, pos1, force),
                vec![RogerEvent::ToolRun(pen_1_payload)],
            ),
            RogerTestFrame::new(end_touch(2, pos2, None), vec![]),
            RogerTestFrame::new(end_touch(3, pos3, None), vec![]),
            RogerTestFrame::new(
                end_touch(1, pos1, force),
                vec![RogerEvent::ToolEnd(pen_1_payload)],
            ),
            RogerTestFrame::new(
                start_touch(4, pos4, force),
                vec![RogerEvent::ToolStart(pen_2_payload)],
            ),
        ]);

        test.eval(&mut roger, &LayoutContext::default());
    }

    #[test]
    fn finger_then_pen_ignore_pen() {
        let mut roger = Roger::new(RogerConfig::default());
        let pos1 = egui::Pos2::new(10.0, 10.0);
        let pos2 = egui::Pos2::new(20.0, 20.0);
        let force = Some(0.5);
        let touch_payloud = ToolPayload { pos: pos2, force: None, id: Some(TouchId(1)) };

        let test = RogerTestRunner::new(vec![
            RogerTestFrame::new(
                start_touch(1, pos1, None), // Finger starts
                vec![RogerEvent::ToolStart(ToolPayload {
                    pos: pos1,
                    force: None,
                    id: Some(TouchId(1)),
                })],
            ),
            RogerTestFrame::new(
                start_touch(2, pos2, force), // Pen starts, but it's ignored because the finger that runs the tool started first
                vec![],
            ),
            RogerTestFrame::new(move_touch(2, pos2, force), vec![]),
            RogerTestFrame::new(
                move_touch(1, pos2, None),
                vec![RogerEvent::ToolRun(ToolPayload {
                    pos: pos2,
                    force: None,
                    id: Some(TouchId(1)),
                })],
            ),
            RogerTestFrame::new(end_touch(1, pos2, None), vec![RogerEvent::ToolEnd(touch_payloud)]),
        ]);

        test.eval(&mut roger, &LayoutContext::default());
    }

    #[test]
    fn touch_cancel_during_tool_run() {
        let mut roger = Roger::new(RogerConfig::default());
        let pos = egui::Pos2::new(10.0, 10.0);
        let force = Some(0.5);
        let payload = ToolPayload { pos, force, id: Some(TouchId(1)) };

        let test = RogerTestRunner::new(vec![
            RogerTestFrame::new(start_touch(1, pos, force), vec![RogerEvent::ToolStart(payload)]),
            RogerTestFrame::new(move_touch(1, pos, force), vec![RogerEvent::ToolRun(payload)]),
            RogerTestFrame::new(cancel_touch(1, pos, force), vec![RogerEvent::ToolCancel]),
        ]);

        test.eval(&mut roger, &LayoutContext::default());
    }

    #[test]
    fn kinetic_scroll() {
        let mut roger = Roger::new(RogerConfig::new(true, false));
        let pos = egui::Pos2::new(10.0, 10.0);
        let force = Some(0.5);
        let payload = ToolPayload { pos, force, id: Some(TouchId(1)) };

        let test = RogerTestRunner::new(vec![
            RogerTestFrame::new(start_touch(1, pos, None), vec![ViewportChange::identity()]),
            RogerTestFrame::new(
                move_touch(1, pos + egui::vec2(100.0, 100.0), None),
                vec![ViewportChange::identity()],
            ),
            RogerTestFrame::new(end_touch(1, pos + egui::vec2(100.0, 100.0), None), vec![]),
            RogerTestFrame::new(
                vec![Event::MultiTouchGesture {
                    rotation_delta: 0.0,
                    translation_delta: egui::Vec2::new(0.0, 0.0),
                    zoom_factor: 1.0,
                    center_pos: egui::Pos2::ZERO,
                    start_positions: vec![pos],
                }],
                vec![ViewportChange::identity()],
            ),
            RogerTestFrame::new(start_touch(1, pos, force), vec![RogerEvent::ToolStart(payload)]),
        ]);

        test.eval(&mut roger, &LayoutContext::default());
    }

    #[test]
    fn touch_cancel_during_viewport_change() {
        let mut roger = Roger::new(RogerConfig::new(true, false));
        let pos = egui::Pos2::new(10.0, 10.0);
        let force = Some(0.5);
        let pen_payload = ToolPayload { pos, force, id: Some(TouchId(2)) };

        let test = RogerTestRunner::new(vec![
            RogerTestFrame::new(start_touch(1, pos, None), vec![ViewportChange::identity()]),
            RogerTestFrame::new(move_touch(1, pos, None), vec![ViewportChange::identity()]),
            RogerTestFrame::new(cancel_touch(1, pos, None), vec![]),
            RogerTestFrame::new(
                start_touch(2, pen_payload.pos, pen_payload.force),
                vec![RogerEvent::ToolStart(pen_payload)],
            ),
        ]);

        test.eval(&mut roger, &LayoutContext::default());
    }

    #[test]
    fn read_only_mode_blocks_tool_events() {
        let mut roger = Roger::new(RogerConfig::new(false, true));
        let pos = egui::Pos2::new(10.0, 10.0);

        let test = RogerTestRunner::new(vec![
            RogerTestFrame::new(
                start_touch(1, pos, Some(0.5)),
                vec![], // Tool events blocked in read-only
            ),
            RogerTestFrame::new(
                end_touch(1, pos, Some(0.5)),
                vec![], // Tool events blocked in read-only
            ),
            RogerTestFrame::new(
                vec![mousewheel(egui::Vec2::new(0.0, 0.0))],
                vec![ViewportChange::identity()], // Viewport changes still work
            ),
        ]);

        test.eval(&mut roger, &LayoutContext::default());
    }

    #[test]
    fn mouse_events_ignored_during_touch() {
        let mut roger = Roger::new(RogerConfig::default());
        let pos = egui::Pos2::new(10.0, 10.0);
        let touch_payload = ToolPayload { pos, force: None, id: Some(TouchId(1)) };

        let test = RogerTestRunner::new(vec![
            RogerTestFrame::new(
                [
                    start_touch(1, pos, None),
                    vec![primary_button(pos, true)], // Mouse event during touch - ignored
                ]
                .concat(),
                vec![RogerEvent::ToolStart(touch_payload)],
            ),
            RogerTestFrame::new(move_touch(1, pos, None), vec![RogerEvent::ToolRun(touch_payload)]),
            RogerTestFrame::new(vec![pointer_moved(pos)], vec![]),
        ]);

        test.eval(&mut roger, &LayoutContext::default());
    }

    #[test]
    fn three_finger_tap_gesture() {
        let mut roger = Roger::new(RogerConfig::new(true, false));
        let pos1 = egui::Pos2::new(10.0, 10.0);
        let pos2 = egui::Pos2::new(20.0, 20.0);
        let pos3 = egui::Pos2::new(30.0, 30.0);

        let test = RogerTestRunner::new(vec![
            RogerTestFrame::new(start_touch(1, pos1, None), vec![ViewportChange::identity()]),
            RogerTestFrame::new(start_touch(2, pos2, None), vec![ViewportChange::identity()]),
            RogerTestFrame::new(start_touch(3, pos3, None), vec![ViewportChange::identity()]),
            RogerTestFrame::new(end_touch(1, pos1, None), vec![]),
            RogerTestFrame::new(end_touch(2, pos2, None), vec![]),
            RogerTestFrame::new(end_touch(3, pos3, None), vec![RogerEvent::Gesture(3)]),
        ]);

        test.eval(&mut roger, &LayoutContext::default());
    }

    #[test]
    fn pen_movement_updates_position() {
        let mut roger = Roger::new(RogerConfig::default());
        let pos1 = egui::Pos2::new(10.0, 10.0);
        let pos2 = egui::Pos2::new(15.0, 15.0);
        let pos3 = egui::Pos2::new(20.0, 20.0);
        let force = Some(0.7);

        let test = RogerTestRunner::new(vec![
            RogerTestFrame::new(
                start_touch(1, pos1, force),
                vec![RogerEvent::ToolStart(ToolPayload { pos: pos1, force, id: Some(TouchId(1)) })],
            ),
            RogerTestFrame::new(
                move_touch(1, pos2, force),
                vec![RogerEvent::ToolRun(ToolPayload { pos: pos2, force, id: Some(TouchId(1)) })],
            ),
            RogerTestFrame::new(
                move_touch(1, pos3, force),
                vec![RogerEvent::ToolRun(ToolPayload { pos: pos3, force, id: Some(TouchId(1)) })],
            ),
        ]);

        test.eval(&mut roger, &LayoutContext::default());
    }

    #[test]
    fn tool_run_collides_with_layout() {
        let mut roger = Roger::new(RogerConfig::default());
        let layout = LayoutContext {
            draw_area: egui::Rect::EVERYTHING,
            overlay_areas: vec![egui::Rect::from_min_size(
                egui::Pos2::new(0.0, 0.0),
                egui::vec2(10.0, 10.0),
            )],
        };

        let pos1 = egui::Pos2::new(5.0, 5.0);
        let force = Some(0.7);

        let test = RogerTestRunner::new(vec![
            RogerTestFrame::new(start_touch(1, pos1, force), vec![]),
            RogerTestFrame::new(move_touch(1, pos1, force), vec![]),
        ]);

        test.eval(&mut roger, &layout);
    }
}
