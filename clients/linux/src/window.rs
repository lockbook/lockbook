use egui::{epaint::text::cursor, Context, PlatformOutput, Visuals};
use egui_wgpu_backend::{
    wgpu::{self, CompositeAlphaMode},
    ScreenDescriptor,
};
use lbeguiapp::{IntegrationOutput, UpdateOutput, WgpuLockbook};
use raw_window_handle::{
    HasRawDisplayHandle, HasRawWindowHandle, RawDisplayHandle, RawWindowHandle, XcbDisplayHandle,
    XcbWindowHandle,
};
use std::{ffi::c_void, time::Instant};
use x11rb::{
    atom_manager,
    connection::Connection,
    protocol::xproto::{ConnectionExt as _, *},
    protocol::{xproto, Event},
    wrapper::ConnectionExt as _,
    xcb_ffi::XCBConnection,
    COPY_DEPTH_FROM_PARENT,
};

// A collection of the atoms we will need.
atom_manager! {
    pub AtomCollection: AtomCollectionCookie {
        WM_PROTOCOLS,
        WM_DELETE_WINDOW,
        _NET_WM_NAME,
        UTF8_STRING,
        CLIPBOARD,
        ANY,
        NONE,
        TARGETS,
        INCR,

        // xdnd: drag 'n' drop x protocol extension
        XdndAware,
        XdndEnter,
        XdndPosition,
        XdndStatus,
        XdndLeave,
        XdndDrop,
        XdndFinished,
        XdndTypeList,
        XdndSelection,
        XdndActionCopy,
        XdndActionMove,
        XdndActionLink,
        XdndActionNone,
        XdndTargets,
        XdndVersion,

        // content types
        TextUriList: b"text/uri-list",
        ImagePng: b"image/png",
    }
}

use crate::{
    input::{self, clipboard_paste},
    output,
};

pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    std::env::set_var("WAYLAND_DISPLAY", "");

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
        // EventMask::KEYMAP_STATE,
        // EventMask::EXPOSURE,
        // EventMask::VISIBILITY_CHANGE,
        EventMask::STRUCTURE_NOTIFY,
        // EventMask::RESIZE_REDIRECT,
        EventMask::SUBSTRUCTURE_NOTIFY,
        EventMask::SUBSTRUCTURE_REDIRECT,
        // EventMask::FOCUS_CHANGE,
        EventMask::PROPERTY_CHANGE,
        // EventMask::COLOR_MAP_CHANGE,
        // EventMask::OWNER_GRAB_BUTTON,
    ];
    let event_mask = events.iter().fold(EventMask::NO_EVENT, |acc, &x| acc | x);

    let (conn, screen_num) = x11rb::xcb_ffi::XCBConnection::connect(None).unwrap();
    let conn = &conn;
    let atoms = AtomCollection::new(conn)?.reply()?;
    let screen = &conn.setup().roots[screen_num];
    let db: x11rb::resource_manager::Database = x11rb::resource_manager::new_from_default(conn)?;
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

    output::window_title::handle(conn, window_id, &atoms, Some("Lockbook".to_string()))?;

    // register for a 'delete window' event
    conn.change_property32(
        PropMode::REPLACE,
        window_id,
        atoms.WM_PROTOCOLS,
        AtomEnum::ATOM,
        &[atoms.WM_DELETE_WINDOW],
    )?;

    // we are aware of drag 'n' drop (xdnd version 5)
    conn.change_property32(PropMode::REPLACE, window_id, atoms.XdndAware, AtomEnum::ATOM, &[5])?;

    // sets window class & instance (one of these appears as title in alt tab) and makes app respond to window manager
    conn.change_property8(
        PropMode::REPLACE,
        window_id,
        AtomEnum::WM_CLASS,
        AtomEnum::STRING,
        "Lockbook\0Lockbook\0".as_bytes(),
    )?;

    conn.map_window(window_id)?;
    conn.flush()?;

    let window_handle = WindowHandle {
        window_id,
        connection: conn.get_raw_xcb_connection(),
        screen: screen_num as _,
    };
    let mut lb = init(
        &window_handle,
        ScreenDescriptor { physical_width: 1300, physical_height: 800, scale_factor: 1.0 },
        false,
    );

    let mut last_copied_text = String::new();
    let mut paste_context = clipboard_paste::Context::new(window_id, conn, &atoms);
    let cursor_manager = output::cursor::Manager::new(conn, screen_num)?;
    loop {
        while let Some(event) = conn.poll_for_event()? {
            handle(conn, &atoms, &last_copied_text, event, &mut lb, &mut paste_context)?;
        }
        let IntegrationOutput {
            redraw_in: _, // todo: handle? how's this different from checking egui context?
            egui: PlatformOutput { cursor_icon, open_url, copied_text, .. },
            update_output: UpdateOutput { close, set_window_title },
        } = lb.frame();

        // set modifiers
        let pointer_state = conn.query_pointer(window_id)?.reply()?;
        lb.raw_input.modifiers = input::modifiers(pointer_state.mask);

        // set scale factor
        let scale_factor = match db.get_string("Xft.dpi", "") {
            Some(dpi) => {
                let dpi = dpi.parse::<f32>().unwrap_or(96.0);
                dpi / 96.0
            }
            None => {
                println!("Failed to get Xft.dpi");
                1.0
            }
        };
        lb.screen.scale_factor = scale_factor;
        lb.raw_input.pixels_per_point = Some(scale_factor);

        if close {
            output::close();
        }
        output::window_title::handle(conn, window_id, &atoms, set_window_title)?;
        cursor_manager.handle(conn, &db, screen_num, window_id, cursor_icon);
        output::open_url::handle(open_url);
        output::clipboard_copy::handle_copy(
            conn,
            &atoms,
            window_id,
            copied_text,
            &mut last_copied_text,
        )?;
        conn.flush()?;
    }
}

