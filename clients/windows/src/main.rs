#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use clipboard_win::{formats, get_clipboard, set_clipboard};
use egui::{Context, Visuals};
use egui_wgpu_backend::wgpu::CompositeAlphaMode;
use egui_wgpu_backend::{wgpu, ScreenDescriptor};
use input::key;
use lbeguiapp::WgpuLockbook;
use message::Message;
use std::mem;
use std::time::{Duration, Instant};
use windows::{
    core::*, Win32::Foundation::*, Win32::Graphics::Direct3D12::*, Win32::Graphics::Dxgi::*,
    Win32::System::LibraryLoader::*, Win32::UI::HiDpi::*, Win32::UI::Input::KeyboardAndMouse::*,
    Win32::UI::WindowsAndMessaging::*,
};

mod input;
mod message;
mod window;

#[cfg(not(windows))]
fn main() {}

#[cfg(windows)]
fn main() -> Result<()> {
    env_logger::init();

    let instance = unsafe { GetModuleHandleA(None)? };

    let wc = WNDCLASSEXA {
        cbSize: std::mem::size_of::<WNDCLASSEXA>() as u32,
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(handle_messages), // "Long Pointer to FuNction WiNDows PROCedure" (message handling callback)
        hInstance: instance.into(),
        hCursor: unsafe { LoadCursorW(None, IDC_ARROW) }?,
        lpszClassName: s!("Lockbook"),
        ..Default::default()
    };
    debug_assert_ne!(unsafe { RegisterClassExA(&wc) }, 0);

    let dxgi_factory: IDXGIFactory4 = {
        if cfg!(debug_assertions) {
            unsafe {
                let mut debug: Option<ID3D12Debug> = None;
                if let Some(debug) = D3D12GetDebugInterface(&mut debug).ok().and(debug) {
                    debug.EnableDebugLayer();
                }
            }
        }

        let dxgi_factory_flags = if cfg!(debug_assertions) { DXGI_CREATE_FACTORY_DEBUG } else { 0 };
        unsafe { CreateDXGIFactory2(dxgi_factory_flags) }?
    };

    let mut window_rect = RECT { left: 0, top: 0, right: 1300, bottom: 800 };
    unsafe { AdjustWindowRect(&mut window_rect, WS_OVERLAPPEDWINDOW, false) }?;

    let mut window = Window::default();

    // "'Setting the process-default DPI awareness via API call can lead to unexpected application behavior'... This is probably bullshit"
    // https://www.anthropicstudios.com/2022/01/13/asking-windows-nicely/#setting-dpi-awareness-programmatically
    unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) }?;

    let hwnd = unsafe {
        CreateWindowExA(
            WINDOW_EX_STYLE::default(),
            s!("Lockbook"),
            PCSTR(s!("Lockbook\0").as_ptr()),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            window_rect.right - window_rect.left,
            window_rect.bottom - window_rect.top,
            None,
            None,
            instance,
            Some(&mut window as *mut _ as _), // pass a pointer to our Window struct as the window's "user data"
        )
    };

    unsafe { dxgi_factory.MakeWindowAssociation(hwnd, DXGI_MWA_NO_ALT_ENTER) }?;

    let app = init(&crate::window::Window::new(hwnd), false);

    window.resources = Some(Resources { app });
    window.dpi_scale = dpi_to_scale_factor(unsafe { GetDpiForWindow(hwnd) } as _);

    // "If the window was previously visible, the return value is nonzero."
    // https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-showwindow
    unsafe { ShowWindow(hwnd, SW_SHOW) };

    let mut message = MSG::default();
    let mut last_frame = Instant::now();
    'event_loop: loop {
        while unsafe { PeekMessageA(&mut message, None, 0, 0, PM_REMOVE) }.into() {
            unsafe {
                // "If the message is translated [...], the return value is nonzero."
                // https://docs.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-translatemessage
                TranslateMessage(&message);

                // "...the return value generally is ignored."
                // https://docs.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-dispatchmessage
                DispatchMessageA(&message);
            }

            if message.message == WM_QUIT {
                break 'event_loop;
            }
        }

        // target framerate
        let frame_period = Duration::from_micros(8333);
        let now = Instant::now();
        let elapsed = now - last_frame;
        if elapsed < frame_period {
            std::thread::sleep(frame_period - elapsed);
        }
        last_frame = now;
    }

    Ok(())
}

