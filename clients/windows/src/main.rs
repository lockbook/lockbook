use egui::{Context, Visuals};
use egui_wgpu_backend::wgpu::CompositeAlphaMode;
use egui_wgpu_backend::{wgpu, ScreenDescriptor};
use lbeditor::{Editor, WgpuEditor};
use raw_window_handle::{
    HasRawDisplayHandle, HasRawWindowHandle, RawDisplayHandle, RawWindowHandle, Win32WindowHandle,
    WindowsDisplayHandle,
};
use std::mem::transmute;
use std::time::{Duration, Instant};
use windows::{
    core::*, Win32::Foundation::*, Win32::Graphics::Direct3D12::*, Win32::Graphics::Dxgi::*,
    Win32::System::LibraryLoader::*, Win32::UI::HiDpi::*, Win32::UI::WindowsAndMessaging::*,
};

fn main() -> Result<()> {
    env_logger::init();

    let instance = unsafe { GetModuleHandleA(None)? };

    let wc = WNDCLASSEXA {
        cbSize: std::mem::size_of::<WNDCLASSEXA>() as u32,
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(handle_messages), // "Long Pointer to FuNction WiNDows PROCedure" (message handling callback)
        hInstance: instance.into(),
        hCursor: unsafe { LoadCursorW(None, IDC_ARROW)? },
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
    let mut window = Window::default();

    let mut window_rect = RECT { left: 0, top: 0, right: 1000, bottom: 1000 };
    if unsafe { AdjustWindowRect(&mut window_rect, WS_OVERLAPPEDWINDOW, false) }
        == windows::Win32::Foundation::FALSE
    {
        // "If the function succeeds, the return value is nonzero."
        // https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-adjustwindowrect
        println!("AdjustWindowRect failed: {:?}", unsafe { GetLastError() });
        unsafe { PostQuitMessage(1) };
    };

    // "This function returns TRUE if the operation was successful, and FALSE otherwise"
    // https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setprocessdpiawarenesscontext
    // "Setting the process-default DPI awareness via API call can lead to unexpected application behavior... This is probably bullshit"
    // https://www.anthropicstudios.com/2022/01/13/asking-windows-nicely/#setting-dpi-awareness-programmatically
    if unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) }
        == windows::Win32::Foundation::FALSE
    {
        println!("SetProcessDpiAwarenessContext failed: {:?}", unsafe { GetLastError() });
        unsafe { PostQuitMessage(1) };
    }

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

    unsafe {
        dxgi_factory.MakeWindowAssociation(hwnd, DXGI_MWA_NO_ALT_ENTER)?;
    }

    let mut core = lb::Core::init(&lb::Config {
        logs: false,
        colored_logs: false,
        writeable_path: format!(
            "{}/.lockbook/cli",
            std::env::var("HOME").unwrap_or(".".to_string())
        ),
    })
    .unwrap();
    let editor = init_editor(&mut core, &WindowWrapper::new(hwnd), false);

    window.resources = Some(Resources { editor });

    // "If the window was previously visible, the return value is nonzero."
    // https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-showwindow
    unsafe { ShowWindow(hwnd, SW_SHOW) };

    let mut message = MSG::default();
    let mut last_frame = Instant::now();
    loop {
        if unsafe { PeekMessageA(&mut message, None, 0, 0, PM_REMOVE) }.into() {
            unsafe {
                // "If the message is translated [...], the return value is nonzero."
                // https://docs.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-translatemessage
                TranslateMessage(&message);

                // "...the return value generally is ignored."
                // https://docs.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-dispatchmessage
                DispatchMessageA(&message);
            }

            if message.message == WM_QUIT {
                break;
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
    match message {
        WM_CREATE => {
            unsafe {
                let create_struct: &CREATESTRUCTA = transmute(lparam);
                SetWindowLongPtrA(window_handle, GWLP_USERDATA, create_struct.lpCreateParams as _);
            }
            LRESULT::default()
        }
        WM_DESTROY => {
            unsafe { PostQuitMessage(0) };
            LRESULT::default()
        }
        _ => {
            // retrieve the pointer to our Window struct from the window's "user data"
            let user_data = unsafe { GetWindowLongPtrA(window_handle, GWLP_USERDATA) };
            let window = std::ptr::NonNull::<Window>::new(user_data as _);
            let handled = if let Some(mut window) = window {
                let window = unsafe { window.as_mut() };
                if let Some(resources) = &mut window.resources {
                    let editor = &mut resources.editor;
                    // winit used as reference for interesting events
                    // https://github.com/rust-windowing/winit/blob/789a4979801cffc20c9dfbc34e72c15ebf3c737c/src/platform_impl/windows/event_loop.rs#L1071
                    match message {
                        WM_KEYDOWN => {
                            // todo: handle keydown
                            true
                        }
                        WM_KEYUP => {
                            // todo: handle keyup
                            true
                        }
                        WM_SIZE => {
                            editor.screen.physical_width = loword(lparam.0 as u32) as u32;
                            editor.screen.physical_height = hiword(lparam.0 as u32) as u32;
                            true
                        }
                        WM_DPICHANGED => {
                            // convert the new DPI to a scale factor
                            let new_dpi_x = loword(wparam.0 as u32);
                            let new_scale_factor = dpi_to_scale_factor(new_dpi_x);
                            editor.screen.scale_factor = new_scale_factor;

                            // resize the window based on Window's suggestion
                            let suggested_rect = unsafe { *(lparam.0 as *const RECT) };
                            editor.screen.physical_width =
                                (suggested_rect.right - suggested_rect.left) as u32;
                            editor.screen.physical_height =
                                (suggested_rect.bottom - suggested_rect.top) as u32;

                            true
                        }
                        // WM_MOUSEMOVE | WM_MOUSELEAVE | WM_MOUSEWHEEL | WM_MOUSEHWHEEL
                        // | WM_LBUTTONDOWN | WM_LBUTTONUP | WM_RBUTTONDOWN | WM_RBUTTONUP => {
                        //     todo!("handle mouse events");
                        // }
                        WM_TOUCH => {
                            todo!("handle touch events");
                        }
                        WM_POINTERDOWN | WM_POINTERUPDATE | WM_POINTERUP => {
                            todo!("handle pointer events"); // how are these different from touch and mouse events?
                        }
                        WM_PAINT => {
                            editor.frame();
                            true
                        }
                        _ => false,
                    }
                } else {
                    false
                }
            } else {
                false
            };

            if handled {
                LRESULT::default()
            } else {
                // use the default handling for messages we don't care about
                unsafe { DefWindowProcA(window_handle, message, wparam, lparam) }
            }
        }
    }
}

#[derive(Default)]
pub struct Window {
    resources: Option<Resources>,
}

// resources must be populated after the window is created
struct Resources {
    editor: lbeditor::WgpuEditor,
}

// Taken from other lockbook code
pub fn init_editor<
    W: raw_window_handle::HasRawWindowHandle + raw_window_handle::HasRawDisplayHandle,
>(
    core: &mut lb::Core, window: &W, dark_mode: bool,
) -> WgpuEditor {
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
    let mut editor = Editor::new(core.clone());
    editor.set_font(&context);
    editor.buffer = "# hello from editor".into();

    let start_time = Instant::now();
    let mut obj = WgpuEditor {
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
        editor,
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

// NativeWindow taken from Smail's android PR:
// https://github.com/lockbook/lockbook/pull/1835/files#diff-0f28854a868a55fcd30ff5f0fda476aed540b2e1fc3762415ac6e0588ed76fb6
pub struct WindowWrapper {
    handle: Win32WindowHandle,
}

// Smails implementations adapted for windows with reference to winit's windows implementation:
// https://github.com/rust-windowing/winit/blob/ee0db52ac49d64b46c500ef31d7f5f5107ce871a/src/platform_impl/windows/window.rs#L334-L346
impl WindowWrapper {
    pub fn new(window: HWND) -> Self {
        let mut handle = Win32WindowHandle::empty();
        handle.hwnd = window.0 as *mut _;
        let hinstance = unsafe { get_window_long(window, GWLP_HINSTANCE) };
        handle.hinstance = hinstance as *mut _;

        Self { handle }
    }
}

unsafe impl HasRawWindowHandle for WindowWrapper {
    fn raw_window_handle(&self) -> RawWindowHandle {
        RawWindowHandle::Win32(self.handle)
    }
}

unsafe impl HasRawDisplayHandle for WindowWrapper {
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

// https://github.com/rust-windowing/winit/blob/789a4979801cffc20c9dfbc34e72c15ebf3c737c/src/platform_impl/windows/mod.rs#L144C1-L152C2
#[inline(always)]
pub(crate) const fn loword(x: u32) -> u16 {
    (x & 0xFFFF) as u16
}

#[inline(always)]
const fn hiword(x: u32) -> u16 {
    ((x >> 16) & 0xFFFF) as u16
}

// https://github.com/rust-windowing/winit/blob/789a4979801cffc20c9dfbc34e72c15ebf3c737c/src/platform_impl/windows/dpi.rs#L75C1-L78C2
pub fn dpi_to_scale_factor(dpi: u16) -> f32 {
    dpi as f32 / USER_DEFAULT_SCREEN_DPI as f32
}
