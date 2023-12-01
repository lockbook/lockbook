#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use crate::input::file_drop::FileDropHandler;
use egui::{Context, Visuals};
use egui_wgpu_backend::wgpu::CompositeAlphaMode;
use egui_wgpu_backend::{wgpu, ScreenDescriptor};
use input::message::{Message, MessageAppDep, MessageNoDeps, MessageWindowDep};
use lbeguiapp::{IntegrationOutput, UpdateOutput, WgpuLockbook};
use std::time::{Duration, Instant};
use windows::{
    core::*, Win32::Foundation::*, Win32::Graphics::Direct3D12::*, Win32::Graphics::Dxgi::*,
    Win32::System::Com::*, Win32::System::DataExchange::*, Win32::System::LibraryLoader::*,
    Win32::System::Memory::*, Win32::System::Ole::*, Win32::UI::HiDpi::*,
    Win32::UI::Input::KeyboardAndMouse::*, Win32::UI::Shell::*, Win32::UI::WindowsAndMessaging::*,
};

mod input;
mod output;
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

    unsafe { dxgi_factory.MakeWindowAssociation(hwnd, DXGI_MWA_NO_ALT_ENTER) }?;

    let app = init(&crate::window::Window::new(hwnd), false);
    app.context.set_request_repaint_callback(move |_rri| {
        // todo: fix this; makes the app laggy (unclear why)
        // thread::spawn(move || {
        //     // todo: verify thread safety or add a mutex
        //     thread::sleep(rri.after);
        //     unsafe {
        //         PostMessageA(hwnd, WM_PAINT, WPARAM(0), LPARAM(0))
        //             .expect("post paint message to self")
        //     };
        // });
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
                        | MessageAppDep::PointerUp { pointer_id } => input::pointer::handle(
                            app,
                            hwnd,
                            modifiers,
                            window.dpi_scale,
                            pointer_id,
                        ),
                        MessageAppDep::MouseWheel { delta }
                        | MessageAppDep::MouseHWheel { delta } => {
                            input::mouse::handle_wheel(app, message, delta)
                        }
                        MessageAppDep::Paint => {
                            let IntegrationOutput {
                                redraw_in: _, // todo: handle? how's this different from checking egui context?
                                update_output: UpdateOutput { close, set_window_title },
                            } = app.frame();

                            output::clipboard::handle(app);
                            output::close::handle(close);
                            output::window_title::handle(hwnd, set_window_title);

                            true
                        }
                    }
                } else {
                    false
                }
            } else {
                false
            }
        }

        // events raised by drop handler
        // todo: set cursor to indicate drop is possible
        Message::FileDrop(message) => match message {
            input::file_drop::Message::DragEnter { .. } => true,
            input::file_drop::Message::DragOver { .. } => true,
            input::file_drop::Message::DragLeave => true,
            input::file_drop::Message::Drop { object, .. } => {
                println!("--------------------------------------------------------------------------------");
                if let Some(object) = object {
                    let format_enumerator: IEnumFORMATETC = unsafe {
                        object
                            .EnumFormatEtc(DATADIR_GET.0 as _)
                            .expect("enumerate drop formats")
                    };
                    let mut rgelt = [FORMATETC::default(); 1];
                    loop {
                        let mut fetched: u32 = 0;
                        if unsafe { format_enumerator.Next(&mut rgelt, Some(&mut fetched as _)) }
                            .is_err()
                        {
                            println!("formats enumeration error");
                            break;
                        }
                        if fetched == 0 {
                            println!("no more formats");
                            break;
                        }

                        let format = CLIPBOARD_FORMAT(rgelt[0].cfFormat);
                        let mut format_name = [0u16; 1000];
                        let is_predefined_format = format_str(format).is_some();
                        let is_registered_format = unsafe {
                            GetClipboardFormatNameW(format.0 as _, &mut format_name) != 0
                        };
                        if !is_predefined_format && !is_registered_format {
                            println!("skipping unknown format: {:?}", format);
                            continue;
                        }
                        let format_name = String::from_utf16_lossy(&format_name);
                        println!(
                            "format: {} ({:?})",
                            format_name,
                            format_str(format).unwrap_or_default()
                        );

                        let stgm = unsafe { object.GetData(&rgelt[0]) }.expect("get drop data");

                        let tymed = TYMED(stgm.tymed as _);
                        if tymed_str(tymed).is_none() {
                            println!("skipping unknown tymed: {:?}", tymed);
                            continue;
                        }
                        println!("tymed: {:?}", tymed_str(tymed));

                        match tymed {
                            TYMED_HGLOBAL => {
                                let hglobal = unsafe { stgm.u.hGlobal };

                                // for unknown reasons, if I don't cast the HGLOBAL to an HDROP and query the file count, the next call to object.GetData fails
                                // (this applies even if the format isn't CF_HDROP)
                                let hdrop = HDROP(unsafe { std::mem::transmute(hglobal) });

                                if format == CF_HDROP {
                                    let file_count =
                                        unsafe { DragQueryFileW(hdrop, 0xFFFFFFFF, None) };
                                    println!("d");
                                    for i in 0..file_count {
                                        let mut file_name_bytes = [0u16; MAX_PATH as _];
                                        unsafe {
                                            DragQueryFileW(hdrop, i, Some(&mut file_name_bytes))
                                        };
                                        println!(
                                            "hdrop file path: {}",
                                            String::from_utf16_lossy(&file_name_bytes)
                                        );
                                    }
                                } else {
                                    let size = unsafe { GlobalSize(hglobal) };
                                    let mut bytes = vec![0u8; size as _];
                                    unsafe {
                                        std::ptr::copy_nonoverlapping(
                                            GlobalLock(hglobal),
                                            bytes.as_mut_ptr() as _,
                                            size as _,
                                        );
                                        let _ = GlobalUnlock(hglobal);
                                    }
                                    println!("global bytes: {}", String::from_utf8_lossy(&bytes));
                                }
                            }
                            TYMED_ISTREAM => {
                                if let Some(istream) = unsafe { stgm.u.pstm.as_ref() } {
                                    let mut bytes = [0u8; 1000];
                                    let mut read = 0;
                                    loop {
                                        let result = unsafe {
                                            istream.Read(
                                                &mut bytes[read..] as *mut _ as _,
                                                (bytes.len() - read) as _,
                                                Some(&mut read as *mut _ as _),
                                            )
                                        };
                                        if result.is_err() {
                                            println!("TyMed IStream read error: {:?}", result);
                                            break;
                                        }
                                        if read == 0 {
                                            break;
                                        }
                                    }
                                    println!(
                                        "stream bytes: {}",
                                        String::from_utf8_lossy(&bytes).len()
                                    );
                                }
                            }
                            _ => {}
                        }
                    }
                }
                true
            }
        },

        // remaining events
        Message::Unknown { .. } => false,
        Message::Unhandled { .. } => false,
    }
}

