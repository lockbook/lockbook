use std::{iter, time::Instant};

use egui::{PlatformOutput, ViewportIdMap, ViewportOutput};
use egui_wgpu::{Renderer, ScreenDescriptor};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use wgpu::{
    Adapter, CompositeAlphaMode, Device, Instance, Queue, Surface, SurfaceTargetUnsafe,
    TextureDescriptor, TextureFormat, TextureUsages,
};

pub struct RendererState<'w> {
    pub context: egui::Context,
    pub raw_input: egui::RawInput,
    pub screen: ScreenDescriptor,
    pub bottom_inset: Option<u32>,

    device: Device,
    adapter: Adapter,
    surface: Surface<'w>,
    renderer: Renderer,
    queue: Queue,

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

    pub fn from_surface(surface: SurfaceTargetUnsafe) -> Self {
        let instance = Self::instance();
        let surface = unsafe { instance.create_surface_unsafe(surface).unwrap() };
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
        let format = Self::text_format(&adapter, &surface);
        let screen = ScreenDescriptor { size_in_pixels: [1300, 800], pixels_per_point: 1.0 };

        let renderer = Renderer::new(&device, format, None, 4, false);

        RendererState {
            screen,
            adapter,
            surface,
            device,
            renderer,
            queue,
            surface_width: 0,
            surface_height: 0,
            bottom_inset: None,
            context: Default::default(),
            raw_input: Default::default(),
            start_time: Instant::now(),
        }
    }

    pub fn begin_frame(&mut self) {
        self.configure_surface();

        self.set_egui_screen();
        self.raw_input.time = Some(self.start_time.elapsed().as_secs_f64());
        self.context.begin_frame(self.raw_input.take());
    }

    pub fn end_frame(&mut self) -> (PlatformOutput, ViewportIdMap<ViewportOutput>) {
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
        let full_output = self.context.end_frame();
        self.context.tessellation_options_mut(|w| {
            w.feathering = false;
        });

        let paint_jobs = self
            .context
            .tessellate(full_output.shapes, full_output.pixels_per_point);
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("encoder") });

        for (id, image_delta) in &full_output.textures_delta.set {
            self.renderer
                .update_texture(&self.device, &self.queue, *id, image_delta);
        }
        self.renderer.update_buffers(
            &self.device,
            &self.queue,
            &mut encoder,
            &paint_jobs,
            &self.screen,
        );

        // Record all render passes.
        {
            let _ = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &output_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::WHITE), // todo: these are different
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
        }

        {
            let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &msaa_view,
                    resolve_target: Some(&output_view),
                    ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            self.renderer
                .render(&mut pass.forget_lifetime(), &paint_jobs, &self.screen);
        }

        // Submit the commands.
        self.queue.submit(iter::once(encoder.finish()));

        // Redraw egui
        output_frame.present();

        for id in &full_output.textures_delta.free {
            self.renderer.free_texture(id);
        }

        (full_output.platform_output, full_output.viewport_output)
    }

    /// inspired by egui_wgpu::RenderState
    fn text_format(adapter: &Adapter, surface: &Surface<'w>) -> TextureFormat {
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

    fn configure_surface(&mut self) {
        let resized = self.screen.size_in_pixels[0] != self.surface_width
            || self.screen.size_in_pixels[1] != self.surface_height;
        let visible = self.screen.size_in_pixels[0] * self.screen.size_in_pixels[1] != 0;
        if resized && visible {
            let surface_config = wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format: Self::text_format(&self.adapter, &self.surface),
                width: self.screen.size_in_pixels[0],
                height: self.screen.size_in_pixels[1],
                present_mode: wgpu::PresentMode::Fifo,
                alpha_mode: Self::text_alpha(&self.adapter, &self.surface),
                view_formats: vec![],
                desired_maximum_frame_latency: 2,
            };
            self.surface.configure(&self.device, &surface_config);
            self.surface_width = self.screen.size_in_pixels[0];
            self.surface_height = self.screen.size_in_pixels[1];
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
                    memory_hints: Default::default(),
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
