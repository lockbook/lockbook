use std::{iter, time::Instant};

use egui::epaint::ClippedPrimitive;
use egui::{PlatformOutput, TexturesDelta, ViewportIdMap, ViewportOutput};
use egui_wgpu::{Renderer, ScreenDescriptor};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use wgpu::{
    Adapter, CompositeAlphaMode, Device, Instance, Queue, Surface, SurfaceTargetUnsafe,
    TextureDescriptor, TextureFormat, TextureUsages,
};

pub use egui_wgpu;
pub use wgpu;

pub struct RendererState<'w> {
    pub context: egui::Context,
    pub raw_input: egui::RawInput,
    pub screen: ScreenDescriptor,
    pub bottom_inset: Option<u32>,
    backend: Option<RenderBackend<'w>>,

    start_time: Instant,
}

pub struct PreparedFrame {
    pub platform_output: PlatformOutput,
    pub viewport_output: ViewportIdMap<ViewportOutput>,
    pub textures_delta: TexturesDelta,
    pub paint_jobs: Vec<ClippedPrimitive>,
}

pub struct RenderBackend<'w> {
    pub device: Device,
    pub adapter: Adapter,
    pub surface: Surface<'w>,
    pub renderer: Renderer,
    pub queue: Queue,
    pub sample_count: u32,
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

    fn init(instance: Instance, surface: Surface<'w>) -> Self {
        let (adapter, device, queue) =
            pollster::block_on(Self::request_device(&instance, &surface));
        let format = Self::text_format(&adapter, &surface);
        let screen = ScreenDescriptor { size_in_pixels: [1300, 800], pixels_per_point: 1.0 };

        let renderer = Renderer::new(&device, format, None, 4, false);

        RendererState {
            screen,
            backend: Some(RenderBackend {
                adapter,
                surface,
                device,
                renderer,
                queue,
                sample_count: 4,
                surface_width: 0,
                surface_height: 0,
            }),
            bottom_inset: None,
            context: Default::default(),
            raw_input: Default::default(),
            start_time: Instant::now(),
        }
    }
}

impl RendererState<'static> {
    pub fn from_surface(surface: SurfaceTargetUnsafe) -> Self {
        let instance = Self::instance();
        let surface = unsafe { instance.create_surface_unsafe(surface).unwrap() };
        Self::init(instance, surface)
    }
}

impl<'w> RendererState<'w> {
    fn instance() -> wgpu::Instance {
        let instance_desc = wgpu::InstanceDescriptor::from_env_or_default();
        wgpu::Instance::new(&instance_desc)
    }

    /// Call to update the screen ppp based on an up-to-date native ppp. This is
    /// how the app responds to native ppp changes, such as when the app is
    /// moved to a display with a different pixel density.
    pub fn set_native_pixels_per_point(&mut self, native: f32) {
        self.screen.pixels_per_point = native * self.context.zoom_factor();
    }

    pub fn pos_from_pixels(&self, x: f32, y: f32) -> egui::Pos2 {
        egui::Pos2 { x: x / self.screen.pixels_per_point, y: y / self.screen.pixels_per_point }
    }

    pub fn vec_from_pixels(&self, x: f32, y: f32) -> egui::Vec2 {
        egui::Vec2 { x: x / self.screen.pixels_per_point, y: y / self.screen.pixels_per_point }
    }

    pub fn pos_from_points(&self, x: f32, y: f32) -> egui::Pos2 {
        let z = self.context.zoom_factor();
        egui::Pos2 { x: x / z, y: y / z }
    }

    pub fn begin_frame(&mut self) {
        self.set_egui_screen();
        self.raw_input.time = Some(self.start_time.elapsed().as_secs_f64());
        self.context.begin_pass(self.raw_input.take());
    }

    pub fn prepare_frame(&mut self) -> PreparedFrame {
        let full_output = self.context.end_pass();

        // Update the screen ppp based on an up-to-date screen ppp from egui.
        // This is how the app responds to zoom factor changes, such as cmd+-,
        // cmd+=, or cmd+0. If the zoom factor changed this frame, the new zoom
        // factor was already used, so this value must be updated before using
        // self.screen for tesselation & render.
        self.screen.pixels_per_point = full_output.pixels_per_point;

        self.context.tessellation_options_mut(|w| {
            w.feathering = false;
        });

        let paint_jobs = self
            .context
            .tessellate(full_output.shapes, full_output.pixels_per_point);

        PreparedFrame {
            platform_output: full_output.platform_output,
            viewport_output: full_output.viewport_output,
            textures_delta: full_output.textures_delta,
            paint_jobs,
        }
    }

    pub fn end_frame(&mut self) -> (PlatformOutput, ViewportIdMap<ViewportOutput>) {
        let prepared = self.prepare_frame();
        let platform_output = prepared.platform_output.clone();
        let viewport_output = prepared.viewport_output.clone();
        self.render_prepared_frame(prepared);
        (platform_output, viewport_output)
    }

    pub fn render_prepared_frame(&mut self, prepared: PreparedFrame) {
        let size_in_pixels = self.screen.size_in_pixels;
        let pixels_per_point = self.screen.pixels_per_point;
        self.backend_mut()
            .render_prepared_frame(prepared, size_in_pixels, pixels_per_point);
    }

