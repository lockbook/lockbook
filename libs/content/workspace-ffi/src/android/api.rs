use egui::{PointerButton, Pos2, TouchDeviceId, TouchId, TouchPhase};
use jni::JNIEnv;
use jni::objects::{JClass, JString};
use jni::sys::{jboolean, jfloat, jint, jlong, jstring};
use lb_c::Uuid;
use lb_c::model::text::offset_types::DocCharOffset;
use serde::Serialize;
use std::panic::catch_unwind;
use workspace_rs::tab::markdown_editor::input::{Event, Location, Region};
use workspace_rs::tab::svg_editor::Tool;
use workspace_rs::tab::{ContentState, ExtendedInput, TabContent};

use super::keyboard::AndroidKeys;
use super::response::*;
use crate::WgpuWorkspace;

#[unsafe(no_mangle)]
pub extern "system" fn Java_app_lockbook_workspace_Workspace_enterFrame(
    env: JNIEnv, _: JClass, obj: jlong,
) -> jstring {
    let maybe_err = catch_unwind(|| {
        let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };
        let response: AndroidResponse = obj.frame().into();
        serde_json::to_string(&response).unwrap()
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
            Err(err) => format!("# The error is: {err:?}"),
        };

        obj.raw_input.events.push(egui::Event::Text(text));
    }

    if let Some(key) = key.egui_key() {
        obj.raw_input.events.push(egui::Event::Key {
            key,
            physical_key: None,
            pressed: pressed == 1,
            repeat: false,
            modifiers,
        });
    }
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_workspace_Workspace_insertTextAtCursor(
    mut env: JNIEnv, _: JClass, obj: jlong, content: JString,
) {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };

    if let Ok(text) = env.get_string(&content) {
        obj.raw_input.events.push(egui::Event::Text(text.into()));
    }
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_workspace_Workspace_touchesBegin(
    _env: JNIEnv, _: JClass, ogbj: jlong, id: jint, x: jfloat, y: jfloat, pressure: jfloat,
) {
    let obj = unsafe { &mut *(ogbj as *mut WgpuWorkspace) };

    obj.raw_input.events.push(egui::Event::Touch {
        device_id: TouchDeviceId(0),
        id: TouchId(id as u64),
        phase: TouchPhase::Start,
        pos: Pos2 { x, y },
        force: Some(pressure),
    });

    obj.raw_input.events.push(egui::Event::PointerButton {
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

    obj.raw_input.events.push(egui::Event::Touch {
        device_id: TouchDeviceId(0),
        id: TouchId(id as u64),
        phase: TouchPhase::Move,
        pos: Pos2 { x, y },
        force: Some(pressure),
    });

    obj.raw_input
        .events
        .push(egui::Event::PointerMoved(Pos2 { x, y }));
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_workspace_Workspace_touchesEnded(
    _env: JNIEnv, _: JClass, obj: jlong, id: jint, x: jfloat, y: jfloat, pressure: jfloat,
) {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };

    obj.raw_input.events.push(egui::Event::Touch {
        device_id: TouchDeviceId(0),
        id: TouchId(id as u64),
        phase: TouchPhase::End,
        pos: Pos2 { x, y },
        force: Some(pressure),
    });

    obj.raw_input.events.push(egui::Event::PointerButton {
        pos: Pos2 { x, y },
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Default::default(),
    });

    obj.raw_input.events.push(egui::Event::PointerGone);
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_workspace_Workspace_touchesCancelled(
    _env: JNIEnv, _: JClass, obj: jlong, id: jint, x: jfloat, y: jfloat, pressure: jfloat,
) {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };

    obj.raw_input.events.push(egui::Event::Touch {
        device_id: TouchDeviceId(0),
        id: TouchId(id as u64),
        phase: TouchPhase::Cancel,
        pos: Pos2 { x, y },
        force: Some(pressure),
    });

    obj.raw_input.events.push(egui::Event::PointerGone);
}

#[derive(Debug, Serialize)]
pub struct WsStatus {
    pub syncing: bool,
    pub msg: String,
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_workspace_Workspace_getStatus(
    env: JNIEnv, _: JClass, obj: jlong,
) -> jstring {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };

    let status = WsStatus {
        syncing: obj.workspace.visibly_syncing(),
        msg: obj.workspace.status.message.clone(),
    };

    env.new_string(serde_json::to_string(&status).unwrap())
        .expect("Couldn't create JString from rust string!")
        .into_raw()
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_workspace_Workspace_openDoc(
    mut env: JNIEnv, _: JClass, obj: jlong, jid: JString, new_file: jboolean,
) {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };

    let rid: String = env.get_string(&jid).unwrap().into();
    let id = Uuid::parse_str(&rid).unwrap();

    obj.workspace.open_file(id, new_file == 1, true, false);
}

// todo: can't close non-file tabs (mind map)
#[no_mangle]
pub extern "system" fn Java_app_lockbook_workspace_Workspace_closeDoc(
    mut env: JNIEnv, _: JClass, obj: jlong, jid: JString,
) {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };

    let rid: String = env.get_string(&jid).unwrap().into();
    let id = Uuid::parse_str(&rid).unwrap();

    if let Some(tab_id) = obj
        .workspace
        .tabs
        .iter()
        .position(|tab| tab.id() == Some(id))
    {
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
    obj.workspace.tasks.queue_sync();
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_workspace_Workspace_currentTab(
    _env: JNIEnv, _: JClass, obj: jlong,
) -> jint {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };

    match obj.workspace.current_tab() {
        Some(tab) => match &tab.content {
            ContentState::Open(tab) => match tab {
                TabContent::Image(_) => 2,
                TabContent::Markdown(_) => 3,
                // TabContent::PlainText(_) => 4,
                TabContent::Pdf(_) => 5,
                TabContent::Svg(_) => 6,
                TabContent::MindMap(_) => 7,
                TabContent::SpaceInspector(_) => 8,
            },
            _ => 1,
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

    obj.workspace.file_renamed(id, new_name);
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
                .into_raw();
        }
    };

    env.new_string(&markdown.buffer.current.text)
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
            let (start, end) = markdown.buffer.current.selection;
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

    obj.context.push_markdown_event(Event::Select {
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

    markdown.buffer.current.segs.last_cursor_position().0 as jint
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

    obj.context.push_markdown_event(Event::Replace {
        region: Region::BetweenLocations {
            start: Location::DocCharOffset(DocCharOffset(0)),
            end: Location::DocCharOffset(markdown.buffer.current.segs.last_cursor_position()),
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
        Err(err) => format!("error: {err:?}"),
    };

    obj.context.push_markdown_event(Event::Replace {
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
        Err(err) => format!("error: {err:?}"),
    };

    let loc = Location::DocCharOffset(DocCharOffset(index as usize));

    obj.context.push_markdown_event(Event::Replace {
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
        Err(err) => format!("error: {err:?}"),
    };

    let loc = Location::DocCharOffset(markdown.buffer.current.segs.last_cursor_position());

    obj.context.push_markdown_event(Event::Replace {
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
                .into_raw();
        }
    };

    let selection = (DocCharOffset(start as usize), DocCharOffset(end as usize));
    env.new_string(&markdown.buffer[selection])
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

    let segs = &markdown.buffer.current.segs;

    obj.context.push_markdown_event(Event::Select {
        region: Region::BetweenLocations {
            start: Location::DocCharOffset(DocCharOffset(0)),
            end: Location::DocCharOffset(segs.last_cursor_position()),
        },
    });
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_workspace_Workspace_clipboardCut(
    _env: JNIEnv, _: JClass, obj: jlong,
) {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };
    obj.context.push_markdown_event(Event::Cut);
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_workspace_Workspace_clipboardCopy(
    _env: JNIEnv, _: JClass, obj: jlong,
) {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };
    obj.context.push_markdown_event(Event::Copy);
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_workspace_Workspace_clipboardPaste(
    mut env: JNIEnv, _: JClass, obj: jlong, content: JString,
) {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };

    let content: String = match env.get_string(&content) {
        Ok(cont) => cont.into(),
        Err(err) => format!("# The error is: {err:?}"),
    };

    obj.raw_input.events.push(egui::Event::Paste(content));
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_workspace_Workspace_toggleEraserSVG(
    _env: JNIEnv, _: JClass, obj: jlong, select: jboolean,
) {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };

    if let Some(svg) = obj.workspace.current_tab_svg_mut() {
        if select == 1 {
            svg.toolbar
                .set_tool(Tool::Eraser, &mut svg.settings, &mut svg.cfg);
        } else if svg.toolbar.active_tool == Tool::Eraser {
            svg.toolbar.set_tool(
                svg.toolbar.previous_tool.unwrap_or(Tool::Pen),
                &mut svg.settings,
                &mut svg.cfg,
            );
        }
    }
}
