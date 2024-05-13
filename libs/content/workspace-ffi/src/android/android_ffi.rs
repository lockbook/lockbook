use crate::android::window;
use crate::android::window::NativeWindow;
use crate::{wgpu, JRect, JTextPosition, JTextRange, WgpuWorkspace};
use egui::{
    Context, Event, FontDefinitions, PointerButton, Pos2, TouchDeviceId, TouchId, TouchPhase,
};
use egui_editor::input::canonical::{Location, Modification, Region};
use egui_editor::input::cursor::Cursor;
use egui_editor::input::mutation;
use egui_editor::offset_types::DocCharOffset;
use egui_wgpu_backend::wgpu::CompositeAlphaMode;
use egui_wgpu_backend::ScreenDescriptor;
use jni::objects::{JClass, JString};
use jni::sys::{jboolean, jfloat, jint, jlong, jobject, jstring};
use jni::JNIEnv;
use lb_external_interface::lb_rs::Uuid;
use lb_external_interface::Core;
use std::panic::catch_unwind;
use std::time::Instant;
use workspace_rs::register_fonts;
use workspace_rs::tab::svg_editor::Tool;
use workspace_rs::tab::EventManager;
use workspace_rs::tab::TabContent;
use workspace_rs::theme::visuals;
use workspace_rs::workspace::{Workspace, WsConfig};

use super::keyboard::AndroidKeys;

