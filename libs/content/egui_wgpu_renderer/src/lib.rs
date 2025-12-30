use egui::Context;
use egui_wgpu::{Renderer, ScreenDescriptor};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use wgpu::{CompositeAlphaMode, Instance, Surface};

pub struct RendererState {}

impl RendererState {
    pub fn init_window<W>(window: &W, dark_mode: bool) -> Self
    where
        W: HasWindowHandle + HasDisplayHandle + Sync,
    {
        let instance = Self::instance();
        let surface = instance.create_surface(window).unwrap();
        Self::init(instance, surface)
    }

    pub fn from_surface() -> Self {
        let instance = Self::instance();
        RendererState {}
    }

    fn instance() -> wgpu::Instance {
        let backends = wgpu::util::backend_bits_from_env().unwrap_or_else(wgpu::Backends::all);
        let instance_desc = wgpu::InstanceDescriptor { backends, ..Default::default() };
        wgpu::Instance::new(instance_desc)
    }

    fn init(instance: Instance, surface: Surface) -> Self {
        let (adapter, device, queue) =
            pollster::block_on(Self::request_device(&instance, &surface));
        let format = surface.get_capabilities(&adapter).formats[0]; // todo: maybe #4065
        let screen = ScreenDescriptor { size_in_pixels: [1000, 1000], pixels_per_point: 1.0 };
        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: screen.size_in_pixels[0], // TODO get from context or something
            height: screen.size_in_pixels[1],
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: CompositeAlphaMode::Auto,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_config);
        let renderer = Renderer::new(&device, format, None, 4);

        let context = Context::default();

        RendererState {}
    }

    async fn request_device(
        instance: &wgpu::Instance, surface: &wgpu::Surface<'_>,
    ) -> (wgpu::Adapter, wgpu::Device, wgpu::Queue) {
        let adapter = wgpu::util::initialize_adapter_from_env_or_default(instance, Some(surface))
            .await
            .expect("No suitable GPU adapters found on the system!");
        let res = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    required_features: adapter.features(),
                    required_limits: adapter.limits(),
                    // memory_hints: Default::default(), // todo: restore after updating wgpu
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
}