    pub fn backend(&self) -> &RenderBackend<'w> {
        self.backend.as_ref().expect("renderer backend unavailable")
    }

    pub fn backend_mut(&mut self) -> &mut RenderBackend<'w> {
        self.backend.as_mut().expect("renderer backend unavailable")
    }

    pub fn take_backend(&mut self) -> RenderBackend<'w> {
        self.backend.take().expect("renderer backend unavailable")
    }

    pub fn set_backend(&mut self, backend: RenderBackend<'w>) {
        self.backend = Some(backend);
    }

    /// inspired by egui_wgpu::RenderState
    pub fn text_format(adapter: &Adapter, surface: &Surface<'w>) -> TextureFormat {
        egui_wgpu::preferred_framebuffer_format(&surface.get_capabilities(adapter).formats).unwrap()
    }

    /// inspired by egui_wgpu::RenderState
    fn text_alpha(adapter: &Adapter, surface: &Surface<'w>) -> CompositeAlphaMode {
        let supported_alpha_modes = surface.get_capabilities(adapter).alpha_modes;

        if supported_alpha_modes.contains(&wgpu::CompositeAlphaMode::PreMultiplied) {
            wgpu::CompositeAlphaMode::PreMultiplied
        } else if supported_alpha_modes.contains(&wgpu::CompositeAlphaMode::PostMultiplied) {
            wgpu::CompositeAlphaMode::PostMultiplied
        } else {
            wgpu::CompositeAlphaMode::Auto
        }
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
        if let Some(viewport) = self
            .raw_input
            .viewports
            .get_mut(&self.raw_input.viewport_id)
        {
            viewport.native_pixels_per_point =
                Some(self.screen.pixels_per_point / self.context.zoom_factor());
        }
    }

    async fn request_device(
        instance: &wgpu::Instance, surface: &wgpu::Surface<'_>,
    ) -> (wgpu::Adapter, wgpu::Device, wgpu::Queue) {
        let adapter = wgpu::util::initialize_adapter_from_env_or_default(instance, Some(surface))
            .await
            .expect("No suitable GPU adapters found on the system!");
        let res = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: None,
                required_features: adapter.features(),
                required_limits: adapter.limits(),
                memory_hints: Default::default(),
                trace: Default::default(),
            })
            .await;
        match res {
            Err(err) => {
                panic!("request_device failed: {err:?}");
            }
            Ok((device, queue)) => (adapter, device, queue),
        }
    }
}

impl<'w> RenderBackend<'w> {
    pub fn render_prepared_frame(
        &mut self, prepared: PreparedFrame, size_in_pixels: [u32; 2], pixels_per_point: f32,
    ) {
        let screen = ScreenDescriptor { size_in_pixels, pixels_per_point };
        self.configure_surface(size_in_pixels);

        let output_frame = match self.surface.get_current_texture() {
            Ok(frame) => frame,
            Err(wgpu::SurfaceError::Outdated) => {
                // This error occurs when the app is minimized on Windows.
                // Silently return here to prevent spamming the console with:
                // "The underlying surface has changed, and therefore the swap chain must be updated"
                eprintln!("wgpu::SurfaceError::Outdated");
                return; // todo: could this be the source of a bug if some
                // response has a default value of true or something
            }
            Err(e) => {
                eprintln!("Dropped frame with error: {e}");
                return;
            }
        };
        let output_view = output_frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let msaa_texture = self.device.create_texture(&TextureDescriptor {
            label: Some("msaa_texture"),
            size: output_frame.texture.size(),
            mip_level_count: output_frame.texture.mip_level_count(),
            sample_count: self.sample_count,
            dimension: output_frame.texture.dimension(),
            format: output_frame.texture.format(),
            usage: TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });

        let msaa_view = msaa_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("encoder") });

        for (id, image_delta) in &prepared.textures_delta.set {
            self.renderer
                .update_texture(&self.device, &self.queue, *id, image_delta);
        }
        self.renderer.update_buffers(
            &self.device,
            &self.queue,
            &mut encoder,
            &prepared.paint_jobs,
            &screen,
        );

        // Record all render passes.
        {
            let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("egui_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &msaa_view,
                    resolve_target: Some(&output_view),
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            self.renderer
                .render(&mut pass.forget_lifetime(), &prepared.paint_jobs, &screen);
        }

        // Submit the commands.
        self.queue.submit(iter::once(encoder.finish()));

        // Redraw egui
        output_frame.present();

        for id in &prepared.textures_delta.free {
            self.renderer.free_texture(id);
        }
    }

    fn configure_surface(&mut self, size_in_pixels: [u32; 2]) {
        let resized =
            size_in_pixels[0] != self.surface_width || size_in_pixels[1] != self.surface_height;
        let visible = size_in_pixels[0] * size_in_pixels[1] != 0;
        if resized && visible {
            let surface_config = wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format: RendererState::text_format(&self.adapter, &self.surface),
                width: size_in_pixels[0],
                height: size_in_pixels[1],
                present_mode: wgpu::PresentMode::Fifo,
                alpha_mode: RendererState::text_alpha(&self.adapter, &self.surface),
                view_formats: vec![],
                desired_maximum_frame_latency: 1,
            };
            self.surface.configure(&self.device, &surface_config);
            self.surface_width = size_in_pixels[0];
            self.surface_height = size_in_pixels[1];
        }
    }
}
