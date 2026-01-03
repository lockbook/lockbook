use crate::input::file_drop::FileDropHandler;
use crate::input::message::{Message, MessageAppDep, MessageNoDeps, MessageWindowDep};
use crate::{input, output};
use egui::{PlatformOutput, ViewportCommand, Visuals};
use egui_wgpu_renderer::RendererState;
use lbeguiapp::{Output, Response, WgpuLockbook};
use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, RawDisplayHandle,
    RawWindowHandle, Win32WindowHandle, WindowHandle, WindowsDisplayHandle,
};
use std::num::NonZeroIsize;
use std::sync::atomic::{AtomicBool, Ordering};
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::*;
use windows::Win32::System::Ole::*;
use windows::Win32::UI::HiDpi::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::*;

#[derive(Default)]
struct Window<'window> {
    maybe_app: Option<WgpuLockbook<'window>>, // app is initialized after window is created
    pointer_manager: input::pointer::PointerManager,
    width: u16,
    height: u16,
    dpi_scale: f32,
}

struct AppWindowHandle {
    window: Win32WindowHandle,
}

unsafe impl Sync for AppWindowHandle {} // window is never actually sent across threads

impl AppWindowHandle {
    fn new(window: HWND) -> Self {
        let mut handle = Win32WindowHandle::new(NonZeroIsize::new(window.0).unwrap());
        let hinstance = unsafe { get_window_long(window, GWLP_HINSTANCE) };
        handle.hinstance = NonZeroIsize::new(hinstance as _);
        Self { window: handle }
    }
}

impl HasWindowHandle for AppWindowHandle {
    fn window_handle(&self) -> std::result::Result<WindowHandle<'_>, HandleError> {
        unsafe { Ok(WindowHandle::borrow_raw(RawWindowHandle::Win32(self.window))) }
    }
}

impl HasDisplayHandle for AppWindowHandle {
    fn display_handle(&self) -> std::result::Result<DisplayHandle<'_>, HandleError> {
        unsafe {
            Ok(DisplayHandle::borrow_raw(RawDisplayHandle::Windows(WindowsDisplayHandle::new())))
        }
    }
}

