use crate::WgpuWorkspace;
use egui::{Context, FontDefinitions};
use egui_wgpu_backend::wgpu::{CompositeAlphaMode, SurfaceTargetUnsafe};
use egui_wgpu_backend::{ScreenDescriptor, wgpu};
use lb_c::Lb;
use std::ffi::c_void;
use std::time::Instant;
use workspace_rs::register_fonts;
use workspace_rs::theme::visuals;
use workspace_rs::workspace::Workspace;

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn init_ws(
    core: *mut c_void, metal_layer: *mut c_void, dark_mode: bool,
) -> *mut c_void {
    let core = unsafe { &mut *(core as *mut Lb) };

    let backends = wgpu::util::backend_bits_from_env().unwrap_or_else(wgpu::Backends::all);
    let instance_desc = wgpu::InstanceDescriptor { backends, ..Default::default() };
    let instance = wgpu::Instance::new(instance_desc);
    let surface = instance
        .create_surface_unsafe(SurfaceTargetUnsafe::CoreAnimationLayer(metal_layer))
        .unwrap();
    let (adapter, device, queue) = pollster::block_on(request_device(&instance, &surface));

    let avail_formats = surface.get_capabilities(&adapter).formats;
    let format = avail_formats[0];

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
    let rpass = egui_wgpu_backend::RenderPass::new(&device, format, 4);

    let context = Context::default();
    visuals::init(&context, dark_mode);
    let workspace = Workspace::new(core, &context);
    let mut fonts = FontDefinitions::default();
    register_fonts(&mut fonts);
    context.set_fonts(fonts);
    egui_extras::install_image_loaders(&context);

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
                memory_hints: Default::default(),
            },
            None,
        )
        .await;
    match res {
        Err(err) => {
            panic!("request_device failed: {err:?}");
        }
        Ok((device, queue)) => (adapter, device, queue),
    }
}

#[no_mangle]
pub extern "C" fn resize_editor(obj: *mut c_void, width: f32, height: f32, scale: f32) {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };
    obj.screen.physical_width = width as u32;
    obj.screen.physical_height = height as u32;
    obj.screen.scale_factor = scale;
}
