use crate::{CUuid, IntegrationOutput, WgpuWorkspace};
use egui::os::OperatingSystem;
use egui::{vec2, Context, Event, FontDefinitions, Pos2};
use egui_wgpu_backend::wgpu::{CompositeAlphaMode, SurfaceTargetUnsafe};
use egui_wgpu_backend::{wgpu, ScreenDescriptor};
use lb_external_interface::lb_rs::Core;
use lb_external_interface::lb_rs::Uuid;
use std::ffi::{c_char, c_void, CStr, CString};
use std::path::PathBuf;
use std::time::Instant;
use workspace_rs::register_fonts;
use workspace_rs::tab::{ClipContent, EventManager as _};
use workspace_rs::theme::visuals;
use workspace_rs::workspace::{Workspace, WsConfig};

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn init_ws(
    core: *mut c_void, metal_layer: *mut c_void, dark_mode: bool,
) -> *mut c_void {
    let core = unsafe { &mut *(core as *mut Core) };
    let writable_dir = core.get_config().unwrap().writeable_path;

    let backends = wgpu::util::backend_bits_from_env().unwrap_or_else(wgpu::Backends::all);
    let instance_desc = wgpu::InstanceDescriptor { backends, ..Default::default() };
    let instance = wgpu::Instance::new(instance_desc);
    let surface = instance
        .create_surface_unsafe(SurfaceTargetUnsafe::CoreAnimationLayer(metal_layer))
        .unwrap();
    let (adapter, device, queue) = pollster::block_on(request_device(&instance, &surface));
    let format = surface.get_capabilities(&adapter).formats[0];
    let screen =
        ScreenDescriptor { physical_width: 1000, physical_height: 1000, scale_factor: 1.0 };
    let surface_config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format,
        width: screen.physical_width, // TODO get from context or something
        height: screen.physical_height,
        present_mode: wgpu::PresentMode::Fifo,
        alpha_mode: CompositeAlphaMode::Auto,
        view_formats: vec![],
        desired_maximum_frame_latency: 2,
    };
    surface.configure(&device, &surface_config);
    let rpass = egui_wgpu_backend::RenderPass::new(&device, format, 1);

    let context = Context::default();
    visuals::init(&context, dark_mode);
    let ws_cfg = WsConfig { data_dir: writable_dir, ..Default::default() };
    let workspace = Workspace::new(ws_cfg, core, &context);
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
        context,
        raw_input: Default::default(),
        workspace,
        surface_width: 0,
        surface_height: 0,
    };

    Box::into_raw(Box::new(obj)) as *mut c_void
}

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

    obj.workspace.open_file(id, new_file, true)
}

#[no_mangle]
pub extern "C" fn request_sync(obj: *mut c_void) {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };
    obj.workspace.perform_sync()
}

#[no_mangle]
pub extern "C" fn draw_workspace(obj: *mut c_void) -> IntegrationOutput {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };
    obj.frame()
}

#[no_mangle]
pub extern "C" fn resize_editor(obj: *mut c_void, width: f32, height: f32, scale: f32) {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };
    obj.screen.physical_width = width as u32;
    obj.screen.physical_height = height as u32;
    obj.screen.scale_factor = scale;
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
/// obj must be a valid pointer to WgpuEditor
#[no_mangle]
pub unsafe extern "C" fn clipboard_paste(obj: *mut c_void, content: *const c_char) {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    let content = CStr::from_ptr(content).to_str().unwrap().into();
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
    obj: *mut c_void, content: *const c_char, is_paste: bool,
) {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    let file_url: String = CStr::from_ptr(content).to_str().unwrap().into();
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
/// obj must be a valid pointer to WgpuEditor
///
/// used solely for image pasting
#[no_mangle]
pub unsafe extern "C" fn paste_text(obj: *mut c_void, content: *const c_char) {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    let content = CStr::from_ptr(content).to_str().unwrap().into();

    obj.raw_input.events.push(Event::Paste(content));
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn deinit_editor(obj: *mut c_void) {
    println!("EDITOR DENININTEED");
    let _ = Box::from_raw(obj as *mut WgpuWorkspace);
}

async fn request_device(
    instance: &wgpu::Instance, surface: &wgpu::Surface<'_>,
) -> (wgpu::Adapter, wgpu::Device, wgpu::Queue) {
    let adapter = wgpu::util::initialize_adapter_from_env_or_default(instance, Some(surface))
        .await
        .expect("No suitable GPU adapters found on the system!");
    let adapter_info = adapter.get_info();
    println!("Using {} ({:?})", adapter_info.name, adapter_info.backend);
    let res = adapter
        .request_device(
            &wgpu::DeviceDescriptor {
                label: None,
                required_features: adapter.features(),
                required_limits: adapter.limits(),
            },
            None,
        )
        .await;
    match res {
        Err(err) => {
            panic!("request_device failed: {:?}", err);
        }
        Ok((device, queue)) => (adapter, device, queue),
    }
}

/// (macos only)
/// # Safety
#[no_mangle]
pub unsafe extern "C" fn scroll_wheel(obj: *mut c_void, scroll_x: f32, scroll_y: f32) {
    let obj = &mut *(obj as *mut WgpuWorkspace);

    if matches!(obj.context.os(), OperatingSystem::IOS) {
        obj.raw_input
            .events
            .push(Event::PointerMoved(Pos2 { x: 1.0, y: 200.0 }));
    }

    if obj.raw_input.modifiers.command || obj.raw_input.modifiers.ctrl {
        let factor = (scroll_y / 50.).exp();

        obj.raw_input.events.push(Event::Zoom(factor))
    } else {
        obj.raw_input
            .events
            .push(Event::Scroll(vec2(scroll_x, scroll_y)));
    }

    if matches!(obj.context.os(), OperatingSystem::IOS) {
        obj.raw_input.events.push(Event::PointerGone);
    }
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

    let _ = obj
        .workspace
        .updates_tx
        .send(workspace_rs::workspace::WsMsg::FileRenamed { id, new_name });
}

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

    if let Some(tab_id) = obj.workspace.tabs.iter().position(|tab| tab.id == id) {
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
    let syncing = obj.workspace.status.syncing;
    let msg = obj.workspace.status.message.clone();
    let msg = CString::new(msg)
        .expect("Could not Rust String -> C String")
        .into_raw();

    FfiWsStatus { syncing, msg }
}