fn tymed_str(tymed: TYMED) -> Option<&'static str> {
    match tymed {
        TYMED_HGLOBAL => Some("TYMED_HGLOBAL"),
        TYMED_FILE => Some("TYMED_FILE"),
        TYMED_ISTREAM => Some("TYMED_ISTREAM"),
        TYMED_ISTORAGE => Some("TYMED_ISTORAGE"),
        TYMED_GDI => Some("TYMED_GDI"),
        TYMED_MFPICT => Some("TYMED_MFPICT"),
        TYMED_ENHMF => Some("TYMED_ENHMF"),
        TYMED_NULL => Some("TYMED_NULL"),
        _ => None,
    }
}

fn format_str(format: CLIPBOARD_FORMAT) -> Option<&'static str> {
    match format {
        CF_TEXT => Some("CF_TEXT"),
        CF_BITMAP => Some("CF_BITMAP"),
        CF_METAFILEPICT => Some("CF_METAFILEPICT"),
        CF_SYLK => Some("CF_SYLK"),
        CF_DIF => Some("CF_DIF"),
        CF_TIFF => Some("CF_TIFF"),
        CF_OEMTEXT => Some("CF_OEMTEXT"),
        CF_DIB => Some("CF_DIB"),
        CF_PALETTE => Some("CF_PALETTE"),
        CF_PENDATA => Some("CF_PENDATA"),
        CF_RIFF => Some("CF_RIFF"),
        CF_WAVE => Some("CF_WAVE"),
        CF_UNICODETEXT => Some("CF_UNICODETEXT"),
        CF_ENHMETAFILE => Some("CF_ENHMETAFILE"),
        CF_HDROP => Some("CF_HDROP"),
        CF_LOCALE => Some("CF_LOCALE"),
        CF_DIBV5 => Some("CF_DIBV5"),
        CF_OWNERDISPLAY => Some("CF_OWNERDISPLAY"),
        CF_DSPTEXT => Some("CF_DSPTEXT"),
        CF_DSPBITMAP => Some("CF_DSPBITMAP"),
        CF_DSPMETAFILEPICT => Some("CF_DSPMETAFILEPICT"),
        CF_DSPENHMETAFILE => Some("CF_DSPENHMETAFILE"),
        _ => None,
    }
}

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

pub fn string_from_utf16_bytes(slice: &[u8]) -> String {
    let size = slice.len() / 2;
    let iter = (0..size).map(|i| u16::from_be_bytes([slice[2 * i], slice[2 * i + 1]]));

    std::char::decode_utf16(iter)
        .collect::<std::result::Result<String, _>>()
        .expect("utf-16 string")
}
