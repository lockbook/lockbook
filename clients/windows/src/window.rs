use crate::input;
use crate::input::{
    file_drop::FileDropHandler,
    message::{Message, MessageAppDep, MessageNoDeps, MessageWindowDep},
};
use crate::output;
use egui::{Context, PlatformOutput, Visuals};
use egui_wgpu_backend::{
    wgpu::{self, CompositeAlphaMode},
    ScreenDescriptor,
};
use lbeguiapp::{IntegrationOutput, UpdateOutput, WgpuLockbook};
use raw_window_handle::{
    HasRawDisplayHandle, HasRawWindowHandle, RawDisplayHandle, RawWindowHandle, Win32WindowHandle,
    WindowsDisplayHandle,
};
use std::time::Instant;
use windows::{
    core::*, Win32::Foundation::*, Win32::Graphics::Direct3D12::*, Win32::Graphics::Dxgi::*,
    Win32::Graphics::Gdi::*, Win32::System::LibraryLoader::*, Win32::System::Ole::*,
    Win32::UI::HiDpi::*, Win32::UI::Input::KeyboardAndMouse::*, Win32::UI::WindowsAndMessaging::*,
};

// todo: improve
// instead of redrawing at a particulr time in the future, we redraw it constantly until that time
static mut REDRAW_UNTIL: Option<Instant> = None;

pub struct WindowHandle {
    handle: Win32WindowHandle,
}

// Smails implementations adapted for windows with reference to winit's windows implementation:
// https://github.com/lockbook/lockbook/pull/1835/files#diff-0f28854a868a55fcd30ff5f0fda476aed540b2e1fc3762415ac6e0588ed76fb6
// https://github.com/rust-windowing/winit/blob/ee0db52ac49d64b46c500ef31d7f5f5107ce871a/src/platform_impl/windows/window.rs#L334-L346
impl WindowHandle {
    pub fn new(window: HWND) -> Self {
        let mut handle = Win32WindowHandle::empty();
        handle.hwnd = window.0 as *mut _;
        let hinstance = unsafe { get_window_long(window, GWLP_HINSTANCE) };
        handle.hinstance = hinstance as *mut _;

        Self { handle }
    }
}

unsafe impl HasRawWindowHandle for WindowHandle {
    fn raw_window_handle(&self) -> RawWindowHandle {
        RawWindowHandle::Win32(self.handle)
    }
}

unsafe impl HasRawDisplayHandle for WindowHandle {
    fn raw_display_handle(&self) -> RawDisplayHandle {
        RawDisplayHandle::Windows(WindowsDisplayHandle::empty())
    }
}

#[inline(always)]
unsafe fn get_window_long(hwnd: HWND, nindex: WINDOW_LONG_PTR_INDEX) -> isize {
    #[cfg(target_pointer_width = "64")]
    return unsafe { GetWindowLongPtrW(hwnd, nindex) };
    #[cfg(target_pointer_width = "32")]
    return unsafe { GetWindowLongW(hwnd, nindex) as isize };
}

// The rest of the code in this file would go in main except for this code to build on linux it needs to all be under a cfg(windows)
#[derive(Default)]
pub struct Window {
    maybe_app: Option<WgpuLockbook>, // must be populated after the window is created
    width: u16,
    height: u16,
    dpi_scale: f32,
}

