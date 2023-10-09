use egui::{Context, Visuals};
use egui_wgpu_backend::wgpu::CompositeAlphaMode;
use egui_wgpu_backend::{wgpu, ScreenDescriptor};
use lbeditor::{Editor, WgpuEditor};
use raw_window_handle::{
    HasRawDisplayHandle, HasRawWindowHandle, RawDisplayHandle, RawWindowHandle, Win32WindowHandle,
    WindowsDisplayHandle,
};
use std::ffi::c_void;
use std::time::Instant;
use windows::core::ComInterface;
use windows::{
    Win32, Win32::Graphics::Direct2D, Win32::Graphics::Direct3D, Win32::Graphics::Direct3D11,
    Win32::Graphics::Dxgi, Win32::Graphics::Gdi, Win32::System::Com, Win32::System::LibraryLoader,
    Win32::UI::WindowsAndMessaging,
};

// taken from windows-rs examples
fn main() -> windows::core::Result<()> {
    unsafe {
        Com::CoInitializeEx(None, Com::COINIT_MULTITHREADED)?;
    }
    let mut window = Window::new()?;
    window.run()
}

struct Window {
    handle: Win32::Foundation::HWND,
    factory: Direct2D::ID2D1Factory1,
    dxfactory: Dxgi::IDXGIFactory2,

    target: Option<Direct2D::ID2D1DeviceContext>,
    swapchain: Option<Dxgi::IDXGISwapChain1>,
    bitmap: Option<Direct2D::ID2D1Bitmap1>,
    dpi: f32,
    visible: bool,
    occlusion: u32,

    editor: Option<WgpuEditor>,
}

impl Window {
    fn new() -> windows::core::Result<Self> {
        let factory = create_factory()?;
        let dxfactory: Dxgi::IDXGIFactory2 = unsafe { Dxgi::CreateDXGIFactory1()? };

        let mut dpi = 0.0;
        let mut dpiy = 0.0;
        unsafe { factory.GetDesktopDpi(&mut dpi, &mut dpiy) };

        Ok(Window {
            handle: Win32::Foundation::HWND(0),
            factory,
            dxfactory,
            target: None,
            swapchain: None,
            bitmap: None,
            dpi,
            visible: false,
            occlusion: 0,
            editor: None,
        })
    }

    fn render(&mut self) -> windows::core::Result<()> {
        if self.target.is_none() {
            let device = create_device()?;
            let target = create_render_target(&self.factory, &device)?;
            unsafe { target.SetDpi(self.dpi, self.dpi) };

            let swapchain = create_swapchain(&device, self.handle)?;
            create_swapchain_bitmap(&swapchain, &target)?;

            let bitmap = {
                let size_f = unsafe { target.GetSize() };

                let size_u = Direct2D::Common::D2D_SIZE_U {
                    width: (size_f.width * self.dpi / 96.0) as u32,
                    height: (size_f.height * self.dpi / 96.0) as u32,
                };

                let properties = Direct2D::D2D1_BITMAP_PROPERTIES1 {
                    pixelFormat: Direct2D::Common::D2D1_PIXEL_FORMAT {
                        format: Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM,
                        alphaMode: Direct2D::Common::D2D1_ALPHA_MODE_PREMULTIPLIED,
                    },
                    dpiX: self.dpi,
                    dpiY: self.dpi,
                    bitmapOptions: Direct2D::D2D1_BITMAP_OPTIONS_TARGET,
                    ..Default::default()
                };

                unsafe { target.CreateBitmap2(size_u, None, 0, &properties) }
            }?;

            self.target = Some(target);
            self.swapchain = Some(swapchain);
            self.bitmap = Some(bitmap);

            let mut core = lb::Core::init(&lb::Config {
                logs: false,
                colored_logs: false,
                writeable_path: format!(
                    "{}/.lockbook/cli",
                    std::env::var("HOME").unwrap_or(".".to_string())
                ),
            })
            .unwrap();
            let native_window = NativeWindow::new(self.handle);
            self.editor = Some(init_editor(&mut core, &native_window, false));
        }

        self.draw().unwrap();

        if let Err(error) = self.present(1, 0) {
            if error.code() == Win32::Foundation::DXGI_STATUS_OCCLUDED {
                self.occlusion = unsafe {
                    self.dxfactory
                        .RegisterOcclusionStatusWindow(self.handle, WindowsAndMessaging::WM_USER)?
                };
                self.visible = false;
            } else {
                self.release_device();
            }
        }

        Ok(())
    }