#[inline(always)]
unsafe fn get_window_long(hwnd: HWND, nindex: WINDOW_LONG_PTR_INDEX) -> isize {
    #[cfg(target_pointer_width = "64")]
    return unsafe { GetWindowLongPtrW(hwnd, nindex) };
    #[cfg(target_pointer_width = "32")]
    return unsafe { GetWindowLongW(hwnd, nindex) as isize };
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

    // must declare maybe_window_handle before window for it to drop after window, sating the borrow checker
    #[allow(unused_assignments)]
    let mut maybe_window_handle = None;
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
        print!("error: {error}");
    }

    unsafe { dxgi_factory.MakeWindowAssociation(hwnd, DXGI_MWA_NO_ALT_ENTER) }?;

    let got_events_atomic = std::sync::Arc::new(AtomicBool::new(false));
    let got_events_clone = got_events_atomic.clone();

    maybe_window_handle = Some(AppWindowHandle::new(hwnd));
    window.maybe_app = {
        let scale_factor = dpi_to_scale_factor(unsafe { GetDpiForWindow(hwnd) } as _);
        let app = init(maybe_window_handle.as_ref().unwrap(), false);
        app.renderer.context.set_pixels_per_point(scale_factor);
        window.dpi_scale = scale_factor;

        app.renderer
            .context
            .set_request_repaint_callback(move |rri| {
                let got_events_clone = got_events_clone.clone();
                let _ = std::thread::spawn(move || {
                    std::thread::sleep(rri.delay);
                    got_events_clone.store(true, Ordering::SeqCst);
                });
            });

        Some(app)
    };

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
    unsafe { ShowWindow(hwnd, SW_SHOW) };

    let mut messages = Vec::new();
    let mut msg = MSG::default();
    'outer: loop {
        let mut got_events = got_events_atomic.load(Ordering::SeqCst);
        unsafe {
            while PeekMessageA(&mut msg, HWND(0), 0, 0, PM_REMOVE).into() {
                if msg.message == WM_QUIT {
                    break 'outer;
                }

                messages.push(msg);

                if msg.message == WM_PAINT {
                    break;
                }

                got_events = true;
            }

            for msg in messages.drain(..) {
                TranslateMessage(&msg);
                DispatchMessageA(&msg);
            }

            if got_events {
                got_events_atomic.store(false, Ordering::SeqCst);
                InvalidateRect(hwnd, None, false);
            }

            // wait a lil before possibly rendering another frame
            std::thread::sleep(std::time::Duration::from_millis(1));
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
        if let Some(mut window) = window { Some(unsafe { window.as_mut() }) } else { None }
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
                MessageNoDeps::Quit => {
                    unsafe { DestroyWindow(hwnd).expect("destroy window") };
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
                    app.renderer.context.set_pixels_per_point(window.dpi_scale);
                    app.renderer.screen.pixels_per_point = window.dpi_scale;
                    app.renderer.screen.size_in_pixels[0] = window.width as _;
                    app.renderer.screen.size_in_pixels[1] = window.height as _;
                    app.renderer.raw_input.modifiers = modifiers;

                    match message {
                        MessageAppDep::KeyDown { key } | MessageAppDep::KeyUp { key } => {
                            input::key::handle(app, message, key, modifiers)
                        }
                        MessageAppDep::LButtonDown { pos }
                        | MessageAppDep::LButtonUp { pos }
                        | MessageAppDep::RButtonDown { pos }
                        | MessageAppDep::RButtonUp { pos }
                        | MessageAppDep::MouseMove { pos } => {
                            input::mouse::handle(app, message, pos, modifiers, window.dpi_scale)
                        }
                        MessageAppDep::PointerDown { pointer_id }
                        | MessageAppDep::PointerUpdate { pointer_id }
                        | MessageAppDep::PointerUp { pointer_id } => window.pointer_manager.handle(
                            app,
                            hwnd,
                            modifiers,
                            window.dpi_scale,
                            pointer_id,
                        ),
                        MessageAppDep::MouseWheel { delta }
                        | MessageAppDep::MouseHWheel { delta } => {
                            input::mouse::handle_wheel(app, message, delta, modifiers)
                        }
                        MessageAppDep::SetCursor => output::cursor::handle(),
                        MessageAppDep::Paint => {
                            // "you'll find that your UI thread starts to burn 100% cpu core and your WM_PAINT handler
                            // getting called over and over again... WM_PAINT is generated as long as the window has a
                            // dirty rectangle, created by an InvalidateRect() call by either the window manager or
                            // your program explicitly calling it. BeginPaint() clears it."
                            // https://stackoverflow.com/questions/5841299/difference-between-getdc-and-beginpaint
                            unsafe { BeginPaint(hwnd, std::ptr::null_mut()) };

                            let Output {
                                platform: PlatformOutput { cursor_icon, open_url, copied_text, .. },
                                viewport,
                                app: Response { close },
                            } = app.frame();

                            let mut redraw_in = None;
                            let mut window_title = None;
                            let mut request_paste = false;
                            if let Some(viewport) = viewport.into_values().next() {
                                redraw_in = Some(viewport.repaint_delay.as_millis() as _);
                                for cmd in viewport.commands.into_iter() {
                                    match cmd {
                                        ViewportCommand::Title(title) => window_title = Some(title),
                                        ViewportCommand::RequestPaste => request_paste = true,
                                        _ => {} // remaining viewport commands ignored (many such cases!)
                                    }
                                }
                            } else {
                                eprintln!(
                                    "viewport missing: not redrawing or setting window title"
                                );
                            }
                            let _: Option<u64> = redraw_in; // todo: use; unclear how this app works at all without it

                            if output::clipboard_copy::handle(copied_text.clone()).is_err() {
                                // windows clipboard sometimes has transient errors
                                app.renderer
                                    .context
                                    .output_mut(|o| o.copied_text = copied_text);
                            }
                            if request_paste {
                                input::clipboard_paste::handle(app);
                            }
                            output::close::handle(close);
                            output::window_title::handle(hwnd, window_title);
                            output::cursor::update(cursor_icon); // output saved and handled by 'SetCursor' message
                            output::open_url::handle(open_url);

                            unsafe { EndPaint(hwnd, std::ptr::null_mut()) };

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
pub fn init<W: raw_window_handle::HasWindowHandle + raw_window_handle::HasDisplayHandle + Sync>(
    window: &W, dark_mode: bool,
) -> WgpuLockbook<'_> {
    let renderer = RendererState::init_window(window);
    renderer
        .context
        .set_visuals(if dark_mode { Visuals::dark() } else { Visuals::light() });

    let app = lbeguiapp::Lockbook::new(&renderer.context);

    let mut obj = WgpuLockbook {
        renderer,
        queued_events: Default::default(),
        double_queued_events: Default::default(),
        app,
    };

    obj.frame();

    obj
}

// https://github.com/rust-windowing/winit/blob/789a4979801cffc20c9dfbc34e72c15ebf3c737c/src/platform_impl/windows/dpi.rs#L75C1-L78C2
pub fn dpi_to_scale_factor(dpi: u16) -> f32 {
    dpi as f32 / USER_DEFAULT_SCREEN_DPI as f32
}
