use std::{collections::HashMap, slice::Iter};

use egui::{TouchDeviceId, TouchId, TouchPhase};
use time::Duration;
use tracing::{debug, warn};
use web_time::Instant;

#[derive(Debug)]
pub struct Roger {
    touches: Vec<TouchInfo>,
    buttons: HashMap<MouseProps, Instant>,
    tool_running: Option<Instant>,
    tool_start_touch: Option<Pointer>, // keep track of the touch id that started a touch, to inform tool end
    viewport_changing: Option<Instant>,
    config: RogerConfig,
    is_touch_frame: bool, // as we traverse the input event stream do we see touch events.
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

#[derive(Eq, Hash, PartialEq, Debug, Clone, Copy)]
enum Pointer {
    Finger(u64), // Touch ID
    Pen(u64),    // Pen ID
}
impl Pointer {
    fn id(self) -> u64 {
        match self {
            Pointer::Finger(id) => id,
            Pointer::Pen(id) => id,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct TouchInfo {
    id: Pointer,
    start: Instant,
    is_active: bool,
    lifetime_distance: f32,
    frame_delta: egui::Vec2,
    last_pos: egui::Pos2,
}

impl TouchInfo {
    fn new(id: Pointer, pos: egui::Pos2) -> Self {
        Self {
            id,
            start: Instant::now(),
            is_active: true,
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
    Tertiary,
    Middle,
    Back,
    Extra1,
    Extra2,
}

#[derive(PartialEq, Debug)]
pub enum RogerEvent {
    ToolStart(ToolPayload),
    ToolRun(ToolPayload),
    ToolEnd(ToolPayload),
    ToolCancel,
    ToolHover(ToolPayload),
    ViewportChange,
    Gesture(usize), // number of fingers in the gesture, ex: two finger undo
    ViewportChangeWithToolCancel,
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
        }
    }

    pub fn process(&mut self, ui: &mut egui::Ui) -> Vec<RogerEvent> {
        ui.input(|r| self.process_events(r.events.iter()))
    }

    pub fn process_events(&mut self, events: Iter<egui::Event>) -> Vec<RogerEvent> {
        self.is_touch_frame = false;
        events
            .filter_map(|event| {
                let roger_event = self.ui_to_roger_event(event);
                debug!(?event, ?roger_event, "processing ui event to roger ");
                debug!(?self, "roger state");

                // only return viewport change events on read only mode.
                if self.config.is_read_only
                    && !matches!(roger_event, Some(RogerEvent::ViewportChange))
                {
                    None
                } else {
                    roger_event
                }
            })
            .collect()
    }

    fn ui_to_roger_event(&mut self, event: &egui::Event) -> Option<RogerEvent> {
        let run_button =
            &MouseProps { button: ButtonType::Primary, modifiers: egui::Modifiers::NONE };

        match *event {
            egui::Event::PointerButton { pos, button, pressed, modifiers } => {
                if !self.touches.is_empty() || self.is_touch_frame {
                    return None;
                }

                let payload = ToolPayload { pos, force: None, id: None };
                let button = MouseProps { button: button.into(), modifiers };
                if pressed {
                    self.buttons.insert(button, Instant::now());

                    if button == *run_button {
                        self.viewport_changing = None;
                        self.tool_running = Some(Instant::now());

                        return Some(RogerEvent::ToolStart(payload));
                    }
                } else {
                    let exists = self.buttons.remove(&button).is_some();
                    if !exists {
                        warn!(
                            "Mouse Button {:?} at position {:?} released without being pressed",
                            button, pos
                        );
                    }

                    if button == *run_button {
                        self.tool_running = None;
                        return Some(RogerEvent::ToolEnd(payload));
                    }
                }
                None
            }
            egui::Event::PointerMoved(pos) => {
                if !self.touches.is_empty() || self.is_touch_frame {
                    return None;
                }
                let payload = ToolPayload { pos, force: None, id: None };

                if self.buttons.contains_key(run_button) && self.tool_running.is_some() {
                    return Some(RogerEvent::ToolRun(payload));
                }
                if self.viewport_changing.is_none() {
                    return Some(RogerEvent::ToolHover(payload));
                }
                // todo: what happens when pointer moves outside of the canvas? do nothing or end.
                // for selection do nothing makes sense, we still wanna drag things
                // for pen tool, you wanna end.
                None
            }
            egui::Event::PointerGone => {
                println!("Pointer gone");
                None
            }
            egui::Event::MouseWheel { unit, delta, modifiers } => {
                if self.tool_running.is_none() {
                    self.viewport_changing = Some(Instant::now());
                    return Some(RogerEvent::ViewportChange);
                }
                // when did we aquire the tool run lock. if it's less than 100ms ago, then we can assume
                // this is a pan and not a tool run
                None
            }
            egui::Event::Touch { device_id, id, phase, pos, force } => {
                self.is_touch_frame = true;
                return self.touch_to_roger_event(device_id, id, pos, phase, force);
            }
            egui::Event::Zoom(factor) => {
                println!("Zoom event with factor: {:?}", factor);
                None
            }
            _ => None,
        }
    }

    fn touch_to_roger_event(
        &mut self, device_id: TouchDeviceId, id: TouchId, pos: egui::Pos2, phase: TouchPhase,
        force: Option<f32>,
    ) -> Option<RogerEvent> {
        let curr_touch_id =
            if force.is_some() { Pointer::Pen(id.0) } else { Pointer::Finger(id.0) };
        let payload = ToolPayload { pos, force, id: Some(id) };

        match phase {
            egui::TouchPhase::Start => {
                self.touches.push(TouchInfo::new(curr_touch_id, pos));
                if let Some(last_touch) = self.touches.iter().rev().nth(1) {
                    match last_touch.id {
                        Pointer::Finger(_) => match curr_touch_id {
                            Pointer::Finger(_) => {
                                if self.config.pencil_only_drawing {
                                    // one finger touch trigers a gesture, and so does two fingers
                                    self.viewport_changing = Some(Instant::now());
                                    return Some(RogerEvent::ViewportChange);
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
                                        return Some(RogerEvent::ViewportChangeWithToolCancel);
                                    } else {
                                        // else: the two fingers are spaced out. the first one runs the tool
                                        // the second one will be ignored
                                        return None;
                                    }
                                }
                            }
                            Pointer::Pen(_) => {
                                self.viewport_changing = None;
                                self.tool_start_touch = Some(curr_touch_id);
                                self.tool_running = Some(Instant::now());
                                return Some(RogerEvent::ToolStart(payload));
                            }
                        },
                        Pointer::Pen(_) => {} // never interrupt a tool run invoked by a pen,
                    }
                }
                // there's only one touch, but maybe there's a mousewheel event. exmaple ipad touchpad?
                // and we're viewport changing. respect that
                if self.viewport_changing.is_some() {
                    return None;
                }

                if self.config.pencil_only_drawing && matches!(curr_touch_id, Pointer::Finger(_)) {
                    self.viewport_changing = Some(Instant::now());
                    return Some(RogerEvent::ViewportChange);
                }
                self.tool_running = Some(Instant::now());
                self.tool_start_touch = Some(curr_touch_id);
                return Some(RogerEvent::ToolStart(payload));
            }
            egui::TouchPhase::Move => {
                // update touch info with movement data.
                if let Some(i) = self.touches.iter().position(|&t| t.id.eq(&curr_touch_id)) {
                    let last_pos = self.touches[i].last_pos;

                    self.touches[i].frame_delta = pos - last_pos;
                    self.touches[i].lifetime_distance += last_pos.distance(pos);

                    self.touches[i].last_pos = pos;
                }

                if let Some(start_touch) = self.tool_start_touch {
                    if start_touch.eq(&curr_touch_id) {
                        return Some(RogerEvent::ToolRun(payload));
                    }
                };

                if self.viewport_changing.is_some() {
                    self.viewport_changing = Some(Instant::now());
                    return Some(RogerEvent::ViewportChange);
                }
            }
            egui::TouchPhase::End => {
                if let Some(i) = self.touches.iter().position(|&t| t.id.eq(&curr_touch_id)) {
                    if let Some(start_touch) = self.tool_start_touch {
                        if start_touch.eq(&curr_touch_id) {
                            self.tool_running = None;
                            self.tool_start_touch = None;
                            // the touch that started the tool ended, a gesture won't fire, so let's end all other touches.
                            self.touches.remove(i);
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
                if self.config.pencil_only_drawing && active_touches == 0 {
                    self.viewport_changing = None;
                } else if !self.config.pencil_only_drawing && active_touches == 1 {
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
                } else {
                    warn!(?id, ?force, "cancelling a touch that didn't start")
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use egui::{Event, PointerButton};
    use futures::AsyncReadExt;

    use super::*;

    struct RogerTestFrame {
        frame: (Vec<egui::Event>, Vec<RogerEvent>),
    }

    impl RogerTestFrame {
        fn new(ui_events: Vec<egui::Event>, roger_events: Vec<RogerEvent>) -> Self {
            Self { frame: (ui_events, roger_events) }
        }

        fn eval(&self, roger: &mut Roger) {
            let test_data = self.frame.0.iter();
            let want = &self.frame.1;

            let got = roger.process_events(test_data);

            assert_eq!(want, &got)
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

        fn eval(&self, roger: &mut Roger) {
            for frame in &self.scenario {
                frame.eval(roger);
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

    fn move_touch(id: u64, pos: egui::Pos2, force: Option<f32>) -> egui::Event {
        egui::Event::Touch {
            device_id: TouchDeviceId(0),
            id: TouchId(id),
            phase: TouchPhase::Move,
            pos,
            force,
        }
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
                Event::PointerMoved(egui::Pos2::ZERO),
                Event::PointerMoved(egui::Pos2::ZERO),
                mousewheel(egui::Vec2::ZERO), // pan doesn't do anything while tool is running
                primary_button(egui::Pos2::ZERO, false),
                Event::PointerMoved(egui::Pos2::ZERO),
                mousewheel(egui::Vec2::ZERO), // pan now works because tool stopped running
            ],
            vec![
                RogerEvent::ToolStart(payload),
                RogerEvent::ToolRun(payload),
                RogerEvent::ToolRun(payload),
                RogerEvent::ToolEnd(payload),
                RogerEvent::ToolHover(payload),
                RogerEvent::ViewportChange,
            ],
        )]);

        test.eval(&mut roger);
    }

    #[test]
    fn mousewheel_then_button() {
        let mut roger = Roger::new(RogerConfig::default());
        let payload = ToolPayload { pos: egui::Pos2::ZERO, force: None, id: None };

        let test = RogerTestRunner::new(vec![RogerTestFrame::new(
            vec![
                mousewheel(egui::Vec2::ZERO),
                primary_button(egui::Pos2::ZERO, true),
                Event::PointerMoved(egui::Pos2::ZERO),
                Event::PointerMoved(egui::Pos2::ZERO),
                primary_button(egui::Pos2::ZERO, false),
                Event::PointerMoved(egui::Pos2::ZERO),
            ],
            vec![
                RogerEvent::ViewportChange,
                RogerEvent::ToolStart(payload),
                RogerEvent::ToolRun(payload),
                RogerEvent::ToolRun(payload),
                RogerEvent::ToolEnd(payload),
                RogerEvent::ToolHover(payload),
            ],
        )]);

        test.eval(&mut roger);
    }
    #[test]
    fn single_pen_touch() {
        let mut roger = Roger::new(RogerConfig::default());
        let pos = egui::Pos2::new(10.0, 10.0);
        let force = Some(0.5);
        let payload = ToolPayload { pos, force, id: Some(TouchId(1)) };

        let test = RogerTestRunner::new(vec![
            RogerTestFrame::new(start_touch(1, pos, force), vec![RogerEvent::ToolStart(payload)]),
            RogerTestFrame::new(
                vec![move_touch(1, pos, force)],
                vec![RogerEvent::ToolRun(payload)],
            ),
            RogerTestFrame::new(end_touch(1, pos, force), vec![RogerEvent::ToolEnd(payload)]),
        ]);

        test.eval(&mut roger);
    }

    #[test]
    fn single_finger_touch_default_mode() {
        let mut roger = Roger::new(RogerConfig::default());
        let pos = egui::Pos2::new(10.0, 10.0);
        let payload = ToolPayload { pos, force: None, id: Some(TouchId(1)) };

        let test = RogerTestRunner::new(vec![
            RogerTestFrame::new(start_touch(1, pos, None), vec![RogerEvent::ToolStart(payload)]),
            RogerTestFrame::new(vec![move_touch(1, pos, None)], vec![RogerEvent::ToolRun(payload)]),
            RogerTestFrame::new(end_touch(1, pos, None), vec![RogerEvent::ToolEnd(payload)]),
        ]);

        test.eval(&mut roger);
    }

    #[test]
    fn single_finger_touch_pencil_only_mode() {
        let mut roger = Roger::new(RogerConfig::new(true, false));
        let pos = egui::Pos2::new(10.0, 10.0);

        let test = RogerTestRunner::new(vec![
            RogerTestFrame::new(start_touch(1, pos, None), vec![RogerEvent::ViewportChange]),
            RogerTestFrame::new(
                vec![move_touch(1, pos + egui::vec2(10.0, 10.0), None)],
                vec![RogerEvent::ViewportChange],
            ),
            RogerTestFrame::new(end_touch(1, pos, None), vec![]),
        ]);

        test.eval(&mut roger);
    }

    #[test]
    fn single_finger_touch_pencil_only_mode_small_movement() {
        let mut roger = Roger::new(RogerConfig::new(true, false));
        let pos = egui::Pos2::new(10.0, 10.0);

        let test = RogerTestRunner::new(vec![
            RogerTestFrame::new(start_touch(1, pos, None), vec![RogerEvent::ViewportChange]),
            RogerTestFrame::new(
                vec![move_touch(1, pos + egui::vec2(0.0, 3.0), None)], // small movement will triger a gesture©
                vec![RogerEvent::ViewportChange],
            ),
            RogerTestFrame::new(end_touch(1, pos, None), vec![RogerEvent::Gesture(1)]),
        ]);

        test.eval(&mut roger);
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
                vec![RogerEvent::ViewportChangeWithToolCancel],
            ),
            RogerTestFrame::new(
                vec![move_touch(1, pos1, None), move_touch(2, pos2, None)],
                vec![RogerEvent::ViewportChange, RogerEvent::ViewportChange],
            ),
            RogerTestFrame::new(end_touch(1, pos1, None), vec![]),
            RogerTestFrame::new(
                end_touch(2, pos2, None),
                vec![RogerEvent::Gesture(2)], // Two finger tap gesture
            ),
        ]);

        test.eval(&mut roger);
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
            RogerTestFrame::new(
                vec![move_touch(1, pos1, force)],
                vec![RogerEvent::ToolRun(pen_payload)],
            ),
            RogerTestFrame::new(end_touch(1, pos1, force), vec![RogerEvent::ToolEnd(pen_payload)]),
        ]);

        test.eval(&mut roger);
    }

    #[test]
    fn finger_then_pen_starts_pen() {
        let mut roger = Roger::new(RogerConfig::default());
        let pos1 = egui::Pos2::new(10.0, 10.0);
        let pos2 = egui::Pos2::new(20.0, 20.0);
        let force = Some(0.5);
        let pen_payload = ToolPayload { pos: pos2, force, id: Some(TouchId(2)) };

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
                start_touch(2, pos2, force), // Pen starts
                vec![RogerEvent::ToolStart(pen_payload)],
            ),
            RogerTestFrame::new(
                vec![move_touch(2, pos2, force)],
                vec![RogerEvent::ToolRun(pen_payload)],
            ),
            RogerTestFrame::new(end_touch(2, pos2, force), vec![RogerEvent::ToolEnd(pen_payload)]),
        ]);

        test.eval(&mut roger);
    }

    #[test]
    fn touch_cancel_during_tool_run() {
        let mut roger = Roger::new(RogerConfig::default());
        let pos = egui::Pos2::new(10.0, 10.0);
        let force = Some(0.5);
        let payload = ToolPayload { pos, force, id: Some(TouchId(1)) };

        let test = RogerTestRunner::new(vec![
            RogerTestFrame::new(start_touch(1, pos, force), vec![RogerEvent::ToolStart(payload)]),
            RogerTestFrame::new(
                vec![move_touch(1, pos, force)],
                vec![RogerEvent::ToolRun(payload)],
            ),
            RogerTestFrame::new(cancel_touch(1, pos, force), vec![RogerEvent::ToolCancel]),
        ]);

        test.eval(&mut roger);
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
                vec![mousewheel(egui::Vec2::new(0.0, 1.0))],
                vec![RogerEvent::ViewportChange], // Viewport changes still work
            ),
        ]);

