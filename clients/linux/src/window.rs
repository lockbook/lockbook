
use std::{time::{Instant, Duration}, ffi::c_void};

use egui::{Context, Visuals};
use egui_wgpu_backend::{
    wgpu::{self, CompositeAlphaMode},
    ScreenDescriptor,
};
use lbeguiapp::WgpuLockbook;
use raw_window_handle::{
    HasRawDisplayHandle, HasRawWindowHandle, RawDisplayHandle, RawWindowHandle, XcbWindowHandle, XcbDisplayHandle,
};

use x11rb::{protocol::xproto::*, connection::Connection};
use x11rb::COPY_DEPTH_FROM_PARENT;

pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let (conn, screen_num) = x11rb::xcb_ffi::XCBConnection::connect(None).unwrap();
    let screen = &conn.setup().roots[screen_num];
    let win_id = conn.generate_id()?;
    conn.create_window(
        COPY_DEPTH_FROM_PARENT,
        win_id,
        screen.root,
        0,
        0,
        1300,
        800,
        0,
        WindowClass::INPUT_OUTPUT,
        0,
        &CreateWindowAux::new().background_pixel(screen.white_pixel),
    )?;
    conn.map_window(win_id)?;
    conn.flush()?;

    let window = WindowHandle {
        window: win_id,
        connection: conn.get_raw_xcb_connection(),
        screen: screen_num as _,
    };
    let mut lb = init(&window, false);

    loop {
        lb.frame();
        std::thread::sleep(Duration::from_millis(16));
    }
}

pub struct WindowHandle {
    window: u32,
    connection: *mut c_void,
    screen: i32,
}

// Smails implementations adapted for windows with reference to winit's linux implementation:
// https://github.com/lockbook/lockbook/pull/1835/files#diff-0f28854a868a55fcd30ff5f0fda476aed540b2e1fc3762415ac6e0588ed76fb6
// https://github.com/rust-windowing/winit/blob/ee0db52ac49d64b46c500ef31d7f5f5107ce871a/src/platform_impl/windows/window.rs#L334-L346
unsafe impl HasRawWindowHandle for WindowHandle {
    fn raw_window_handle(&self) -> RawWindowHandle {
        let mut result = XcbWindowHandle::empty();
        result.window = self.window;
        RawWindowHandle::Xcb(result)
    }
}

unsafe impl HasRawDisplayHandle for WindowHandle {
    fn raw_display_handle(&self) -> RawDisplayHandle {
        let mut result = XcbDisplayHandle::empty();
        result.connection = self.connection;
        result.screen = self.screen;
        RawDisplayHandle::Xcb(result)
    }
}

// The rest of the code in this file would go in main except for this code to build on linux it needs to all be under a cfg(windows)
#[derive(Default)]
pub struct Window {
    maybe_app: Option<WgpuLockbook>, // must be populated after the window is created
    width: u16,
    height: u16,
    dpi_scale: f32,
}

// Taken from other lockbook code
pub fn init<W: raw_window_handle::HasRawWindowHandle + raw_window_handle::HasRawDisplayHandle>(
    window: &W, dark_mode: bool,
) -> WgpuLockbook {
    let backends = wgpu::util::backend_bits_from_env().unwrap_or_else(wgpu::Backends::all);
    let instance_desc = wgpu::InstanceDescriptor { backends, ..Default::default() };
    let instance = wgpu::Instance::new(instance_desc);
    let surface = unsafe { instance.create_surface(window) }.unwrap();
    let (adapter, device, queue) =
        pollster::block_on(request_device(&instance, backends, &surface));
    let format = surface.get_capabilities(&adapter).formats[0];
    let screen = ScreenDescriptor { physical_width: 1300, physical_height: 800, scale_factor: 1.0 }; // initial value overridden by resize
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
    context.set_visuals(if dark_mode { Visuals::dark() } else { Visuals::light() });

    let (settings, maybe_settings_err) = match lbeguiapp::Settings::read_from_file() {
        Ok(s) => (s, None),
        Err(err) => (Default::default(), Some(err.to_string())),
    };
    let app = lbeguiapp::Lockbook::new(&context, settings, maybe_settings_err);

    let start_time = Instant::now();
    let mut obj = WgpuLockbook {
        start_time,
        device,
        queue,
        surface,
        adapter,
        rpass,
        screen,
        context,
        raw_input: Default::default(),
        from_egui: None,
        from_host: None,
        app,
    };

    obj.frame();

    obj
}

async fn request_device(
    instance: &wgpu::Instance, backend: wgpu::Backends, surface: &wgpu::Surface,
) -> (wgpu::Adapter, wgpu::Device, wgpu::Queue) {
    let adapter =
        wgpu::util::initialize_adapter_from_env_or_default(instance, backend, Some(surface))
            .await
            .expect("No suitable GPU adapters found on the system!");
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

// https://github.com/rust-windowing/winit/blob/789a4979801cffc20c9dfbc34e72c15ebf3c737c/src/platform_impl/windows/dpi.rs#L75C1-L78C2
pub fn dpi_to_scale_factor(dpi: u16) -> f32 {
    todo!()
}