fn handle(
    conn: &XCBConnection, atoms: &AtomCollection, last_copied_text: &str, event: Event,
    lb: &mut WgpuLockbook, paste_context: &mut clipboard_paste::Context,
) -> Result<(), Box<dyn std::error::Error>> {
    match event {
        // pointer
        Event::ButtonPress(event) => {
            input::pointer::handle_press(lb, event, lb.screen.scale_factor)
        }
        Event::ButtonRelease(event) => {
            input::pointer::handle_release(lb, event, lb.screen.scale_factor)
        }
        Event::MotionNotify(event) => {
            input::pointer::handle_motion(lb, event, lb.screen.scale_factor)
        }

        // keyboard
        Event::KeyPress(event) => {
            input::key::handle(event.detail, event.state, true, lb, paste_context)?
        }
        Event::KeyRelease(event) => {
            input::key::handle(event.detail, event.state, false, lb, paste_context)?
        }

        // resize
        Event::ConfigureNotify(event) => {
            lb.screen.physical_width = event.width as _;
            lb.screen.physical_height = event.height as _;
        }

        // drag 'n' drop/copy 'n' paste
        Event::ClientMessage(event) => {
            if event.type_ == atoms.WM_PROTOCOLS
                && event.data.as_data32()[0] == atoms.WM_DELETE_WINDOW
            {
                // close
                output::close();
            } else if event.type_ == atoms.XdndEnter {
                input::file_drop::handle_enter(conn, atoms, &event)?;
            } else if event.type_ == atoms.XdndPosition {
                input::file_drop::handle_position(conn, atoms, &event)?;
            } else if event.type_ == atoms.XdndStatus {
                input::file_drop::handle_status(conn, atoms, &event)?;
            } else if event.type_ == atoms.XdndLeave {
                input::file_drop::handle_leave(conn, atoms, &event)?;
            } else if event.type_ == atoms.XdndDrop {
                input::file_drop::handle_drop(conn, atoms, &event)?;
            }
        }
        Event::SelectionNotify(event) => {
            if event.property == atoms.XdndSelection {
                input::file_drop::handle_selection_notify(conn, atoms, &event, lb)?;
            } else {
                paste_context.handle_selection_notify(&event, lb)?;
            }
        }
        Event::PropertyNotify(event) => {
            if event.atom == atoms.CLIPBOARD && event.state == xproto::Property::NEW_VALUE {
                paste_context.handle_property_notify(&event, lb)?;
            }
        }
        Event::SelectionRequest(event) => {
            output::clipboard_copy::handle_selection_request(
                conn,
                atoms,
                &event,
                last_copied_text,
            )?;
        }

        _ => {}
    };

    Ok(())
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
    window: &W, screen: ScreenDescriptor, dark_mode: bool,
) -> WgpuLockbook {
    let backends = wgpu::util::backend_bits_from_env().unwrap_or_else(wgpu::Backends::all);
    let instance_desc = wgpu::InstanceDescriptor { backends, ..Default::default() };
    let instance = wgpu::Instance::new(instance_desc);
    let surface = unsafe { instance.create_surface(window) }.unwrap();
    let (adapter, device, queue) =
        pollster::block_on(request_device(&instance, backends, &surface));
    let format = surface.get_capabilities(&adapter).formats[0];
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
