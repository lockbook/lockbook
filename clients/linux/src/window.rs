use egui::{Context, Visuals};
use egui_wgpu_backend::{
    wgpu::{self, CompositeAlphaMode},
    ScreenDescriptor,
};
use lbeguiapp::WgpuLockbook;
use raw_window_handle::{
    HasRawDisplayHandle, HasRawWindowHandle, RawDisplayHandle, RawWindowHandle, XcbDisplayHandle,
    XcbWindowHandle,
};
use std::{ffi::c_void, time::Instant};

use x11rb::{connection::Connection, protocol::xproto::*};
use x11rb::{protocol::Event, COPY_DEPTH_FROM_PARENT};

use crate::input;

pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let events = [
        EventMask::KEY_PRESS,
        EventMask::KEY_RELEASE,
        EventMask::BUTTON_PRESS,
        EventMask::BUTTON_RELEASE,
        EventMask::ENTER_WINDOW,
        EventMask::LEAVE_WINDOW,
        EventMask::POINTER_MOTION,
        // EventMask::POINTER_MOTION_HINT,
        // EventMask::BUTTON1_MOTION,
        // EventMask::BUTTON2_MOTION,
        // EventMask::BUTTON3_MOTION,
        // EventMask::BUTTON4_MOTION,
        // EventMask::BUTTON5_MOTION,
        // EventMask::BUTTON_MOTION,
        EventMask::KEYMAP_STATE,
        EventMask::EXPOSURE,
        EventMask::VISIBILITY_CHANGE,
        EventMask::STRUCTURE_NOTIFY,
        EventMask::RESIZE_REDIRECT,
        EventMask::SUBSTRUCTURE_NOTIFY,
        EventMask::SUBSTRUCTURE_REDIRECT,
        EventMask::FOCUS_CHANGE,
        EventMask::PROPERTY_CHANGE,
        EventMask::COLOR_MAP_CHANGE,
        EventMask::OWNER_GRAB_BUTTON,
    ];
    let event_mask = events.iter().fold(EventMask::NO_EVENT, |acc, &x| acc | x);

    let (conn, screen_num) = x11rb::xcb_ffi::XCBConnection::connect(None).unwrap();
    let screen = &conn.setup().roots[screen_num];
    let window_id = conn.generate_id()?;
    conn.create_window(
        COPY_DEPTH_FROM_PARENT,
        window_id,
        screen.root,
        0,
        0,
        1300,
        800,
        0,
        WindowClass::INPUT_OUTPUT,
        0,
        &CreateWindowAux::new()
            .background_pixel(screen.white_pixel)
            .event_mask(event_mask),
    )?;
    conn.map_window(window_id)?;
    conn.flush()?;

    let window = WindowHandle {
        window_id,
        connection: conn.get_raw_xcb_connection(),
        screen: screen_num as _,
    };
    let mut lb = init(&window, false);

    loop {
        while let Some(event) = conn.poll_for_event()? {
            handle(&mut lb, event);
        }
        println!("frame");
        lb.frame();
    }
}

fn handle(lb: &mut WgpuLockbook, event: Event) {
    match event {
        // pointer
        Event::ButtonPress(event) => input::pointer::handle_press(lb, event),
        Event::ButtonRelease(event) => input::pointer::handle_release(lb, event),
        Event::MotionNotify(event) => input::pointer::handle_motion(lb, event),

        // keyboard
        Event::KeyPress(_) => {
            println!("KeyPress")
        }
        Event::KeyRelease(_) => {
            println!("KeyRelease")
        }

        // window resize
        Event::ResizeRequest(_) => {
            println!("ResizeRequest")
        }

        // focus
        Event::FocusIn(_) => {
            println!("FocusIn")
        }
        Event::FocusOut(_) => {
            println!("FocusOut")
        }

        // ?
        Event::ConfigureNotify(_) => {
            println!("ConfigureNotify")
        }
        Event::PropertyNotify(_) => {
            println!("PropertyNotify")
        }

        _ => {}
    }
}

pub struct WindowHandle {
    window_id: u32,
    connection: *mut c_void,
    screen: i32,
}

unsafe impl HasRawWindowHandle for WindowHandle {
    fn raw_window_handle(&self) -> RawWindowHandle {
        let mut result = XcbWindowHandle::empty();
        result.window = self.window_id;
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
