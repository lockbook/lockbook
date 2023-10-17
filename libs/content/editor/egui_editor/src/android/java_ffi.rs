use std::cmp::min;
use crate::{CompositeAlphaMode, Editor, Pos2, wgpu, WgpuEditor};
use crate::android::window::NativeWindow;
use egui::{Context, Event, PointerButton, TouchDeviceId, TouchId, TouchPhase, Visuals};
use egui_wgpu_backend::ScreenDescriptor;
use jni::objects::{JClass, JString};
use jni::sys::{jboolean, jfloat, jint, jlong, jobject, jstring};
use jni::JNIEnv;
use std::time::Instant;
use crate::android::keyboard::AndroidKeys;
use crate::android::window;
use crate::input::canonical::{Location, Modification, Region};
use crate::input::cursor::Cursor;
use crate::offset_types::{DocCharOffset, RangeExt};

#[no_mangle]
pub extern "system" fn Java_app_lockbook_egui_1editor_EGUIEditor_createWgpuCanvas(
    mut env: JNIEnv, _: JClass, surface: jobject, core: jlong, content: JString, scale_factor: jfloat, dark_mode: bool
) -> jlong {
    let core = unsafe { &mut *(core as *mut lb::Core) };

    let native_window = NativeWindow::new(&env, surface);
    let backends = wgpu::Backends::VULKAN;
    let instance_desc = wgpu::InstanceDescriptor { backends, ..Default::default() };
    let instance = wgpu::Instance::new(instance_desc);
    let surface = unsafe { instance.create_surface(&native_window).unwrap() };
    let (adapter, device, queue) =
        pollster::block_on(window::request_device(&instance, backends, &surface));
    let format = surface.get_capabilities(&adapter).formats[0];
    let config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format,
        width: native_window.get_width(),
        height: native_window.get_height(),
        present_mode: wgpu::PresentMode::Fifo,
        alpha_mode: CompositeAlphaMode::Auto,
        view_formats: vec![],
    };
    surface.configure(&device, &config);
    let rpass = egui_wgpu_backend::RenderPass::new(&device, format, 1);

    let context = Context::default();
    context.set_visuals(if dark_mode { Visuals::dark() } else { Visuals::light() });
    let mut editor = Editor::new(core.clone());
    editor.set_font(&context);

    let content: String = match env
        .get_string(&content) {
        Ok(cont) => cont.into(),
        Err(err) => format!("# The error is: {:?}", err)
    };
    editor.buffer = content.as_str().into();

    let start_time = Instant::now();
    let mut obj = WgpuEditor {
        start_time,
        device,
        queue,
        surface,
        adapter,
        rpass,
        screen: ScreenDescriptor {
            physical_width: native_window.get_width(),
            physical_height: native_window.get_height(),
            scale_factor,
        },
        context,
        raw_input: Default::default(),
        from_egui: None,
        from_host: None,
        editor,
    };

    obj.frame();

    Box::into_raw(Box::new(obj)) as jlong
}

