pub mod file_cache;
pub mod landing;
#[cfg(not(target_family = "wasm"))]
pub mod mind_map;
pub mod output;
pub mod show;
pub mod space_inspector;
pub mod tab;
pub mod task_manager;
pub mod theme;
pub mod widgets;
pub mod workspace;

use std::ops::DerefMut as _;
use std::sync::{Arc, Mutex, RwLock};

use egui::Rect;
use egui_wgpu_renderer::egui_wgpu::{self, Renderer, ScreenDescriptor};
use glyphon::{
    Buffer, Color, ColorMode, RenderError, Resolution, SwashCache, TextArea, TextAtlas, TextBounds,
    TextRenderer, Viewport,
};
use glyphon::{FontSystem, PrepareError};
pub use output::Response;
pub use tab::Event;

use egui_wgpu_renderer::wgpu::{self, Device, MultisampleState, Queue, TextureFormat};
use epaint::text::FontDefinitions;

pub fn register_fonts(fonts: &mut FontDefinitions) {
    tab::markdown_editor::register_fonts(fonts)
}

pub struct GlyphonRenderCallbackResources {
    pub font_system: Arc<Mutex<FontSystem>>,
    pub swash_cache: SwashCache,
    pub text_atlas: TextAtlas,
    pub viewport: Viewport,
    pub text_renderer: TextRenderer,
}

pub fn register_render_callback_resources(
    device: &Device, queue: &Queue, texture_format: TextureFormat, renderer: &mut Renderer,
    font_system: Arc<Mutex<FontSystem>>,
) {
    let swash_cache = SwashCache::new();
    let gcache = glyphon::Cache::new(device);
    let viewport = Viewport::new(device, &gcache);
    let mut text_atlas =
        TextAtlas::with_color_mode(device, queue, &gcache, texture_format, ColorMode::Web);
    let text_renderer = TextRenderer::new(
        &mut text_atlas,
        device,
        MultisampleState {
            count: 1, /* todo: 4 on macos, ? on ios, ...? */
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        None,
    );

    renderer
        .callback_resources
        .insert(GlyphonRenderCallbackResources {
            font_system: Arc::clone(&font_system),
            swash_cache,
            viewport,
            text_atlas,
            text_renderer,
        });
}

impl GlyphonRenderCallbackResources {
    pub fn prepare<'a>(
        &mut self, device: &wgpu::Device, queue: &wgpu::Queue, screen_resolution: Resolution,
        text_areas: impl IntoIterator<Item = TextArea<'a>>,
    ) -> Result<(), PrepareError> {
        self.viewport.update(queue, screen_resolution);
        self.text_renderer.prepare(
            device,
            queue,
            self.font_system.lock().unwrap().deref_mut(),
            &mut self.text_atlas,
            &self.viewport,
            text_areas,
            &mut self.swash_cache,
        )
    }

    pub fn render(&self, pass: &mut wgpu::RenderPass<'static>) -> Result<(), RenderError> {
        self.text_renderer
            .render(&self.text_atlas, &self.viewport, pass)
    }
}

pub struct GlyphonRendererCallback {
    pub buffers: Vec<TextBufferArea>,
}

#[derive(Clone)]
pub struct TextBufferArea {
    pub buffer: Arc<RwLock<Buffer>>,
    pub rect: Rect,
    pub scale: f32,
    pub default_color: Color,
}

impl TextBufferArea {
    pub fn new(
        buffer: Arc<RwLock<Buffer>>, rect: Rect, default_color: Color, ctx: &egui::Context,
    ) -> Self {
        let ppi = ctx.pixels_per_point();
        let rect = rect * ppi;
        TextBufferArea { buffer, rect, scale: ppi, default_color }
    }
}

impl egui_wgpu::CallbackTrait for GlyphonRendererCallback {
    fn prepare(
        &self, device: &wgpu::Device, queue: &wgpu::Queue, screen_descriptor: &ScreenDescriptor,
        _egui_encoder: &mut wgpu::CommandEncoder, resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let glyphon_renderer: &mut GlyphonRenderCallbackResources = resources.get_mut().unwrap();
        glyphon_renderer.text_atlas.trim();
        let bufrefs: Vec<_> = self
            .buffers
            .iter()
            .map(|b| b.buffer.read().unwrap())
            .collect();
        let text_areas: Vec<_> = self
            .buffers
            .iter()
            .enumerate()
            .map(|(i, b)| TextArea {
                custom_glyphs: &[],
                buffer: bufrefs.get(i).unwrap(),
                left: b.rect.left(),
                top: b.rect.top(),
                scale: b.scale,
                bounds: TextBounds {
                    left: b.rect.left() as i32,
                    top: b.rect.top() as i32,
                    right: b.rect.right() as i32,
                    bottom: b.rect.bottom() as i32,
                },
                default_color: b.default_color,
            })
            .collect();

        glyphon_renderer
            .prepare(
                device,
                queue,
                Resolution {
                    width: screen_descriptor.size_in_pixels[0],
                    height: screen_descriptor.size_in_pixels[1],
                },
                text_areas,
            )
            .unwrap();
        Vec::new()
    }

    fn paint<'a>(
        &self, info: egui::PaintCallbackInfo, render_pass: &mut wgpu::RenderPass<'static>,
        resources: &egui_wgpu::CallbackResources,
    ) {
        render_pass.set_viewport(
            0.0,
            0.0,
            info.screen_size_px[0] as f32,
            info.screen_size_px[1] as f32,
            0.0,
            1.0,
        );

        let glyphon_renderer: &GlyphonRenderCallbackResources = resources.get().unwrap();
        glyphon_renderer.render(render_pass).unwrap();
    }
}
