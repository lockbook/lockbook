use crate::WgpuWorkspace;
use egui::{Event, MouseWheelUnit, Pos2, vec2};
use lb_c::Uuid;
use std::ffi::{CStr, CString, c_char, c_void};
use std::path::PathBuf;
use workspace_rs::tab::{ClipContent, ExtendedInput as _};
use workspace_rs::theme::visuals;

use super::response::*;

#[no_mangle]
pub extern "C" fn folder_selected(obj: *mut c_void, id: CUuid) {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };
    let id = id.into();

    obj.workspace.focused_parent = Some(id);
}

#[no_mangle]
pub extern "C" fn no_folder_selected(obj: *mut c_void) {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };
    obj.workspace.focused_parent = None;
}

#[no_mangle]
pub extern "C" fn open_file(obj: *mut c_void, id: CUuid, new_file: bool) {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };
    let id = id.into();

    obj.workspace.open_file(id, new_file, true, true)
}

#[no_mangle]
pub extern "C" fn request_sync(obj: *mut c_void) {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };
    obj.workspace.tasks.queue_sync();
}

#[no_mangle]
pub extern "C" fn set_scale(obj: *mut c_void, scale: f32) {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };
    obj.screen.scale_factor = scale;
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn dark_mode(obj: *mut c_void, dark: bool) {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    visuals::init(&obj.context, dark);
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn scroll_wheel(
    obj: *mut c_void, scroll_x: f32, scroll_y: f32, shift: bool, ctrl: bool, option: bool,
    command: bool,
) {
    let obj = &mut *(obj as *mut WgpuWorkspace);

    let modifiers = egui::Modifiers { alt: option, ctrl, shift, mac_cmd: command, command };
    obj.raw_input.modifiers = modifiers;

    if obj.raw_input.modifiers.command || obj.raw_input.modifiers.ctrl {
        let factor = (scroll_y / 50.).exp();

        obj.raw_input.events.push(Event::Zoom(factor))
    } else {
        obj.raw_input.events.push(Event::MouseWheel {
            unit: MouseWheelUnit::Point,
            delta: vec2(scroll_x, scroll_y),
            modifiers: obj.raw_input.modifiers,
        });
    }
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn clipboard_paste(obj: *mut c_void, content: *const c_char) {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    let content: String = CStr::from_ptr(content).to_str().unwrap().into();

    obj.raw_input.events.push(Event::Paste(content));
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn clipboard_send_image(
    obj: *mut c_void, content: *const u8, length: usize, is_paste: bool,
) {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    let img = std::slice::from_raw_parts(content, length).to_vec();
    let content = vec![ClipContent::Image(img)];
    let position = egui::Pos2::ZERO; // todo: cursor position

    if is_paste {
        obj.context
            .push_event(workspace_rs::Event::Paste { content, position });
    } else {
        obj.context
            .push_event(workspace_rs::Event::Drop { content, position });
    }
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn clipboard_send_file(
    obj: *mut c_void, file_url: *const c_char, is_paste: bool,
) {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    let file_url: String = CStr::from_ptr(file_url).to_str().unwrap().into();
    let content = vec![ClipContent::Files(vec![PathBuf::from(file_url)])];
    let position = egui::Pos2::ZERO; // todo: cursor position

    if is_paste {
        obj.context
            .push_event(workspace_rs::Event::Paste { content, position });
    } else {
        obj.context
            .push_event(workspace_rs::Event::Drop { content, position });
    }
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn free_text(s: *const c_char) {
    if s.is_null() {
        return;
    }
    drop(CString::from_raw(s as *mut c_char));
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn deinit_editor(obj: *mut c_void) {
    let _ = Box::from_raw(obj as *mut WgpuWorkspace);
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn mouse_moved(obj: *mut c_void, x: f32, y: f32) {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    obj.raw_input
        .events
        .push(Event::PointerMoved(Pos2 { x, y }))
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn mouse_gone(obj: *mut c_void) {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    obj.raw_input.events.push(egui::Event::PointerGone);
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn tab_renamed(obj: *mut c_void, id: *const c_char, new_name: *const c_char) {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    let new_name: String = CStr::from_ptr(new_name).to_str().unwrap().into();

    let id: Uuid = CStr::from_ptr(id)
        .to_str()
        .expect("Could not C String -> Rust String")
        .to_string()
        .parse()
        .expect("Could not String -> Uuid");

    obj.workspace.file_renamed(id, new_name);
}

// todo: can't close non-file tabs (mind map)
/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn close_tab(obj: *mut c_void, id: *const c_char) {
    let obj = &mut *(obj as *mut WgpuWorkspace);

    let id: Uuid = CStr::from_ptr(id)
        .to_str()
        .expect("Could not C String -> Rust String")
        .to_string()
        .parse()
        .expect("Could not String -> Uuid");

    if let Some(tab_id) = obj
        .workspace
        .tabs
        .iter()
        .position(|tab| tab.id() == Some(id))
    {
        obj.workspace.close_tab(tab_id);
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct FfiWsStatus {
    pub syncing: bool,
    pub msg: *const c_char,
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn get_status(obj: *mut c_void) -> FfiWsStatus {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    let syncing = obj.workspace.visibly_syncing();
    let msg = obj.workspace.status.message.clone();
    let msg = CString::new(msg)
        .expect("Could not Rust String -> C String")
        .into_raw();

    FfiWsStatus { syncing, msg }
}