    fn draw(&mut self) -> windows::core::Result<()> {
        let target = self.target.as_ref().unwrap();
        let bitmap = self.bitmap.as_ref().unwrap();

        unsafe {
            target.BeginDraw();

            target.Clear(Some(&Direct2D::Common::D2D1_COLOR_F { r: 1.0, g: 1.0, b: 1.0, a: 1.0 }));

            let previous = target.GetTarget()?;
            target.SetTarget(bitmap);
            target.Clear(None);

            if let Some(editor) = &mut self.editor {
                editor.frame();
            };

            target.SetTarget(&previous);

            target.DrawImage(
                bitmap,
                None,
                None,
                Direct2D::D2D1_INTERPOLATION_MODE_LINEAR,
                Direct2D::Common::D2D1_COMPOSITE_MODE_SOURCE_OVER,
            );

            target.EndDraw(None, None)?;
        }

        Ok(())
    }

    fn release_device(&mut self) {
        self.target = None;
        self.swapchain = None;
    }

    fn present(&self, sync: u32, flags: u32) -> windows::core::Result<()> {
        unsafe { self.swapchain.as_ref().unwrap().Present(sync, flags).ok() }
    }

    fn resize_swapchain_bitmap(&mut self) -> windows::core::Result<()> {
        if let Some(target) = &self.target {
            let swapchain = self.swapchain.as_ref().unwrap();
            unsafe { target.SetTarget(None) };

            if unsafe {
                swapchain
                    .ResizeBuffers(0, 0, 0, Dxgi::Common::DXGI_FORMAT_UNKNOWN, 0)
                    .is_ok()
            } {
                create_swapchain_bitmap(swapchain, target)?;
            } else {
                self.release_device();
            }

            self.render()?;
        }

        Ok(())
    }

    fn message_handler(
        &mut self, message: u32, wparam: Win32::Foundation::WPARAM,
        lparam: Win32::Foundation::LPARAM,
    ) -> Win32::Foundation::LRESULT {
        unsafe {
            match message {
                WindowsAndMessaging::WM_PAINT => {
                    let mut ps = Gdi::PAINTSTRUCT::default();
                    Gdi::BeginPaint(self.handle, &mut ps);
                    self.render().unwrap();
                    Gdi::EndPaint(self.handle, &ps);
                    Win32::Foundation::LRESULT(0)
                }
                WindowsAndMessaging::WM_SIZE => {
                    if wparam.0 != WindowsAndMessaging::SIZE_MINIMIZED as usize {
                        self.resize_swapchain_bitmap().unwrap();
                    }
                    Win32::Foundation::LRESULT(0)
                }
                WindowsAndMessaging::WM_DISPLAYCHANGE => {
                    self.render().unwrap();
                    Win32::Foundation::LRESULT(0)
                }
                WindowsAndMessaging::WM_USER => {
                    if self.present(0, Dxgi::DXGI_PRESENT_TEST).is_ok() {
                        self.dxfactory.UnregisterOcclusionStatus(self.occlusion);
                        self.occlusion = 0;
                        self.visible = true;
                    }
                    Win32::Foundation::LRESULT(0)
                }
                WindowsAndMessaging::WM_ACTIVATE => {
                    self.visible = true; // TODO: unpack !HIWORD(wparam);
                    Win32::Foundation::LRESULT(0)
                }
                WindowsAndMessaging::WM_DESTROY => {
                    WindowsAndMessaging::PostQuitMessage(0);
                    Win32::Foundation::LRESULT(0)
                }
                _ => WindowsAndMessaging::DefWindowProcA(self.handle, message, wparam, lparam),
            }
        }
    }

