use crate::apple::keyboard::NSKeys;
use crate::WgpuWorkspace;
use egui::PointerButton::{Primary, Secondary};
use egui::{Event, Pos2};
use std::ffi::{c_char, c_void, CStr};

/// (macos only)
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

    let mut clip_event = false;
    if pressed && key == NSKeys::V && modifiers.command {
        let clip = obj.from_host.clone().unwrap_or_default();
        obj.raw_input.events.push(Event::Paste(clip));
        clip_event = true
    }

    // Event::Text
    if !clip_event && pressed && (modifiers.shift_only() || modifiers.is_none()) && key.valid_text()
    {
        let text = CStr::from_ptr(characters).to_str().unwrap().to_string();
        obj.raw_input.events.push(Event::Text(text));
    }

    // Event::Key
    if let Some(key) = key.egui_key() {
        obj.raw_input
            .events
            .push(Event::Key { key, pressed, repeat: false, modifiers });
    }
}

/// (macos only)
/// # Safety
#[no_mangle]
pub unsafe extern "C" fn modifier_event(
    obj: *mut c_void, shift: bool, ctrl: bool, option: bool, command: bool,
) {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    let modifiers = egui::Modifiers { alt: option, ctrl, shift, mac_cmd: command, command };
    obj.raw_input.modifiers = modifiers;
}

/// (macos only)
/// # Safety
#[no_mangle]
pub unsafe extern "C" fn mouse_moved(obj: *mut c_void, x: f32, y: f32) {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    obj.raw_input
        .events
        .push(Event::PointerMoved(Pos2 { x, y }))
}

/// (macos only)
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