pub fn main() -> Result<()> {
    env_logger::init();

    let instance = unsafe { GetModuleHandleA(None)? };

    let icon_bytes = include_bytes!("../lockbook.png");
    let icon = unsafe {
        CreateIconFromResourceEx(icon_bytes, true, 0x00030000, 128, 128, LR_DEFAULTCOLOR)
    }
    .expect("create icon");

    let wc = WNDCLASSEXA {
        cbSize: std::mem::size_of::<WNDCLASSEXA>() as u32,
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(handle_messages), // "Long Pointer to FuNction WiNDows PROCedure" (message handling callback)
        hInstance: instance.into(),
        hCursor: unsafe { LoadCursorW(None, IDC_ARROW) }?,
        lpszClassName: s!("Lockbook"),
        hIcon: icon,
        ..Default::default()
    };
    if unsafe { RegisterClassExA(&wc) } == 0 {
        println!("RegisterClassExA failed");
    }

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
            PCSTR(s!("Lockbook").as_ptr()),
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
    if let Err(error) = unsafe { GetLastError() } {
        print!("error: {}", error);
    }

    unsafe { dxgi_factory.MakeWindowAssociation(hwnd, DXGI_MWA_NO_ALT_ENTER) }?;

    let app = init(&WindowHandle::new(hwnd), false);
    app.context.set_request_repaint_callback(move |rri| {
        unsafe { InvalidateRect(hwnd, None, false) };
        unsafe { REDRAW_UNTIL = Some(Instant::now() + rri.after) };
    });
    window.maybe_app = Some(app);
    window.dpi_scale = dpi_to_scale_factor(unsafe { GetDpiForWindow(hwnd) } as _);

    // register file drop handler
    {
        unsafe { OleInitialize(None) }?;
        let file_drop_handler: IDropTarget = FileDropHandler {
            handler: Box::new(move |event| {
                handle_message(hwnd, Message::FileDrop(event));
            }),
        }
        .into();

        unsafe { RegisterDragDrop(hwnd, &file_drop_handler) }?;
        file_drop_handler
    };

    // "If the window was previously visible, the return value is nonzero."
    // https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-showwindow
    unsafe { ShowWindow(hwnd, SW_SHOW) };

    let mut message = MSG::default();
    // "If the function retrieves the WM_QUIT message, the return value is zero."
    // https://docs.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getmessagea
    while unsafe { GetMessageA(&mut message, None, 0, 0) }.into() {
        unsafe {
            // "If the message is translated [...], the return value is nonzero."
            // https://docs.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-translatemessage
            TranslateMessage(&message);

            // "...the return value generally is ignored."
            // https://docs.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-dispatchmessage
            DispatchMessageA(&message);
        }
    }

    Ok(())
}

// callback invoked when Windows sends a message to the window
extern "system" fn handle_messages(
    window_handle: HWND, message: u32, wparam: WPARAM, lparam: LPARAM,
) -> LRESULT {
    if handle_message_raw(window_handle, message, wparam, lparam) {
        LRESULT::default()
    } else {
        // use the default handling for unhandled messages
        unsafe { DefWindowProcA(window_handle, message, wparam, lparam) }
    }
}

