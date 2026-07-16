use crate::WgpuWorkspace;
use egui::{Color32, Event, MouseWheelUnit, vec2};
use lb_c::Uuid;
use lb_c::model::errors::Unexpected;
use std::ffi::{CStr, CString, c_char, c_void};
use std::path::PathBuf;
use workspace_rs::tab::{ClipContent, ExtendedInput as _};
use workspace_rs::theme::palette_v2::{Mode, ThemeExt, ensure_normal_text_contrast};

use super::response::*;

/// Edit-menu "Edit" over a selected image: select the URL inside the atom, revealing its
/// source (mobile has no arrow keys to get inside).
///
/// # Safety
#[no_mangle]
pub unsafe extern "C" fn enter_selected_atom(obj: *mut c_void) {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    obj.renderer
        .context
        .push_markdown_event(workspace_rs::tab::markdown_editor::input::Event::EnterAtom);
}

#[no_mangle]
pub extern "C" fn folder_selected(obj: *mut c_void, id: CUuid) {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };
    let id = id.into();

    obj.workspace.out.selected_folder_changed = true;
    obj.workspace.focused_parent = Some(id);
}

#[no_mangle]
pub extern "C" fn no_folder_selected(obj: *mut c_void) {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };
    obj.workspace.out.selected_folder_changed = true;
    obj.workspace.focused_parent = None;
}

#[no_mangle]
pub extern "C" fn get_selected_folder(obj: *mut c_void) -> CUuid {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };
    obj.workspace.focused_parent.unwrap_or_default().into()
}

#[no_mangle]
pub extern "C" fn open_file(obj: *mut c_void, id: CUuid, new_tab: bool) {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };
    let id = id.into();

    obj.workspace.open_file(id, true, new_tab)
}

#[no_mangle]
pub extern "C" fn open_file_at(
    obj: *mut c_void, id: CUuid, range_start: usize, range_end: usize, new_tab: bool,
) {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };
    let id = id.into();

    obj.workspace
        .open_file_at_range(id, range_start..range_end, new_tab);
}

#[no_mangle]
pub extern "C" fn nav_back(obj: *mut c_void) {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };
    if obj.workspace.can_back() {
        obj.workspace.back();
    }
}

#[no_mangle]
pub extern "C" fn nav_forward(obj: *mut c_void) {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };
    if obj.workspace.can_forward() {
        obj.workspace.forward();
    }
}

#[no_mangle]
pub extern "C" fn can_nav_back(obj: *mut c_void) -> bool {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };
    obj.workspace.can_back()
}

#[no_mangle]
pub extern "C" fn can_nav_forward(obj: *mut c_void) -> bool {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };
    obj.workspace.can_forward()
}

#[no_mangle]
pub extern "C" fn show_search(obj: *mut c_void) {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };
    obj.workspace.upsert_search(None);
}

#[no_mangle]
pub extern "C" fn show_find(obj: *mut c_void) {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };
    if let Some(md) = obj.workspace.current_tab_markdown_mut() {
        md.find.open_requested = true;
    }
}

#[no_mangle]
pub extern "C" fn create_doc_at(obj: *mut c_void, parent: CUuid, is_drawing: bool) {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };
    let parent = parent.into();

    obj.workspace.create_doc_at(is_drawing, parent);
}

// todo, should we deprecate this in favor of the one in lb? Better error handling.
#[no_mangle]
pub extern "C" fn request_sync(obj: *mut c_void) {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };
    obj.workspace.core.sync().log_and_ignore();
}

