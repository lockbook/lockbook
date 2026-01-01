use std::time::Instant;

use egui_wgpu::{Renderer, ScreenDescriptor};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use wgpu::{
    Adapter, CompositeAlphaMode, Device, Instance, Surface, SurfaceTargetUnsafe, TextureDescriptor,
    TextureUsages,
};

pub struct RendererState<'w> {
    pub context: egui::Context,
    pub raw_input: egui::RawInput,

    device: Device,
    screen: ScreenDescriptor,
    adapter: Adapter,
    surface: Surface<'w>,

    start_time: Instant,
    surface_width: u32,
    surface_height: u32,
}

impl<'w> RendererState<'w> {
    pub fn init_window<W>(window: &'w W) -> Self
    where
        W: HasWindowHandle + HasDisplayHandle + Sync,
    {
        let instance = Self::instance();
        let surface = instance.create_surface(window).unwrap();
        Self::init(instance, surface)
    }

    pub unsafe fn from_surface(surface: SurfaceTargetUnsafe) -> Self {
        let instance = Self::instance();
        let surface = instance.create_surface_unsafe(surface).unwrap();
        Self::init(instance, surface)
    }

    fn instance() -> wgpu::Instance {
        let backends = wgpu::util::backend_bits_from_env().unwrap_or_else(wgpu::Backends::all);
        let instance_desc = wgpu::InstanceDescriptor { backends, ..Default::default() };
        wgpu::Instance::new(instance_desc)
    }

    fn init(instance: Instance, surface: Surface<'w>) -> Self {
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

        RendererState {
            screen,
            adapter,
            surface,
            device,
            surface_width: 0,
            surface_height: 0,
            context: Default::default(),
            raw_input: Default::default(),
        }
    }

    pub fn frame(&mut self) {
        self.configure_surface();

        let output_frame = match self.surface.get_current_texture() {
            Ok(frame) => frame,
            Err(wgpu::SurfaceError::Outdated) => {
                // This error occurs when the app is minimized on Windows.
                // Silently return here to prevent spamming the console with:
                // "The underlying surface has changed, and therefore the swap chain must be updated"
                eprintln!("wgpu::SurfaceError::Outdated");
                return Default::default(); // todo: could this be the source of a bug if some
                // response has a default value of true or something
            }
            Err(e) => {
                eprintln!("Dropped frame with error: {e}");
                return Default::default();
            }
        };
        let output_view = output_frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let msaa_texture = self.device.create_texture(&TextureDescriptor {
            label: Some("msaa_texture"),
            size: output_frame.texture.size(),
            mip_level_count: output_frame.texture.mip_level_count(),
            sample_count: 4,
            dimension: output_frame.texture.dimension(),
            format: output_frame.texture.format(),
            usage: TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });

        let msaa_view = msaa_texture.create_view(&wgpu::TextureViewDescriptor::default());
        self.set_egui_screen();
    }

    fn configure_surface(&mut self) {
        use egui_wgpu::wgpu::CompositeAlphaMode;

        let resized = self.screen.size_in_pixels[0] != self.surface_width
            || self.screen.size_in_pixels[1] != self.surface_height;
        let visible = self.screen.size_in_pixels[0] * self.screen.size_in_pixels[1] != 0;
        if resized && visible {
            let surface_config = wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format: self.surface_format(),
                width: self.screen.size_in_pixels[0],
                height: self.screen.size_in_pixels[1],
                present_mode: wgpu::PresentMode::Fifo,
                alpha_mode: CompositeAlphaMode::Auto,
                view_formats: vec![],
                desired_maximum_frame_latency: 2,
            };
            self.surface.configure(&self.device, &surface_config);
            self.surface_width = self.screen.size_in_pixels[0];
            self.surface_height = self.screen.size_in_pixels[1];
        }
    }

    pub fn surface_format(&self) -> wgpu::TextureFormat {
        // todo: is this really fine?
        // from here: https://github.com/hasenbanck/egui_example/blob/master/src/main.rs#L65
        self.surface.get_capabilities(&self.adapter).formats[0]
    }

    fn set_egui_screen(&mut self) {
        use egui::{Pos2, Rect};

        self.raw_input.screen_rect = Some(Rect {
            min: Pos2::ZERO,
            max: Pos2::new(
                self.screen.size_in_pixels[0] as f32 / self.screen.pixels_per_point,
                self.screen.size_in_pixels[1] as f32 / self.screen.pixels_per_point,
            ),
        });
        self.context
            .set_pixels_per_point(self.screen.pixels_per_point);
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
