use lbeguiapp::WgpuLockbook;
use windows::Win32::UI::WindowsAndMessaging::*;

use super::{message::MessageAppDep, Point};

pub fn handle(
    app: &mut WgpuLockbook, message: MessageAppDep, pos: Point<u16>, modifiers: egui::Modifiers,
    dpi_scale: f32,
) -> bool {
    let pos = egui::Pos2 { x: pos.x as f32 / dpi_scale, y: pos.y as f32 / dpi_scale };

    if matches!(message, MessageAppDep::MouseMove { .. }) {
        app.raw_input.events.push(egui::Event::PointerMoved(pos));
    } else {
        let button = if matches!(
            message,
            MessageAppDep::LButtonDown { .. } | MessageAppDep::LButtonUp { .. }
        ) {
            egui::PointerButton::Primary
        } else {
            egui::PointerButton::Secondary
        };
        let pressed = matches!(
            message,
            MessageAppDep::LButtonDown { .. } | MessageAppDep::RButtonDown { .. }
        );
        pointer_button_event(pos, button, pressed, modifiers, app);
    }

    true
}

pub fn pointer_button_event(
    pos: egui::Pos2, button: egui::PointerButton, pressed: bool, modifiers: egui::Modifiers,
    app: &mut WgpuLockbook,
) {
    app.raw_input
        .events
        .push(egui::Event::PointerButton { pos, button, pressed, modifiers });
}

pub fn queue_pointer_button_event(
    pos: egui::Pos2, button: egui::PointerButton, pressed: bool, modifiers: egui::Modifiers,
    app: &mut WgpuLockbook,
) {
    app.queued_events
        .push(egui::Event::PointerButton { pos, button, pressed, modifiers });
}

pub fn handle_wheel(app: &mut WgpuLockbook, message: MessageAppDep, delta: i16) -> bool {
    if matches!(message, MessageAppDep::MouseWheel { .. }) {
        let y = delta as f32 / WHEEL_DELTA as f32;
        let y = y * 20.0; // arbitrary multiplier to make scrolling feel better
        app.raw_input
            .events
            .push(egui::Event::Scroll(egui::Vec2 { x: 0.0, y }));

        true
    } else {
        let x = -delta as f32 / WHEEL_DELTA as f32;
        let x = x * 20.0; // arbitrary multiplier to make scrolling feel better
        app.raw_input
            .events
            .push(egui::Event::Scroll(egui::Vec2 { x, y: 0.0 }));

        true
    }
}
