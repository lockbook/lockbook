use egui::PointerButton::{Primary, Secondary};
use egui::{Event, Pos2};
use std::ffi::{c_char, c_void, CStr};

use super::response::*;
use crate::apple::keyboard::NSKeys;
use crate::WgpuWorkspace;

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn macos_frame(obj: *mut c_void) -> MacOSResponse {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    obj.frame().into()
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn key_event(
    obj: *mut c_void, key_code: u16, shift: bool, ctrl: bool, option: bool, command: bool,
    pressed: bool, characters: *const c_char,
) {
    let obj = &mut *(obj as *mut WgpuWorkspace);

    let modifiers = egui::Modifiers { alt: option, ctrl, shift, mac_cmd: command, command };

    obj.raw_input.modifiers = modifiers;

    let Some(key) = NSKeys::from(key_code) else { return };

    // Event::Text
    let alt_only = modifiers.alt && !modifiers.ctrl && !modifiers.shift && !modifiers.mac_cmd  && !modifiers.command;
    let text_modifiers = modifiers.shift_only() || alt_only || modifiers.is_none();
    let text = CStr::from_ptr(characters).to_str().unwrap().to_string();
    let is_valid_text = text.chars().any(|c| !c.is_control());
    
    if pressed && text_modifiers && is_valid_text {
        obj.raw_input.events.push(Event::Text(text));
    } else if let Some(key) = key.egui_key() {
        obj.raw_input.events.push(Event::Key {
            key,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers,
        });
    }
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn modifier_event(
    obj: *mut c_void, shift: bool, ctrl: bool, option: bool, command: bool,
) {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    let modifiers = egui::Modifiers { alt: option, ctrl, shift, mac_cmd: command, command };
    obj.raw_input.modifiers = modifiers;
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn mouse_button(
    obj: *mut c_void, x: f32, y: f32, pressed: bool, primary: bool, shift: bool, ctrl: bool,
    option: bool, command: bool,
) {
    let modifiers = egui::Modifiers { alt: option, ctrl, shift, mac_cmd: command, command };

    let obj = &mut *(obj as *mut WgpuWorkspace);
    obj.raw_input.events.push(Event::PointerButton {
        pos: Pos2 { x, y },
        button: if primary { Primary } else { Secondary },
        pressed,
        modifiers,
    })
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn magnify_gesture(obj: *mut c_void, factor: f32) {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    let factor = factor.exp();

    obj.raw_input.events.push(Event::Zoom(factor))
}
