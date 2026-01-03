use egui::MouseWheelUnit;
use lbeguiapp::WgpuLockbook;
use windows::Win32::UI::WindowsAndMessaging::*;

use super::message::{MessageAppDep, Point};

pub fn handle(
    app: &mut WgpuLockbook, message: MessageAppDep, pos: Point<u16>, modifiers: egui::Modifiers,
    dpi_scale: f32,
) -> bool {
    let pos = egui::Pos2 { x: pos.x as f32 / dpi_scale, y: pos.y as f32 / dpi_scale };

    if matches!(message, MessageAppDep::MouseMove { .. }) {
        app.renderer
            .raw_input
            .events
            .push(egui::Event::PointerMoved(pos));
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
        app.renderer
            .raw_input
            .events
            .push(egui::Event::PointerButton { pos, button, pressed, modifiers });
    }

    true
}

pub fn handle_wheel(
    app: &mut WgpuLockbook, message: MessageAppDep, delta: i16, modifiers: egui::Modifiers,
) -> bool {
    if modifiers.command {
        let resistance = 500.0;
        let factor = (delta as f32 / resistance).exp();
        app.renderer
            .raw_input
            .events
            .push(egui::Event::Zoom(factor));
    } else {
        let scroll_magnitude = 20.0 * delta as f32 / WHEEL_DELTA as f32;
        let delta = if matches!(message, MessageAppDep::MouseWheel { .. }) {
            egui::Vec2 { x: 0.0, y: scroll_magnitude }
        } else {
            egui::Vec2 { x: -scroll_magnitude, y: 0.0 }
        };

        app.renderer.raw_input.events.push(egui::Event::MouseWheel {
            unit: MouseWheelUnit::Point,
            delta,
            modifiers: app.renderer.raw_input.modifiers,
        });
    }
    true
}