// callback invoked when Windows sends a message to the window
extern "system" fn handle_messages(
    window_handle: HWND, message: u32, wparam: WPARAM, lparam: LPARAM,
) -> LRESULT {
    if handled_messages_impl(window_handle, message, wparam, lparam) {
        LRESULT::default()
    } else {
        // use the default handling for unhandled messages
        unsafe { DefWindowProcA(window_handle, message, wparam, lparam) }
    }
}

fn handled_messages_impl(
    window_handle: HWND, message: u32, wparam: WPARAM, lparam: LPARAM,
) -> bool {
    let message = Message::new(message, wparam, lparam);
    let mut handled = false;

    // events always processed
    handled |= match message {
        // Events processed even if we haven't initialized yet
        Message::Create { create_struct } => unsafe {
            SetWindowLongPtrA(window_handle, GWLP_USERDATA, create_struct.lpCreateParams as _);
            true
        },
        Message::Destroy => {
            unsafe { PostQuitMessage(0) };
            true
        }
        _ => false,
    };

    // get window
    let window = {
        // retrieve the pointer to our Window struct from the window's "user data"
        let user_data = unsafe { GetWindowLongPtrA(window_handle, GWLP_USERDATA) };
        let window = std::ptr::NonNull::<Window>::new(user_data as _);
        if let Some(mut window) = window {
            unsafe { window.as_mut() }
        } else {
            return handled;
        }
    };

    // events processed only after window is created
    handled |= match message {
        Message::Size { width, height } => {
            window.width = width;
            window.height = height;
            true
        }
        Message::DpiChanged { dpi, suggested_rect } => {
            // assign a scale factor based on the new DPI
            let new_scale_factor = dpi_to_scale_factor(dpi);
            window.dpi_scale = new_scale_factor;

            // resize the window based on Windows' suggestion
            window.width = (suggested_rect.right - suggested_rect.left) as _;
            window.height = (suggested_rect.bottom - suggested_rect.top) as _;

            true
        }
        _ => false,
    };

    // get app
    let app = {
        if let Some(resources) = &mut window.resources {
            &mut resources.app
        } else {
            return handled;
        }
    };

    // tell app about events it might have missed
    app.raw_input.pixels_per_point = Some(window.dpi_scale);
    app.screen.scale_factor = window.dpi_scale;
    app.screen.physical_width = window.width as _;
    app.screen.physical_height = window.height as _;

    // window doesn't receive key up messages when out of focus so we use GetKeyState instead
    // https://stackoverflow.com/questions/43858986/win32-keyboard-managing-when-key-released-while-not-focused
    let modifiers = egui::Modifiers {
        alt: unsafe { GetKeyState(VK_MENU.0 as i32) } & 0x8000u16 as i16 != 0,
        ctrl: unsafe { GetKeyState(VK_CONTROL.0 as i32) } & 0x8000u16 as i16 != 0,
        shift: unsafe { GetKeyState(VK_SHIFT.0 as i32) } & 0x8000u16 as i16 != 0,
        mac_cmd: false,
        command: unsafe { GetKeyState(VK_CONTROL.0 as i32) } & 0x8000u16 as i16 != 0,
    };
    app.raw_input.modifiers = modifiers;

    // events processed only after app is initialized
    handled |= match message {
        Message::KeyDown { key } | Message::KeyUp { key } => {
            let pressed = matches!(message, Message::KeyDown { .. });
            key_event(key, pressed, modifiers, app)
        }
        Message::LButtonDown { pos }
        | Message::LButtonUp { pos }
        | Message::RButtonDown { pos }
        | Message::RButtonUp { pos }
        | Message::MouseMove { pos } => {
            let pos = egui::Pos2 {
                x: pos.x as f32 / window.dpi_scale,
                y: pos.y as f32 / window.dpi_scale,
            };

            if matches!(message, Message::MouseMove { .. }) {
                app.raw_input.events.push(egui::Event::PointerMoved(pos));
            } else {
                let button =
                    if matches!(message, Message::LButtonDown { .. } | Message::LButtonUp { .. }) {
                        egui::PointerButton::Primary
                    } else {
                        egui::PointerButton::Secondary
                    };
                let pressed =
                    matches!(message, Message::LButtonDown { .. } | Message::RButtonDown { .. });
                pointer_button_event(pos, button, pressed, modifiers, app);
            }

            true
        }
        Message::PointerDown { pointer_id }
        | Message::PointerUpdate { pointer_id }
        | Message::PointerUp { pointer_id } => {
            input::pointer::handle(app, window_handle, modifiers, window.dpi_scale, pointer_id)
        }
        Message::MouseWheel { delta } => {
            let y = delta as f32 / WHEEL_DELTA as f32;
            let y = y * 20.0; // arbitrary multiplier to make scrolling feel better
            app.raw_input
                .events
                .push(egui::Event::Scroll(egui::Vec2 { x: 0.0, y }));

            true
        }
        Message::MouseHWheel { delta } => {
            let x = -delta as f32 / WHEEL_DELTA as f32;
            let x = x * 20.0; // arbitrary multiplier to make scrolling feel better
            app.raw_input
                .events
                .push(egui::Event::Scroll(egui::Vec2 { x, y: 0.0 }));

            true
        }
        Message::Paint => {
            app.frame();

            if let Some(copied_text) = mem::take(&mut app.from_egui) {
                set_clipboard(formats::Unicode, copied_text).expect("set clipboard");
            }

            true
        }
        _ => false,
    };

    handled
}