    fn run(&mut self) -> windows::core::Result<()> {
        unsafe {
            let instance = LibraryLoader::GetModuleHandleA(None)?;
            debug_assert!(instance.0 != 0);
            let window_class = windows::core::s!("window");

            let wc = WindowsAndMessaging::WNDCLASSA {
                hCursor: WindowsAndMessaging::LoadCursorW(None, WindowsAndMessaging::IDC_HAND)?,
                hInstance: instance.into(),
                lpszClassName: window_class,

                style: WindowsAndMessaging::CS_HREDRAW | WindowsAndMessaging::CS_VREDRAW,
                lpfnWndProc: Some(Self::wndproc),
                ..Default::default()
            };

            let atom = WindowsAndMessaging::RegisterClassA(&wc);
            debug_assert!(atom != 0);

            let handle = WindowsAndMessaging::CreateWindowExA(
                WindowsAndMessaging::WINDOW_EX_STYLE::default(),
                window_class,
                windows::core::s!("Sample Window"),
                WindowsAndMessaging::WS_OVERLAPPEDWINDOW | WindowsAndMessaging::WS_VISIBLE,
                WindowsAndMessaging::CW_USEDEFAULT,
                WindowsAndMessaging::CW_USEDEFAULT,
                WindowsAndMessaging::CW_USEDEFAULT,
                WindowsAndMessaging::CW_USEDEFAULT,
                None,
                None,
                instance,
                Some(self as *mut _ as _),
            );

            debug_assert!(handle.0 != 0);
            debug_assert!(handle == self.handle);
            let mut message = WindowsAndMessaging::MSG::default();

            loop {
                if self.visible {
                    self.render()?;

                    while WindowsAndMessaging::PeekMessageA(
                        &mut message,
                        None,
                        0,
                        0,
                        WindowsAndMessaging::PM_REMOVE,
                    )
                    .into()
                    {
                        if message.message == WindowsAndMessaging::WM_QUIT {
                            return Ok(());
                        }
                        WindowsAndMessaging::DispatchMessageA(&message);
                    }
                } else {
                    WindowsAndMessaging::GetMessageA(&mut message, None, 0, 0);

                    if message.message == WindowsAndMessaging::WM_QUIT {
                        return Ok(());
                    }

                    WindowsAndMessaging::DispatchMessageA(&message);
                }
            }
        }
    }

    extern "system" fn wndproc(
        window: Win32::Foundation::HWND, message: u32, wparam: Win32::Foundation::WPARAM,
        lparam: Win32::Foundation::LPARAM,
    ) -> Win32::Foundation::LRESULT {
        unsafe {
            if message == WindowsAndMessaging::WM_NCCREATE {
                let cs = lparam.0 as *const WindowsAndMessaging::CREATESTRUCTA;
                let this = (*cs).lpCreateParams as *mut Self;
                (*this).handle = window;

                WindowsAndMessaging::SetWindowLongPtrA(
                    window,
                    WindowsAndMessaging::GWLP_USERDATA,
                    this as _,
                );
            } else {
                let this = WindowsAndMessaging::GetWindowLongPtrA(
                    window,
                    WindowsAndMessaging::GWLP_USERDATA,
                ) as *mut Self;

                if !this.is_null() {
                    return (*this).message_handler(message, wparam, lparam);
                }
            }

            WindowsAndMessaging::DefWindowProcA(window, message, wparam, lparam)
        }
    }
}

fn create_factory() -> windows::core::Result<Direct2D::ID2D1Factory1> {
    let mut options = Direct2D::D2D1_FACTORY_OPTIONS::default();

    if cfg!(debug_assertions) {
        options.debugLevel = Direct2D::D2D1_DEBUG_LEVEL_INFORMATION;
    }

    unsafe {
        Direct2D::D2D1CreateFactory(Direct2D::D2D1_FACTORY_TYPE_SINGLE_THREADED, Some(&options))
    }
}

fn create_device_with_type(
    drive_type: Direct3D::D3D_DRIVER_TYPE,
) -> windows::core::Result<Direct3D11::ID3D11Device> {
    let mut flags = Direct3D11::D3D11_CREATE_DEVICE_BGRA_SUPPORT;

    if cfg!(debug_assertions) {
        flags |= Direct3D11::D3D11_CREATE_DEVICE_DEBUG;
    }

    let mut device = None;

    unsafe {
        Direct3D11::D3D11CreateDevice(
            None,
            drive_type,
            None,
            flags,
            None,
            Direct3D11::D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            None,
        )
        .map(|()| device.unwrap())
    }
}

