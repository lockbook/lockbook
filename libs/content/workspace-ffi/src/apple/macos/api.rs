use egui::Event;
use egui::PointerButton::{Primary, Secondary};
use std::ffi::{CStr, c_char, c_void};

use super::response::*;
use crate::WgpuWorkspace;
use crate::apple::keyboard::NSKeys;

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

    obj.renderer.raw_input.modifiers = modifiers;

    let Some(key) = NSKeys::from(key_code) else { return };

    let alt_without_ctrl_cmd = modifiers.alt && !modifiers.ctrl && !modifiers.command;
    let text_modifiers = modifiers.shift_only() || alt_without_ctrl_cmd || modifiers.is_none();
    let text = CStr::from_ptr(characters).to_str().unwrap();

    if pressed && text_modifiers && key.valid_text() {
        obj.renderer
            .raw_input
            .events
            .push(Event::Text(text.to_string()));
    }
    if let Some(key) = key.egui_key(shift) {
        obj.renderer.raw_input.events.push(Event::Key {
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
    obj.renderer.raw_input.modifiers = modifiers;
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn mouse_button(
    obj: *mut c_void, x: f32, y: f32, pressed: bool, primary: bool, shift: bool, ctrl: bool,
    option: bool, command: bool,
) {
    let modifiers = egui::Modifiers { alt: option, ctrl, shift, mac_cmd: command, command };

    let obj = &mut *(obj as *mut WgpuWorkspace);
    let pos = obj.renderer.pos_from_points(x, y);
    obj.renderer.raw_input.events.push(Event::PointerButton {
        pos,
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

    obj.renderer.raw_input.events.push(Event::Zoom(factor))
}

/// # Safety
/// obj must be a valid pointer to WgpuWorkspace
#[no_mangle]
pub unsafe extern "C" fn set_tab_strip_inset(obj: *mut c_void, inset: f32) {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    obj.workspace.tab_strip_left_inset = inset;
}

/// # Safety
/// obj must be a valid pointer to WgpuWorkspace
#[no_mangle]
pub unsafe extern "C" fn set_tab_strip_height(obj: *mut c_void, height: f32) {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    obj.workspace.tab_strip_min_height = height;
}

/// Configure the process-wide experimental MCP listener. The returned error
/// string must be released with free_text; null means success.
/// # Safety
/// Call on the main thread. token must point to a valid NUL-terminated UTF-8 string.
#[cfg(target_os = "macos")]
#[no_mangle]
pub unsafe extern "C" fn configure_mcp(
    enabled: bool, port: u16, token: *const c_char,
) -> *const c_char {
    let token = if token.is_null() { "" } else { CStr::from_ptr(token).to_str().unwrap_or("") };
    match workspace_rs::mcp::configure(enabled, port, token) {
        Ok(()) => std::ptr::null(),
        Err(error) => std::ffi::CString::new(error).unwrap().into_raw(),
    }
}

/// Drain MCP commands on the selected window's workspace, including while it
/// is not drawing. No workspace pointers are shared with the network thread.
/// # Safety
/// obj must be a live WgpuWorkspace owned by the calling main thread.
#[cfg(target_os = "macos")]
#[no_mangle]
pub unsafe extern "C" fn process_mcp(obj: *mut c_void) {
    if let Some(obj) = (obj as *mut WgpuWorkspace).as_mut() {
        obj.workspace.process_bg_tasks();
        obj.workspace.process_lb_updates();
        obj.workspace.process_task_updates();
        workspace_rs::mcp::process(&mut obj.workspace);
    }
}