fn key_event(
    key: VIRTUAL_KEY, pressed: bool, modifiers: egui::Modifiers, app: &mut WgpuLockbook,
) -> bool {
    // text
    if pressed && (modifiers.shift_only() || modifiers.is_none()) {
        if let Some(text) = key::key_text(key, modifiers.shift) {
            app.raw_input
                .events
                .push(egui::Event::Text(text.to_owned()));
            return true;
        }
    }

    // todo: something feels weird about this
    if let Some(key) = key::egui_key(key) {
        // ctrl + v
        if pressed && key == egui::Key::V && modifiers.command {
            // somewhat weird that app.from_host isn't involved here
            let clipboard: String = get_clipboard(formats::Unicode).expect("get clipboard");
            app.raw_input.events.push(egui::Event::Paste(clipboard));
            return true;
        }

        // other egui keys
        app.raw_input
            .events
            .push(egui::Event::Key { key, pressed, repeat: false, modifiers });
        return true;
    }

    false
}

fn pointer_button_event(
    pos: egui::Pos2, button: egui::PointerButton, pressed: bool, modifiers: egui::Modifiers,
    app: &mut WgpuLockbook,
) {
    app.raw_input
        .events
        .push(egui::Event::PointerButton { pos, button, pressed, modifiers });
}

#[derive(Default)]
pub struct Window {
    resources: Option<Resources>,
    width: u16,
    height: u16,
    dpi_scale: f32,
}

// resources must be populated after the window is created
struct Resources {
    app: WgpuLockbook,
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
    let screen =
        ScreenDescriptor { physical_width: 1000, physical_height: 1000, scale_factor: 1.0 }; // initial value overridden by resize
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
    dpi as f32 / USER_DEFAULT_SCREEN_DPI as f32
}