fn create_device() -> windows::core::Result<Direct3D11::ID3D11Device> {
    let mut result = create_device_with_type(Direct3D::D3D_DRIVER_TYPE_HARDWARE);

    if let Err(err) = &result {
        if err.code() == Dxgi::DXGI_ERROR_UNSUPPORTED {
            result = create_device_with_type(Direct3D::D3D_DRIVER_TYPE_WARP);
        }
    }

    result
}

fn create_render_target(
    factory: &Direct2D::ID2D1Factory1, device: &Direct3D11::ID3D11Device,
) -> windows::core::Result<Direct2D::ID2D1DeviceContext> {
    unsafe {
        let d2device = factory.CreateDevice(&device.cast::<Dxgi::IDXGIDevice>()?)?;

        let target = d2device.CreateDeviceContext(Direct2D::D2D1_DEVICE_CONTEXT_OPTIONS_NONE)?;

        target.SetUnitMode(Direct2D::D2D1_UNIT_MODE_DIPS);

        Ok(target)
    }
}

fn get_dxgi_factory(
    device: &Direct3D11::ID3D11Device,
) -> windows::core::Result<Dxgi::IDXGIFactory2> {
    let dxdevice = device.cast::<Dxgi::IDXGIDevice>()?;
    unsafe { dxdevice.GetAdapter()?.GetParent() }
}

fn create_swapchain_bitmap(
    swapchain: &Dxgi::IDXGISwapChain1, target: &Direct2D::ID2D1DeviceContext,
) -> windows::core::Result<()> {
    let surface: Dxgi::IDXGISurface = unsafe { swapchain.GetBuffer(0)? };

    let props = Direct2D::D2D1_BITMAP_PROPERTIES1 {
        pixelFormat: Direct2D::Common::D2D1_PIXEL_FORMAT {
            format: Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM,
            alphaMode: Direct2D::Common::D2D1_ALPHA_MODE_IGNORE,
        },
        dpiX: 96.0,
        dpiY: 96.0,
        bitmapOptions: Direct2D::D2D1_BITMAP_OPTIONS_TARGET
            | Direct2D::D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
        ..Default::default()
    };

    unsafe {
        let bitmap = target.CreateBitmapFromDxgiSurface(&surface, Some(&props))?;
        target.SetTarget(&bitmap);
    };

    Ok(())
}

fn create_swapchain(
    device: &Direct3D11::ID3D11Device, window: Win32::Foundation::HWND,
) -> windows::core::Result<Dxgi::IDXGISwapChain1> {
    let factory = get_dxgi_factory(device)?;

    let props = Dxgi::DXGI_SWAP_CHAIN_DESC1 {
        Format: Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM,
        SampleDesc: Dxgi::Common::DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
        BufferUsage: Dxgi::DXGI_USAGE_RENDER_TARGET_OUTPUT,
        BufferCount: 2,
        SwapEffect: Dxgi::DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
        ..Default::default()
    };

    unsafe { factory.CreateSwapChainForHwnd(device, window, &props, None, None) }
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
    obj.editor.buffer = "# hello from editor".into();

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

// NativeWindow taken from Smail's android PR
// https://github.com/lockbook/lockbook/pull/1835/files#diff-0f28854a868a55fcd30ff5f0fda476aed540b2e1fc3762415ac6e0588ed76fb6
pub struct NativeWindow {
    handle: Win32WindowHandle,
}

impl NativeWindow {
    pub fn new(window: Win32::Foundation::HWND) -> Self {
        let mut handle = Win32WindowHandle::empty();
        handle.hwnd = window.0 as *mut c_void;
        Self { handle }
    }
}

unsafe impl HasRawWindowHandle for NativeWindow {
    fn raw_window_handle(&self) -> RawWindowHandle {
        RawWindowHandle::Win32(self.handle)
    }
}

unsafe impl HasRawDisplayHandle for NativeWindow {
    fn raw_display_handle(&self) -> RawDisplayHandle {
        RawDisplayHandle::Windows(WindowsDisplayHandle::empty())
    }
}
