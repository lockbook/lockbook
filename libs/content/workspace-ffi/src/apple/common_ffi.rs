use crate::{CUuid, IntegrationOutput, WgpuWorkspace};
use egui::os::OperatingSystem;
use egui::{vec2, Context, Event, FontDefinitions, Pos2};
use egui_wgpu_backend::wgpu::CompositeAlphaMode;
use egui_wgpu_backend::{wgpu, ScreenDescriptor};
use lb_external_interface::Core;
use std::ffi::{c_char, c_void, CStr, CString};
use std::ptr::NonNull;
use std::sync::Arc;
use std::time::Instant;
use workspace_rs::register_fonts;
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
    let surface = instance.create_surface_from_core_animation_layer(metal_layer);
    let (adapter, device, queue) =
        pollster::block_on(request_device(&instance, backends, &surface));
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
    let mut obj = WgpuWorkspace {
        start_time,
        device,
        queue,
        surface,
        adapter,
        rpass,
        screen,
        context,
        raw_input: Default::default(),
        from_host: None,
        workspace,
    };

    obj.frame();

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

    obj.workspace.open_file(id, new_file)
}

// parameters: context and sync message
pub type UpdateSyncStatus = extern "C" fn(*const c_char, *const c_char);

struct CClassPtr {
    ptr: NonNull<c_char>,
}
unsafe impl Send for CClassPtr {}
unsafe impl Sync for CClassPtr {}

#[no_mangle]
pub extern "C" fn request_sync(
    obj: *mut c_void, context: *const c_char, update_status: UpdateSyncStatus,
) {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };

    let context = Arc::new(CClassPtr {
        ptr: NonNull::new(context as *mut c_char).expect("context cannot be null"),
    });

    let f = move |msg: String| {
        let cmsg = CString::new(msg)
            .expect("Could not Rust String -> C String")
            .into_raw();

        update_status(context.ptr.as_ptr(), cmsg);
    };

    obj.workspace.perform_sync(Some(Box::new(f)))
}

#[no_mangle]
pub extern "C" fn draw_editor(obj: *mut c_void) -> IntegrationOutput {
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
#[no_mangle]
pub unsafe extern "C" fn system_clipboard_changed(obj: *mut c_void, content: *const c_char) {
    let obj = &mut *(obj as *mut WgpuWorkspace);
    let content = CStr::from_ptr(content).to_str().unwrap().into();
    obj.from_host = Some(content)
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn free_text(s: *mut c_void) {
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
    instance: &wgpu::Instance, backend: wgpu::Backends, surface: &wgpu::Surface,
) -> (wgpu::Adapter, wgpu::Device, wgpu::Queue) {
    let adapter =
        wgpu::util::initialize_adapter_from_env_or_default(instance, backend, Some(surface))
            .await
            .expect("No suitable GPU adapters found on the system!");
    let adapter_info = adapter.get_info();
    println!("Using {} ({:?})", adapter_info.name, adapter_info.backend);
    let res = adapter
        .request_device(
            &wgpu::DeviceDescriptor {
                label: None,
                features: adapter.features(),
                limits: adapter.limits(),
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