fn handle_message_raw(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> bool {
    let message = Message::new(message, wparam, lparam);
    handle_message(hwnd, message)
}

fn handle_message(hwnd: HWND, message: Message) -> bool {
    // get window
    let mut maybe_window = {
        // retrieve the pointer to our Window struct from the window's "user data"
        let user_data = unsafe { GetWindowLongPtrA(hwnd, GWLP_USERDATA) };
        let window = std::ptr::NonNull::<Window>::new(user_data as _);
        if let Some(mut window) = window {
            Some(unsafe { window.as_mut() })
        } else {
            None
        }
    };

    // window doesn't receive key up messages when out of focus so we use GetKeyState instead
    // https://stackoverflow.com/questions/43858986/win32-keyboard-managing-when-key-released-while-not-focused
    let modifiers = egui::Modifiers {
        alt: unsafe { GetKeyState(VK_MENU.0 as i32) } & 0x8000u16 as i16 != 0,
        ctrl: unsafe { GetKeyState(VK_CONTROL.0 as i32) } & 0x8000u16 as i16 != 0,
        shift: unsafe { GetKeyState(VK_SHIFT.0 as i32) } & 0x8000u16 as i16 != 0,
        mac_cmd: false,
        command: unsafe { GetKeyState(VK_CONTROL.0 as i32) } & 0x8000u16 as i16 != 0,
    };

    match message {
        // events processed even if we haven't initialized yet
        Message::NoDeps(message) => {
            match message {
                MessageNoDeps::Create { create_struct } => unsafe {
                    SetWindowLongPtrA(hwnd, GWLP_USERDATA, create_struct.lpCreateParams as _);
                },
                MessageNoDeps::Destroy => {
                    unsafe { PostQuitMessage(0) };
                }
            }
            true
        }

        // events processed only after window is created
        Message::WindowDep(message) => {
            if let Some(ref mut window) = maybe_window {
                match message {
                    MessageWindowDep::Size { width, height } => {
                        window.width = width;
                        window.height = height;
                    }
                    MessageWindowDep::DpiChanged { dpi, suggested_rect } => {
                        // assign a scale factor based on the new DPI
                        let new_scale_factor = dpi_to_scale_factor(dpi);
                        window.dpi_scale = new_scale_factor;

                        // resize the window based on Windows' suggestion
                        window.width = (suggested_rect.right - suggested_rect.left) as _;
                        window.height = (suggested_rect.bottom - suggested_rect.top) as _;
                    }
                }
                true
            } else {
                false
            }
        }

        // events processed only after app is initialized
        Message::AppDep(message) => {
            if let Some(ref mut window) = maybe_window {
                if let Some(ref mut app) = window.maybe_app {
                    // events sent to app every frame
                    app.raw_input.pixels_per_point = Some(window.dpi_scale);
                    app.screen.scale_factor = window.dpi_scale;
                    app.screen.physical_width = window.width as _;
                    app.screen.physical_height = window.height as _;
                    app.raw_input.modifiers = modifiers;

                    match message {
                        MessageAppDep::KeyDown { key } | MessageAppDep::KeyUp { key } => {
                            unsafe { InvalidateRect(hwnd, None, false) };
                            input::key::handle(app, message, key, modifiers)
                        }
                        MessageAppDep::LButtonDown { pos }
                        | MessageAppDep::LButtonUp { pos }
                        | MessageAppDep::RButtonDown { pos }
                        | MessageAppDep::RButtonUp { pos }
                        | MessageAppDep::MouseMove { pos } => {
                            unsafe { InvalidateRect(hwnd, None, false) };
                            input::mouse::handle(app, message, pos, modifiers, window.dpi_scale)
                        }
                        MessageAppDep::PointerDown { pointer_id }
                        | MessageAppDep::PointerUpdate { pointer_id }
                        | MessageAppDep::PointerUp { pointer_id } => {
                            unsafe { InvalidateRect(hwnd, None, false) };
                            input::pointer::handle(
                                app,
                                hwnd,
                                modifiers,
                                window.dpi_scale,
                                pointer_id,
                            )
                        }
                        MessageAppDep::MouseWheel { delta }
                        | MessageAppDep::MouseHWheel { delta } => {
                            unsafe { InvalidateRect(hwnd, None, false) };
                            input::mouse::handle_wheel(app, message, delta)
                        }
                        MessageAppDep::SetCursor => output::cursor::handle(),
                        MessageAppDep::Paint => {
                            // "you'll find that your UI thread starts to burn 100% cpu core and your WM_PAINT handler
                            // getting called over and over again... WM_PAINT is generated as long as the window has a
                            // dirty rectangle, created by an InvalidateRect() call by either the window manager or
                            // your program explicitly calling it. BeginPaint() clears it."
                            // https://stackoverflow.com/questions/5841299/difference-between-getdc-and-beginpaint
                            unsafe { BeginPaint(hwnd, std::ptr::null_mut()) };

                            let IntegrationOutput {
                                redraw_in: _, // todo: handle? how's this different from checking egui context?
                                egui: PlatformOutput { cursor_icon, open_url, copied_text, .. },
                                update_output: UpdateOutput { close, set_window_title },
                            } = app.frame();

                            output::clipboard_copy::handle(copied_text);
                            output::close::handle(close);
                            output::window_title::handle(hwnd, set_window_title);
                            output::cursor::update(cursor_icon); // output saved and handled by 'SetCursor' message
                            output::open_url::handle(open_url);

                            unsafe { EndPaint(hwnd, std::ptr::null_mut()) };

                            if let Some(redraw_until) = unsafe { REDRAW_UNTIL } {
                                if redraw_until < Instant::now() {
                                    unsafe { REDRAW_UNTIL = None };
                                } else {
                                    // if a redraw is pending, invalidate the window so that we get another paint message
                                    unsafe { InvalidateRect(hwnd, None, false) };
                                }
                            }

                            true
                        }
                    }
                } else {
                    true
                }
            } else {
                true
            }
        }

        // events raised by drop handler
        // todo: set cursor to indicate drop is possible
        Message::FileDrop(message) => {
            if let Some(ref mut window) = maybe_window {
                if let Some(ref mut app) = window.maybe_app {
                    match message {
                        input::file_drop::Message::DragEnter { .. } => true,
                        input::file_drop::Message::DragOver { .. } => true,
                        input::file_drop::Message::DragLeave => true,
                        input::file_drop::Message::Drop { object, .. } => {
                            unsafe { InvalidateRect(hwnd, None, false) };
                            input::file_drop::handle(app, object)
                        }
                    }
                } else {
                    false
                }
            } else {
                false
            }
        }

        // remaining events
        Message::Unknown { .. } => false,
        Message::Unhandled { .. } => false,
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
        app,
        surface_width: 0,
        surface_height: 0,
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
