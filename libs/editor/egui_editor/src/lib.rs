pub use crate::editor::Editor;
use egui::{Context, FontData, FontDefinitions, FontFamily, Pos2, Rect};
use egui_wgpu_backend::wgpu;
use std::iter;
use std::sync::Arc;

pub mod appearance;
pub mod ast;
pub mod buffer;
pub mod cursor;
pub mod debug;
pub mod draw;
pub mod editor;
pub mod element;
pub mod events;
pub mod galleys;
pub mod layouts;
pub mod offset_types;
pub mod styles;
pub mod test_input;
pub mod unicode_segs;

#[cfg(target_vendor = "apple")]
pub mod apple;

#[repr(C)]
pub struct WgpuEditor {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface: wgpu::Surface,
    pub adapter: wgpu::Adapter,

    pub rpass: egui_wgpu_backend::RenderPass,
    pub screen: egui_wgpu_backend::ScreenDescriptor,

    pub context: egui::Context,
    pub raw_input: egui::RawInput,

    pub editor: Editor,
}

impl WgpuEditor {
    pub fn frame(&mut self) {
        self.configure_surface();
        let output_frame = match self.surface.get_current_texture() {
            Ok(frame) => frame,
            Err(wgpu::SurfaceError::Outdated) => {
                // This error occurs when the app is minimized on Windows.
                // Silently return here to prevent spamming the console with:
                // "The underlying surface has changed, and therefore the swap chain must be updated"
                panic!("wgpu surface outdated")
            }
            Err(e) => {
                eprintln!("Dropped frame with error: {}", e);
                panic!()
            }
        };
        let output_view = output_frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        // can probably use run
        self.set_egui_screen();

        self.context.begin_frame(self.raw_input.take());
        self.editor.draw(&self.context);
        // todo: consider consuming repaint_after
        let full_output = self.context.end_frame();
        let paint_jobs = self.context.tessellate(full_output.shapes);
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("encoder") });

        let tdelta: egui::TexturesDelta = full_output.textures_delta;
        self.rpass
            .add_textures(&self.device, &self.queue, &tdelta)
            .expect("add texture ok");

        self.rpass
            .update_buffers(&self.device, &self.queue, &paint_jobs, &self.screen);
        // Record all render passes.
        self.rpass
            .execute(
                &mut encoder,
                &output_view,
                &paint_jobs,
                &self.screen,
                Some(wgpu::Color::BLACK),
            )
            .unwrap();
        // Submit the commands.
        self.queue.submit(iter::once(encoder.finish()));

        // Redraw egui
        output_frame.present();

        self.rpass
            .remove_textures(tdelta)
            .expect("remove texture ok");
    }

    pub fn set_egui_screen(&mut self) {
        self.raw_input.screen_rect = Some(Rect {
            min: Pos2::ZERO,
            max: Pos2::new(
                self.screen.physical_width as f32 / self.screen.scale_factor,
                self.screen.physical_height as f32 / self.screen.scale_factor,
            ),
        });
        self.raw_input.pixels_per_point = Some(self.screen.scale_factor);
    }

    pub fn surface_format(&self) -> wgpu::TextureFormat {
        // todo: is this really fine?
        // from here: https://github.com/hasenbanck/egui_example/blob/master/src/main.rs#L65
        self.surface.get_supported_formats(&self.adapter)[0]
    }

    pub fn configure_surface(&self) {
        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: self.surface_format(),
            width: self.screen.physical_width,
            height: self.screen.physical_height,
            present_mode: wgpu::PresentMode::Fifo,
        };
        self.surface.configure(&self.device, &surface_config);
    }
}

pub fn register_fonts(ctx: &Context) {
    let mut fonts = FontDefinitions::default();

    fonts.font_data.insert(
        "pt_sans".to_string(),
        FontData::from_static(include_bytes!("../fonts/PTSans-Regular.ttf")),
    );
    fonts.font_data.insert(
        "pt_mono".to_string(),
        FontData::from_static(include_bytes!("../fonts/PTMono-Regular.ttf")),
    );
    fonts.font_data.insert(
        "pt_bold".to_string(),
        FontData::from_static(include_bytes!("../fonts/PTSans-Bold.ttf")),
    );

    fonts
        .families
        .insert(FontFamily::Name(Arc::from("Bold")), vec!["pt_bold".to_string()]);

    fonts
        .families
        .get_mut(&FontFamily::Proportional)
        .unwrap()
        .insert(0, "pt_sans".to_string());

    fonts
        .families
        .get_mut(&FontFamily::Monospace)
        .unwrap()
        .insert(0, "pt_mono".to_string());

    ctx.set_fonts(fonts);
}