#[no_mangle]
pub extern "C" fn set_scale(obj: *mut c_void, scale: f32) {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };
    obj.renderer.set_native_pixels_per_point(scale);
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn set_contact_linked_sites(obj: *mut c_void, value: bool) {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    obj.workspace.cfg.set_contact_linked_sites(value);
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn set_sidebar_open(obj: *mut c_void, open: bool) {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    obj.workspace.sidebar_open = open;
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn dark_mode(obj: *mut c_void, dark: bool) {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    let mut theme = obj.renderer.context.get_lb_theme();
    if dark {
        theme.current = Mode::Dark;
    } else {
        theme.current = Mode::Light;
    }

    obj.renderer.context.set_lb_theme(theme);
}

/// # Safety
///
/// Accent colors packed as 0xRRGGBBAA, resolved for light and dark appearances.
#[no_mangle]
pub unsafe extern "C" fn set_accent_color(obj: *mut c_void, light: u32, dark: u32) {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    let unpack = |c: u32| Color32::from_rgb((c >> 24) as u8, (c >> 16) as u8, (c >> 8) as u8);

    let mut theme = obj.renderer.context.get_lb_theme();
    let light = ensure_normal_text_contrast(unpack(light), theme.bright.grey);
    let dark = ensure_normal_text_contrast(unpack(dark), theme.dim.grey);
    if theme.dim.blue == light && theme.bright.blue == dark {
        return;
    }

    theme.dim.blue = light;
    theme.bright.blue = dark;
    obj.renderer.context.set_lb_theme(theme);
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn scroll_wheel(
    obj: *mut c_void, scroll_x: f32, scroll_y: f32, shift: bool, ctrl: bool, option: bool,
    command: bool,
) {
    let obj = &mut *(obj as *mut WgpuWorkspace);

    let modifiers = egui::Modifiers { alt: option, ctrl, shift, mac_cmd: command, command };
    obj.renderer.raw_input.modifiers = modifiers;

    if obj.renderer.raw_input.modifiers.command || obj.renderer.raw_input.modifiers.ctrl {
        let factor = (scroll_y / 50.).exp();

        obj.renderer.raw_input.events.push(Event::Zoom(factor))
    } else {
        obj.renderer.raw_input.events.push(Event::MouseWheel {
            unit: MouseWheelUnit::Point,
            delta: vec2(scroll_x, scroll_y),
            modifiers: obj.renderer.raw_input.modifiers,
        });
    }
}

/// # Safety
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn clipboard_paste(obj: *mut c_void, content: *const c_char) {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    let content: String = CStr::from_ptr(content).to_str().unwrap().into();

    obj.renderer.raw_input.events.push(Event::Paste(content));
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
        obj.renderer
            .context
            .push_event(workspace_rs::Event::Paste { content, position });
    } else {
        obj.renderer
            .context
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
        obj.renderer
            .context
            .push_event(workspace_rs::Event::Paste { content, position });
    } else {
        obj.renderer
            .context
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

/// The URL of an openable link/image in the current selection, or null. The
/// returned string must be freed with `free_text`.
/// # Safety
#[no_mangle]
pub unsafe extern "C" fn selection_open_target(obj: *mut c_void) -> *const c_char {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    let Some(md) = obj.workspace.focused_mdedit_mut() else {
        return std::ptr::null();
    };
    match md.renderer.selection_open_target() {
        Some(url) => CString::new(url)
            .map(|s| s.into_raw() as *const c_char)
            .unwrap_or(std::ptr::null()),
        None => std::ptr::null(),
    }
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn open_selection_links(obj: *mut c_void) {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    if let Some(md) = obj.workspace.focused_mdedit_mut() {
        md.renderer.open_selection_links();
    }
}

/// Edit-menu "Refresh Preview": re-fetch preview metadata for links in the
/// current selection.
/// # Safety
#[no_mangle]
pub unsafe extern "C" fn refresh_selection_previews(obj: *mut c_void) {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    if let Some(md) = obj.workspace.focused_mdedit_mut() {
        md.renderer.refresh_selection_previews();
    }
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
    let pos = obj.renderer.pos_from_points(x, y);
    obj.renderer.raw_input.events.push(Event::PointerMoved(pos))
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn mouse_gone(obj: *mut c_void) {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    obj.renderer.raw_input.events.push(egui::Event::PointerGone);
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
        .tab_strip
        .iter()
        .position(|s| s.dest.id() == id)
    {
        obj.workspace.close_tab(tab_id);
    }
}
