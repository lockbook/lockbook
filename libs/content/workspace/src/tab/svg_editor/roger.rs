use std::{
    collections::{HashMap, HashSet},
    slice::Iter,
};

use tracing::warn;
use web_time::Instant;

#[derive(Debug)]
struct Roger {
    touches: HashMap<Pointer, Instant>,
    buttons: HashMap<MouseProps, Instant>,
    tool_running: bool,
}

#[derive(Eq, Hash, PartialEq, Debug)]
enum Pointer {
    Mouse(MouseProps),
    Finger(u64), // Touch ID
    Pen(u64),    // Pen ID
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
    pub fn new() -> Self {
        Self { touches: HashMap::new(), buttons: HashMap::new(), tool_running: false }
    }

    pub fn process(&mut self, ui: &mut egui::Ui) {
        ui.input(|r| self.process_events(r.events.iter()));
    }

    fn process_events(&mut self, events: Iter<egui::Event>) {
        let run_button =
            &MouseProps { button: ButtonType::Primary, modifiers: egui::Modifiers::NONE };
        for event in events {
            match *event {
                egui::Event::PointerButton { pos, button, pressed, modifiers } => {
                    let button = MouseProps { button: button.into(), modifiers };
                    if pressed {
                        self.buttons.insert(button, Instant::now());
                    } else {
                        let exists = self.buttons.remove(&button).is_some();
                        if !exists {
                            warn!(
                                "Mouse Button {:?} at position {:?} released without being pressed",
                                button, pos
                            );
                        }
                        if button == *run_button {
                            self.tool_running = false;
                        }
                    }
                }
                egui::Event::PointerMoved(pos) => {
                    // we know know if there are any self.buttons pressed. if there are none, then this is a hover event.
                    // if there is something pressed, then this is a stroke event
                    if self.buttons.contains_key(run_button) {
                        self.tool_running = true;
                    }
                    // todo: tool can specify behavior when the pointer moves outside of the canvas
                    // do nothing or end. for selection do nothing makes sense, we still wanna drag things
                    // for pen tool, you wanna end.
                }
                egui::Event::PointerGone => {
                    println!("Pointer gone");
                }
                egui::Event::MouseWheel { unit, delta, modifiers }{
                    // when did we aquire the tool run lock. if it's less than 100ms ago, then we can assume 
                    // this is a pan and not a tool run 
                }
                egui::Event::Touch { device_id, id, phase, pos, force } => {
                    let touch =
                        if force.is_some() { Pointer::Pen(id.0) } else { Pointer::Finger(id.0) };

                    match phase {
                        egui::TouchPhase::Start => {
                            self.touches.insert(touch, Instant::now());
                        }
                        egui::TouchPhase::End | egui::TouchPhase::Cancel => {
                            self.touches.remove(&touch);
                            let exists = self.touches.remove(&touch).is_some();
                            if !exists {
                                warn!(
                                    "Touch with id {:?} from device {:?} ended without starting",
                                    id, device_id
                                );
                            }
                        }
                        egui::TouchPhase::Move => {
                            // we know this touch is active because it has to have started, and not yet ended or cancelled
                        }
                    }

                    println!(
                        "Touch event from device {:?} with id {:?} in phase {:?} at position {:?} with force {:?}",
                        device_id, id, phase, pos, force
                    );
                }
                egui::Event::Zoom(factor) => {
                    println!("Zoom event with factor: {:?}", factor);
                }
                _ => {}
            }
        }
    }
}

#[test]
fn test_roger() {
    let mut roger = Roger::new();
    let events = vec![
        egui::Event::PointerButton {
            pos: egui::pos2(100.0, 200.0),
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        },
        egui::Event::PointerMoved(egui::pos2(150.0, 250.0)),
    ];

    roger.process_events(events.iter());
    println!("Roger state: {:?}", roger);

    let events = vec![egui::Event::PointerMoved(egui::pos2(155.0, 256.0))];

    roger.process_events(events.iter());
    println!("Roger state: {:?}", roger);

    let events = vec![
        egui::Event::PointerButton {
            pos: egui::pos2(100.0, 200.0),
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        },
        egui::Event::PointerMoved(egui::pos2(150.0, 250.0)),
    ];

    roger.process_events(events.iter());
    println!("Roger state: {:?}", roger);
}