        test.eval(&mut roger);
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
            RogerTestFrame::new(
                vec![
                    move_touch(1, pos, None),
                    pointer_moved(pos), // Mouse move during touch - ignored
                ],
                vec![RogerEvent::ToolRun(touch_payload)],
            ),
        ]);

        test.eval(&mut roger);
    }

    #[test]
    fn three_finger_tap_gesture() {
        let mut roger = Roger::new(RogerConfig::new(true, false));
        let pos1 = egui::Pos2::new(10.0, 10.0);
        let pos2 = egui::Pos2::new(20.0, 20.0);
        let pos3 = egui::Pos2::new(30.0, 30.0);

        let test = RogerTestRunner::new(vec![
            RogerTestFrame::new(start_touch(1, pos1, None), vec![RogerEvent::ViewportChange]),
            RogerTestFrame::new(start_touch(2, pos2, None), vec![RogerEvent::ViewportChange]),
            RogerTestFrame::new(start_touch(3, pos3, None), vec![]),
            RogerTestFrame::new(end_touch(1, pos1, None), vec![]),
            RogerTestFrame::new(end_touch(2, pos2, None), vec![]),
            RogerTestFrame::new(end_touch(3, pos3, None), vec![RogerEvent::Gesture(3)]),
        ]);

        test.eval(&mut roger);
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
                vec![move_touch(1, pos2, force)],
                vec![RogerEvent::ToolRun(ToolPayload { pos: pos2, force, id: Some(TouchId(1)) })],
            ),
            RogerTestFrame::new(
                vec![move_touch(1, pos3, force)],
                vec![RogerEvent::ToolRun(ToolPayload { pos: pos3, force, id: Some(TouchId(1)) })],
            ),
        ]);

        test.eval(&mut roger);
    }
}