#[no_mangle]
pub unsafe extern "system" fn Java_app_lockbook_workspace_Workspace_initWS(
    env: JNIEnv, _: JClass, surface: jobject, core: jlong, scale_factor: jfloat, dark_mode: bool,
    old_wgpu: jlong,
) -> jlong {
    let core = unsafe { &mut *(core as *mut Core) };
    let writable_dir = core.get_config().unwrap().writeable_path;

    let mut native_window = NativeWindow::new(&env, surface);
    let backends = wgpu::Backends::VULKAN;
    let instance_desc = wgpu::InstanceDescriptor { backends, ..Default::default() };
    let instance = wgpu::Instance::new(instance_desc);
    let surface = instance
        .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::from_window(&mut native_window).unwrap())
        .unwrap();
    let (adapter, device, queue) = pollster::block_on(window::request_device(&instance, &surface));
    let format = surface.get_capabilities(&adapter).formats[0];
    let screen = ScreenDescriptor {
        physical_width: native_window.get_width(),
        physical_height: native_window.get_height(),
        scale_factor,
    };
    let config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format,
        width: screen.physical_width,
        height: screen.physical_height,
        present_mode: wgpu::PresentMode::Fifo,
        alpha_mode: CompositeAlphaMode::Auto,
        view_formats: vec![],
        desired_maximum_frame_latency: 2,
    };
    surface.configure(&device, &config);
    let rpass = egui_wgpu_backend::RenderPass::new(&device, format, 1);

    let context = Context::default();
    visuals::init(&context, dark_mode);
    let ws_cfg = WsConfig { data_dir: writable_dir, ..Default::default() };

    let workspace = if old_wgpu != jlong::MAX {
        let mut old_wgpu: Box<WgpuWorkspace> = unsafe { Box::from_raw(old_wgpu as *mut _) };

        old_wgpu
            .workspace
            .invalidate_egui_references(&context, core);
        old_wgpu.workspace
    } else {
        Workspace::new(ws_cfg, core, &context)
    };

    let mut fonts = FontDefinitions::default();
    register_fonts(&mut fonts);
    context.set_fonts(fonts);

    let start_time = Instant::now();
    let obj = WgpuWorkspace {
        start_time,
        device,
        queue,
        surface,
        adapter,
        rpass,
        screen,
        context: context.clone(),
        raw_input: Default::default(),
        workspace,
        surface_width: 0,
        surface_height: 0,
    };

    Box::into_raw(Box::new(obj)) as jlong
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_workspace_Workspace_enterFrame(
    env: JNIEnv, _: JClass, obj: jlong,
) -> jstring {
    let maybe_err = catch_unwind(|| {
        let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };

        serde_json::to_string(&obj.frame()).unwrap()
    });

    match maybe_err {
        Ok(ok) => env
            .new_string(ok)
            .expect("Couldn't create JString from rust string!")
            .into_raw(),
        Err(err) => {
            if let Some(err_str) = err.downcast_ref::<&str>() {
                env.new_string(*err_str)
                    .expect("Couldn't create JString from rust string!")
                    .into_raw()
            } else if let Some(err_string) = err.downcast_ref::<String>() {
                env.new_string(err_string.as_str())
                    .expect("Couldn't create JString from rust string!")
                    .into_raw()
            } else {
                env.new_string("Unknown error occurred")
                    .expect("Couldn't create JString from rust string!")
                    .into_raw()
            }
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_workspace_Workspace_resizeEditor(
    env: JNIEnv, _: JClass, obj: jlong, surface: jobject, scale_factor: jfloat,
) {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };
    let native_window = NativeWindow::new(&env, surface);

    obj.screen.physical_width = native_window.get_width();
    obj.screen.physical_height = native_window.get_height();
    obj.screen.scale_factor = scale_factor;
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_workspace_Workspace_sendKeyEvent(
    mut env: JNIEnv, _: JClass, obj: jlong, key_code: jint, content: JString, pressed: jboolean,
    alt: jboolean, ctrl: jboolean, shift: jboolean,
) {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };

    let modifiers = egui::Modifiers {
        alt: alt == 1,
        ctrl: ctrl == 1,
        shift: shift == 1,
        mac_cmd: false,
        command: false,
    };

    obj.raw_input.modifiers = modifiers;

    let Some(key) = AndroidKeys::from(key_code) else { return };

    if pressed == 1 && (modifiers.shift_only() || modifiers.is_none()) && key.valid_text() {
        let text: String = match env.get_string(&content) {
            Ok(cont) => cont.into(),
            Err(err) => format!("# The error is: {:?}", err),
        };

        obj.raw_input.events.push(Event::Text(text));
    }

    if let Some(key) = key.egui_key() {
        obj.raw_input.events.push(Event::Key {
            key,
            physical_key: None,
            pressed: pressed == 1,
            repeat: false,
            modifiers,
        });
    } else {
    }
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_workspace_Workspace_touchesBegin(
    _env: JNIEnv, _: JClass, ogbj: jlong, id: jint, x: jfloat, y: jfloat, pressure: jfloat,
) {
    let obj = unsafe { &mut *(ogbj as *mut WgpuWorkspace) };

    obj.raw_input.events.push(Event::Touch {
        device_id: TouchDeviceId(0),
        id: TouchId(id as u64),
        phase: TouchPhase::Start,
        pos: Pos2 { x, y },
        force: Some(pressure),
    });

    obj.raw_input.events.push(Event::PointerButton {
        pos: Pos2 { x, y },
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Default::default(),
    });
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_workspace_Workspace_touchesMoved(
    _env: JNIEnv, _: JClass, obj: jlong, id: jint, x: jfloat, y: jfloat, pressure: jfloat,
) {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };

    obj.raw_input.events.push(Event::Touch {
        device_id: TouchDeviceId(0),
        id: TouchId(id as u64),
        phase: TouchPhase::Move,
        pos: Pos2 { x, y },
        force: Some(pressure),
    });

    obj.raw_input
        .events
        .push(Event::PointerMoved(Pos2 { x, y }));
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_workspace_Workspace_touchesEnded(
    _env: JNIEnv, _: JClass, obj: jlong, id: jint, x: jfloat, y: jfloat, pressure: jfloat,
) {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };

    obj.raw_input.events.push(Event::Touch {
        device_id: TouchDeviceId(0),
        id: TouchId(id as u64),
        phase: TouchPhase::End,
        pos: Pos2 { x, y },
        force: Some(pressure),
    });

    obj.raw_input.events.push(Event::PointerButton {
        pos: Pos2 { x, y },
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Default::default(),
    });

    obj.raw_input.events.push(Event::PointerGone);
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_workspace_Workspace_touchesCancelled(
    _env: JNIEnv, _: JClass, obj: jlong, id: jint, x: jfloat, y: jfloat, pressure: jfloat,
) {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };

    obj.raw_input.events.push(Event::Touch {
        device_id: TouchDeviceId(0),
        id: TouchId(id as u64),
        phase: TouchPhase::Cancel,
        pos: Pos2 { x, y },
        force: Some(pressure),
    });

    obj.raw_input.events.push(Event::PointerGone);
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_workspace_Workspace_openDoc(
    mut env: JNIEnv, _: JClass, obj: jlong, jid: JString, new_file: jboolean,
) {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };

    let rid: String = env.get_string(&jid).unwrap().into();
    let id = Uuid::parse_str(&rid).unwrap();

    obj.workspace.open_file(id, new_file == 1, true);
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_workspace_Workspace_closeDoc(
    mut env: JNIEnv, _: JClass, obj: jlong, jid: JString,
) {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };

    let rid: String = env.get_string(&jid).unwrap().into();
    let id = Uuid::parse_str(&rid).unwrap();

    if let Some(tab_id) = obj.workspace.tabs.iter().position(|tab| tab.id == id) {
        obj.workspace.close_tab(tab_id);
    }
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_workspace_Workspace_showTabs(
    _env: JNIEnv, _: JClass, obj: jlong, show: jboolean,
) {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };

    obj.workspace.show_tabs = show == 1;
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_workspace_Workspace_requestSync(
    _env: JNIEnv, _: JClass, obj: jlong,
) {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };
    obj.workspace.perform_sync();
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_workspace_Workspace_currentTab(
    _env: JNIEnv, _: JClass, obj: jlong,
) -> jint {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };

    match obj.workspace.current_tab() {
        Some(tab) => match &tab.content {
            Some(tab) => match tab {
                TabContent::Image(_) => 2,
                TabContent::Markdown(_) => 3,
                TabContent::PlainText(_) => 4,
                TabContent::Pdf(_) => 5,
                TabContent::Svg(_) => 6,
            },
            None => 1,
        },
        None => 0,
    }
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_workspace_Workspace_fileRenamed(
    mut env: JNIEnv, _: JClass, obj: jlong, jid: JString, jnew_name: JString,
) {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };

    let rid: String = env.get_string(&jid).unwrap().into();
    let id = Uuid::parse_str(&rid).unwrap();
    let new_name: String = env.get_string(&jnew_name).unwrap().into();

    let _ = obj
        .workspace
        .updates_tx
        .send(workspace_rs::workspace::WsMsg::FileRenamed { id, new_name });
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_workspace_Workspace_unfocusTitle(
    _env: JNIEnv, _: JClass, obj: jlong,
) {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };

    if let Some(tab) = obj.workspace.current_tab_mut() {
        tab.rename = None;
    }
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_workspace_Workspace_getAllText(
    env: JNIEnv, _: JClass, obj: jlong,
) -> jstring {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };

    let markdown = match obj.workspace.current_tab_markdown_mut() {
        Some(markdown) => markdown,
        None => {
            return env
                .new_string("")
                .expect("Couldn't create JString from rust string!")
                .into_raw()
        }
    };

    env.new_string(&markdown.editor.buffer.current.text)
        .expect("Couldn't create JString from rust string!")
        .into_raw()
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_workspace_Workspace_getSelection(
    env: JNIEnv, _: JClass, obj: jlong,
) -> jstring {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };

    let resp = match obj.workspace.current_tab_markdown_mut() {
        Some(markdown) => {
            let (start, end) = markdown.editor.buffer.current.cursor.selection;
            JTextRange { none: false, start: start.0, end: end.0 }
        }
        None => JTextRange { none: true, start: 0, end: 0 },
    };

    env.new_string(serde_json::to_string(&resp).unwrap())
        .expect("Couldn't create JString from rust string!")
        .into_raw()
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_workspace_Workspace_setSelection(
    _env: JNIEnv, _: JClass, obj: jlong, start: jint, end: jint,
) {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };

    obj.context.push_markdown_event(Modification::Select {
        region: Region::BetweenLocations {
            start: Location::DocCharOffset(DocCharOffset(start as usize)),
            end: Location::DocCharOffset(DocCharOffset(end as usize)),
        },
    });
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_workspace_Workspace_getTextLength(
    _env: JNIEnv, _: JClass, obj: jlong,
) -> jint {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };

    let markdown = match obj.workspace.current_tab_markdown_mut() {
        Some(markdown) => markdown,
        None => return -1,
    };

    return markdown.editor.buffer.current.segs.last_cursor_position().0 as jint;
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_workspace_Workspace_clear(
    _env: JNIEnv, _: JClass, obj: jlong,
) {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };

    let markdown = match obj.workspace.current_tab_markdown_mut() {
        Some(markdown) => markdown,
        None => return,
    };

    obj.context.push_markdown_event(Modification::Replace {
        region: Region::BetweenLocations {
            start: Location::DocCharOffset(DocCharOffset(0)),
            end: Location::DocCharOffset(
                markdown.editor.buffer.current.segs.last_cursor_position(),
            ),
        },
        text: "".to_string(),
    })
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_workspace_Workspace_replace(
    mut env: JNIEnv, _: JClass, obj: jlong, start: jint, end: jint, text: JString,
) {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };

    let text: String = match env.get_string(&text) {
        Ok(cont) => cont.into(),
        Err(err) => format!("error: {:?}", err),
    };

    obj.context.push_markdown_event(Modification::Replace {
        region: Region::BetweenLocations {
            start: Location::DocCharOffset(DocCharOffset(start as usize)),
            end: Location::DocCharOffset(DocCharOffset(end as usize)),
        },
        text,
    })
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_workspace_Workspace_insert(
    mut env: JNIEnv, _: JClass, obj: jlong, index: jint, text: JString,
) {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };

    let text: String = match env.get_string(&text) {
        Ok(cont) => cont.into(),
        Err(err) => format!("error: {:?}", err),
    };

    let loc = Location::DocCharOffset(DocCharOffset(index as usize));

    obj.context.push_markdown_event(Modification::Replace {
        region: Region::BetweenLocations { start: loc, end: loc },
        text,
    })
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_workspace_Workspace_append(
    mut env: JNIEnv, _: JClass, obj: jlong, text: JString,
) {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };

    let markdown = match obj.workspace.current_tab_markdown_mut() {
        Some(markdown) => markdown,
        None => return,
    };

    let text: String = match env.get_string(&text) {
        Ok(cont) => cont.into(),
        Err(err) => format!("error: {:?}", err),
    };

    let loc = Location::DocCharOffset(markdown.editor.buffer.current.segs.last_cursor_position());

    obj.context.push_markdown_event(Modification::Replace {
        region: Region::BetweenLocations { start: loc, end: loc },
        text,
    })
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_workspace_Workspace_getTextInRange(
    env: JNIEnv, _: JClass, obj: jlong, start: jint, end: jint,
) -> jstring {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };

    let markdown = match obj.workspace.current_tab_markdown_mut() {
        Some(markdown) => markdown,
        None => {
            return env
                .new_string("")
                .expect("Couldn't create JString from rust string!")
                .into_raw()
        }
    };

    let cursor: Cursor = (start as usize, end as usize).into();

    let buffer = &markdown.editor.buffer.current;
    let text = cursor.selection_text(buffer);

    env.new_string(text)
        .expect("Couldn't create JString from rust string!")
        .into_raw()
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_workspace_Workspace_selectAll(
    _env: JNIEnv, _: JClass, obj: jlong,
) {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };

    let markdown = match obj.workspace.current_tab_markdown_mut() {
        Some(markdown) => markdown,
        None => return,
    };

    let buffer = &markdown.editor.buffer.current;

    obj.context.push_markdown_event(Modification::Select {
        region: Region::BetweenLocations {
            start: Location::DocCharOffset(DocCharOffset(0)),
            end: Location::DocCharOffset(buffer.segs.last_cursor_position()),
        },
    });
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_workspace_Workspace_clipboardCut(
    _env: JNIEnv, _: JClass, obj: jlong,
) {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };
    obj.context.push_markdown_event(Modification::Cut);
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_workspace_Workspace_clipboardCopy(
    _env: JNIEnv, _: JClass, obj: jlong,
) {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };
    obj.context.push_markdown_event(Modification::Copy);
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_workspace_Workspace_clipboardPaste(
    mut env: JNIEnv, _: JClass, obj: jlong, content: JString,
) {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };

    let content: String = match env.get_string(&content) {
        Ok(cont) => cont.into(),
        Err(err) => format!("# The error is: {:?}", err),
    };

    obj.raw_input.events.push(Event::Paste(content));
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_workspace_Workspace_toggleEraserSVG(
    _env: JNIEnv, _: JClass, obj: jlong, select: jboolean,
) {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };

    if let Some(svg) = obj.workspace.current_tab_svg_mut() {
        if select == 1 {
            svg.toolbar.set_tool(Tool::Eraser);
        } else if svg.toolbar.active_tool == Tool::Eraser {
            svg.toolbar
                .set_tool(svg.toolbar.previous_tool.unwrap_or(Tool::Pen));
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_workspace_Workspace_textOffsetForPosition(
    env: JNIEnv, _: JClass, obj: jlong, x: jfloat, y: jfloat,
) -> jstring {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };

    let markdown = match obj.workspace.current_tab_markdown_mut() {
        Some(markdown) => markdown,
        None => {
            return env
                .new_string("")
                .expect("Couldn't create JString from rust string!")
                .into_raw()
        }
    };

    let segs = &markdown.editor.buffer.current.segs;
    let galleys = &markdown.editor.galleys;
    let text = &markdown.editor.bounds.text;

    let offset = mutation::pos_to_char_offset(Pos2 { x, y }, galleys, segs, text);

    let position = JTextPosition { none: false, position: offset.0 };

    env.new_string(serde_json::to_string(&position).unwrap())
        .expect("Couldn't create JString from rust string!")
        .into_raw()
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_workspace_Workspace_getComposing(
    env: JNIEnv, _: JClass, obj: jlong,
) -> jstring {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };

    let resp = match obj.workspace.current_tab_markdown_mut() {
        Some(markdown) => match markdown.editor.buffer.current.cursor.mark {
            None => JTextRange { none: true, ..Default::default() },
            Some((start, end)) => JTextRange { none: false, start: start.0, end: end.0 },
        },
        None => JTextRange::default(),
    };

    env.new_string(serde_json::to_string(&resp).unwrap())
        .expect("Couldn't create JString from rust string!")
        .into_raw()
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_workspace_Workspace_setComposing(
    mut env: JNIEnv, _: JClass, obj: jlong, none: jboolean, start: jint, end: jint, text: JString,
) {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };
    let text: String = match env.get_string(&text) {
        Ok(cont) => cont.into(),
        Err(err) => format!("error: {:?}", err),
    };

    obj.context.push_markdown_event(Modification::StageMarked {
        highlighted: JTextRange { none: none == 1, start: start as usize, end: end as usize }
            .into(),
        text,
    });
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_workspace_Workspace_uncomposeText(
    _env: JNIEnv, _: JClass, obj: jlong,
) {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };
    obj.context.push_markdown_event(Modification::CommitMarked);
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_workspace_Workspace_getCursorRect(
    env: JNIEnv, _: JClass, obj: jlong,
) -> jstring {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };

    let markdown = match obj.workspace.current_tab_markdown_mut() {
        Some(markdown) => markdown,
        None => {
            return env
                .new_string("")
                .expect("Couldn't create JString from rust string!")
                .into_raw()
        }
    };

    let galleys = &markdown.editor.galleys;
    let text = &markdown.editor.bounds.text;
    let appearance = &markdown.editor.appearance;

    let line = markdown
        .editor
        .buffer
        .current
        .cursor
        .start_line(galleys, text, appearance);

    let jrect = JRect { min_x: line[0].x, min_y: line[0].y, max_x: line[1].x, max_y: line[1].y };

    env.new_string(serde_json::to_string(&jrect).unwrap())
        .expect("Couldn't create JString from rust string!")
        .into_raw()
}