// optimizationcan create class through jni
#[no_mangle]
pub extern "system" fn Java_app_lockbook_egui_1editor_EGUIEditor_enterFrame(
    env: JNIEnv, _: JClass, obj: jlong,
) -> jstring {
    let obj = unsafe { &mut *(obj as *mut WgpuEditor) };

    env
        .new_string(serde_json::to_string(&obj.frame()).unwrap())
        .expect("Couldn't create JString from rust string!")
        .into_raw()
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_egui_1editor_EGUIEditor_resizeEditor(
    env: JNIEnv, _: JClass, obj: jlong, surface: jobject, scale_factor: jfloat
) {
    let obj = unsafe { &mut *(obj as *mut WgpuEditor) };
    let native_window = NativeWindow::new(&env, surface);

    obj.screen.physical_width = native_window.get_width();
    obj.screen.physical_height = native_window.get_height();
    obj.screen.scale_factor = scale_factor;
}

// #[no_mangle]
// pub extern "system" fn Java_app_lockbook_egui_1editor_EGUIEditor_setText(
//     mut env: JNIEnv, _: JClass, obj: jlong, content: JString,
// ) {
//     let obj = unsafe { &mut *(obj as *mut WgpuEditor) };
//
//     let content: String = match env.get_string(&content) {
//         Ok(cont) => cont.into(),
//         Err(err) => format!("# The error is: {:?}", err)
//     };
//     obj.editor.buffer = content.as_str().into();
//     obj.frame();
// }

#[no_mangle]
pub extern "system" fn Java_app_lockbook_egui_1editor_EGUIEditor_addText(
    mut env: JNIEnv, _: JClass, obj: jlong, content: JString,
) {
    let obj = unsafe { &mut *(obj as *mut WgpuEditor) };

    let content: String = match env.get_string(&content) {
        Ok(cont) => cont.into(),
        Err(err) => format!("# The error is: {:?}", err)
    };

    obj.raw_input.events.push(Event::Text(content));
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_egui_1editor_EGUIEditor_dropWgpuCanvas(
    mut _env: JNIEnv, _: JClass, obj: jlong,
) {
    let _obj: Box<WgpuEditor> = unsafe { Box::from_raw(obj as *mut _) };
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_egui_1editor_EGUIEditor_getTextBeforeCursor(
    env: JNIEnv, _: JClass, obj: jlong, n: jint,
) -> jstring {
    let obj = unsafe { &mut *(obj as *mut WgpuEditor) };

    let cursor: Cursor = (
        obj.editor.buffer.current.cursor.selection.start() - (n as usize),
        obj.editor.buffer.current.cursor.selection.end()
    )
        .into();

    let buffer = &obj.editor.buffer.current;
    let text = cursor.selection_text(buffer);

    env
        .new_string(text)
        .expect("Couldn't create JString from rust string!")
        .into_raw()
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_egui_1editor_EGUIEditor_getTextAfterCursor(
    env: JNIEnv, _: JClass, obj: jlong, n: jint
) -> jstring {
    let obj = unsafe { &mut *(obj as *mut WgpuEditor) };

    let buffer = &obj.editor.buffer.current;

    let cursor: Cursor = (
        obj.editor.buffer.current.cursor.selection.start(),
        DocCharOffset(min(obj.editor.buffer.current.cursor.selection.end().0.saturating_add(n as usize), buffer.segs.last_cursor_position().0))
    )
        .into();

    let text = cursor.selection_text(buffer);

    env
        .new_string(text)
        .expect("Couldn't create JString from rust string!")
        .into_raw()
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_egui_1editor_EGUIEditor_getSelectedText(
    env: JNIEnv, _: JClass, obj: jlong
) -> jstring {
    let obj = unsafe { &mut *(obj as *mut WgpuEditor) };

    let cursor = &obj.editor.buffer.current.cursor;
    let selected_text = String::from(cursor.selection_text(&obj.editor.buffer.current));

    env
        .new_string(selected_text)
        .expect("Couldn't create JString from rust string!")
        .into_raw()
}

// #[no_mangle]
// pub extern "system" fn Java_app_lockbook_egui_1editor_EGUIEditor_getCursorCapsMode(
//     env: JNIEnv, _: JClass, obj: jlong, regModes: jint
// ) -> jstring {
//     let obj = unsafe { &mut *(obj as *mut WgpuEditor) };
//
//     let cursor: Cursor = (
//         obj.editor.buffer.current.cursor.selection.start(),
//         obj.editor.buffer.current.cursor.selection.end() + (n as usize)
//     )
//         .into();
//
//     let buffer = &obj.editor.buffer.current;
//     let text = cursor.selection_text(buffer);
//
//     env
//         .new_string(text)
//         .expect("Couldn't create JString from rust string!")
//         .into_raw()
// }

#[no_mangle]
pub extern "system" fn Java_app_lockbook_egui_1editor_EGUIEditor_deleteSurroundingText(
    _env: JNIEnv, _: JClass, obj: jlong, before_length: jint, after_length: jint
) {
    let obj = unsafe { &mut *(obj as *mut WgpuEditor) };

    let current_cursor = &obj.editor.buffer.current.cursor;

    obj.editor.custom_events.push(Modification::Replace {
        region: Region::BetweenLocations {
            start: Location::DocCharOffset(current_cursor.selection.start() + (before_length as usize)),
            end: Location::DocCharOffset(current_cursor.selection.start())
        },
        text: "".to_string()
    });

    obj.editor.custom_events.push(Modification::Replace {
        region: Region::BetweenLocations {
            start: Location::DocCharOffset(current_cursor.selection.end() + (after_length as usize)),
            end: Location::DocCharOffset(current_cursor.selection.end())
        },
        text: "".to_string()
    });
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_egui_1editor_EGUIEditor_getAllText(
    env: JNIEnv, _: JClass, obj: jlong
) -> jstring {
    let obj = unsafe { &mut *(obj as *mut WgpuEditor) };

    env
        .new_string(&obj.editor.buffer.current.text)
        .expect("Couldn't create JString from rust string!")
        .into_raw()
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_egui_1editor_EGUIEditor_getSelection(
    env: JNIEnv, _: JClass, obj: jlong
) -> jstring {
    let obj = unsafe { &mut *(obj as *mut WgpuEditor) };

    let mut selection_text = "".to_string();

    if let Some(selection) = obj.editor.buffer.current.cursor.selection() {
        selection_text = format!("{} {}", selection.start.0, selection.end.0);
    }

    env
        .new_string(selection_text)
        .expect("Couldn't create JString from rust string!")
        .into_raw()
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_egui_1editor_EGUIEditor_setSelection(
    _env: JNIEnv, _: JClass, obj: jlong, start: jint, end: jint
) {
    let obj = unsafe { &mut *(obj as *mut WgpuEditor) };

    obj.editor.custom_events.push(Modification::Select {
        region: Region::BetweenLocations {
            start: Location::DocCharOffset(DocCharOffset(start as usize)),
            end: Location::DocCharOffset(DocCharOffset(end as usize))
        }
    });
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_egui_1editor_EGUIEditor_sendKeyEvent(
    mut env: JNIEnv, _: JClass, obj: jlong, key_code: jint, content: JString, pressed: jboolean, alt: jboolean, ctrl: jboolean, shift: jboolean
) {
    let obj = unsafe { &mut *(obj as *mut WgpuEditor) };

    let modifiers = egui::Modifiers {
        alt: alt == 1,
        ctrl: ctrl == 1,
        shift: shift == 1,
        mac_cmd: false,
        command: false
    };

    obj.raw_input.modifiers = modifiers;

    let Some(key) = AndroidKeys::from(key_code) else {
        return
    };

    if pressed == 1 && (modifiers.shift_only() || modifiers.is_none()) && key.valid_text() {
        let text: String = match env
            .get_string(&content) {
            Ok(cont) => cont.into(),
            Err(err) => format!("# The error is: {:?}", err)
        };

        obj.raw_input.events.push(Event::Text(text));
    }

    if let Some(key) = key.egui_key() {
        obj.raw_input
            .events
            .push(Event::Key { key, pressed: pressed == 1, repeat: false, modifiers });
    } else {

    }
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_egui_1editor_EGUIEditor_touchesBegin(
    _env: JNIEnv, _: JClass, obj: jlong, id: jint, x: jfloat, y: jfloat, pressure: jfloat
) {
    let obj = unsafe { &mut *(obj as *mut WgpuEditor) };

    println!("registering on begin: ({}, {})", x, y);

    obj.raw_input.events.push(Event::Touch {
        device_id: TouchDeviceId(0),
        id: TouchId(id as u64),
        phase: TouchPhase::Start,
        pos: Pos2 { x, y },
        force: pressure,
    });

    obj.raw_input.events.push(Event::PointerButton {
        pos: Pos2 { x, y },
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Default::default(),
    });
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_egui_1editor_EGUIEditor_touchesMoved(
    _env: JNIEnv, _: JClass, obj: jlong, id: jint, x: jfloat, y: jfloat, pressure: jfloat
) {
    let obj = unsafe { &mut *(obj as *mut WgpuEditor) };

    println!("registering on moved: ({}, {})", x, y);

    obj.raw_input.events.push(Event::Touch {
        device_id: TouchDeviceId(0),
        id: TouchId(id as u64),
        phase: TouchPhase::Move,
        pos: Pos2 { x, y },
        force: pressure,
    });

    obj.raw_input
        .events
        .push(Event::PointerMoved(Pos2 { x, y }));
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_egui_1editor_EGUIEditor_touchesEnded(
    _env: JNIEnv, _: JClass, obj: jlong, id: jint, x: jfloat, y: jfloat, pressure: jfloat
) {
    let obj = unsafe { &mut *(obj as *mut WgpuEditor) };

    println!("registering on ended: ({}, {})", x, y);

    obj.raw_input.events.push(Event::Touch {
        device_id: TouchDeviceId(0),
        id: TouchId(id as u64),
        phase: TouchPhase::End,
        pos: Pos2 { x, y },
        force: pressure,
    });

    obj.raw_input.events.push(Event::PointerButton {
        pos: Pos2 { x, y },
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Default::default(),
    });

    obj.raw_input.events.push(Event::PointerGone);
}

