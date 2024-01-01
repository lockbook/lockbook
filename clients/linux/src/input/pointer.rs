use lbeguiapp::WgpuLockbook;
use x11rb::protocol::xproto::{ButtonPressEvent, KeyButMask, MotionNotifyEvent};

pub fn handle_press(app: &mut WgpuLockbook, event: ButtonPressEvent) {
    handle(app, event.event_x, event.event_y, event.detail, event.state, true)
}

pub fn handle_release(app: &mut WgpuLockbook, event: ButtonPressEvent) {
    handle(app, event.event_x, event.event_y, event.detail, event.state, false)
}

fn handle(
    app: &mut WgpuLockbook, event_x: i16, event_y: i16, detail: u8, state: KeyButMask,
    pressed: bool,
) {
    let pos = egui::Pos2::new(event_x as f32, event_y as f32);
    let button = if detail == 1 {
        egui::PointerButton::Primary
    } else if detail == 3 {
        egui::PointerButton::Secondary
    } else {
        return;
    };
    let modifiers = egui::Modifiers {
        alt: state.contains(KeyButMask::MOD1),
        ctrl: state.contains(KeyButMask::CONTROL),
        command: state.contains(KeyButMask::CONTROL),
        shift: state.contains(KeyButMask::SHIFT),
        mac_cmd: false,
    };
    app.raw_input
        .events
        .push(egui::Event::PointerButton { pos, button, pressed, modifiers })
}

pub fn handle_motion(app: &mut WgpuLockbook, event: MotionNotifyEvent) {
    let pos = egui::Pos2::new(event.event_x as f32, event.event_y as f32);
    app.raw_input.events.push(egui::Event::PointerMoved(pos));
}
