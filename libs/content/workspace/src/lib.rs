pub mod file_cache;
mod font;
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
pub mod search;

use std::ops::DerefMut as _;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, RwLock};

use egui::Rect;
use egui_wgpu_renderer::egui_wgpu::{self, Renderer, ScreenDescriptor};
use glyphon::{
    Buffer, Color, ColorMode, Resolution, SwashCache, TextArea, TextAtlas, TextBounds,
    TextRenderer, Viewport,
};
use glyphon::{FontSystem, fontdb};
pub use output::Response;
pub use tab::Event;

use egui_wgpu_renderer::wgpu::{self, Device, MultisampleState, Queue, TextureFormat};
use epaint::text::FontDefinitions;

/// Creates a `FontSystem` and exposes via egui context before returning
pub fn register_font_system(ctx: &egui::Context) -> Arc<Mutex<FontSystem>> {
    let mut db = fontdb::Database::new();
    font::load(&mut db);
    let font_system = Arc::new(Mutex::new(FontSystem::new_with_locale_and_db("en-US".into(), db)));
    ctx.data_mut(|d| d.insert_temp(egui::Id::NULL, Arc::clone(&font_system)));
    font_system
}

pub fn register_fonts(fonts: &mut FontDefinitions) {
    tab::markdown_editor::register_fonts(fonts)
}

// One TextRenderer per layer. Each GlyphonRendererCallback owns a layer, so
// glyphon text and egui shapes can be interleaved at arbitrary depths rather
// than all glyphon text painting on top at the end.
struct GlyphonLayer {
    renderer: TextRenderer,
    pending: Vec<TextBufferArea>,
}

pub struct GlyphonRenderCallbackResources {
    pub font_system: Arc<Mutex<FontSystem>>,
    pub swash_cache: SwashCache,
    pub text_atlas: TextAtlas,
    pub viewport: Viewport,
    msaa_samples: u32,
    // Layers grow to the high-water mark of callbacks in a frame and are reused
    // across frames. Only `pending` is cleared each frame; the TextRenderers
    // themselves accumulate glyph data and are kept alive.
    layers: Vec<GlyphonLayer>,
    // Incremented in prepare() for each callback; gives each one a unique index.
    next_layer: usize,
    // Set to true by the last finish_prepare() of a frame so the first
    // prepare() of the next frame knows to reset next_layer and clear pending.
    frame_reset: bool,
    pub pending_resolution: Resolution,
}

pub fn register_render_callback_resources(
    device: &Device, queue: &Queue, texture_format: TextureFormat, renderer: &mut Renderer,
    font_system: Arc<Mutex<FontSystem>>, msaa_samples: u32,
) {
    let swash_cache = SwashCache::new();
    let gcache = glyphon::Cache::new(device);
    let viewport = Viewport::new(device, &gcache);
    let text_atlas =
        TextAtlas::with_color_mode(device, queue, &gcache, texture_format, ColorMode::Web);

    renderer
        .callback_resources
        .insert(GlyphonRenderCallbackResources {
            font_system: Arc::clone(&font_system),
            swash_cache,
            viewport,
            text_atlas,
            msaa_samples,
            layers: Vec::new(),
            next_layer: 0,
            frame_reset: true,
            pending_resolution: Resolution { width: 0, height: 0 },
        });
}

impl GlyphonRenderCallbackResources {
    fn ensure_layer(&mut self, idx: usize, device: &wgpu::Device) {
        if self.layers.len() <= idx {
            let renderer = TextRenderer::new(
                &mut self.text_atlas,
                device,
                MultisampleState {
                    count: self.msaa_samples,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                None,
            );
            self.layers
                .push(GlyphonLayer { renderer, pending: Vec::new() });
        }
    }
}

pub struct GlyphonRendererCallback {
    pub buffers: Vec<TextBufferArea>,
    // Assigned in prepare(), read in finish_prepare() and paint(). AtomicUsize
    // because all three methods take &self, not &mut self.
    layer: AtomicUsize,
}

impl GlyphonRendererCallback {
    pub fn new(buffers: Vec<TextBufferArea>) -> Self {
        Self { buffers, layer: AtomicUsize::new(0) }
    }
}

#[derive(Clone)]
pub struct TextBufferArea {
    pub buffer: Arc<RwLock<Buffer>>,
    pub rect: Rect,
    pub clip_rect: Rect,
    pub default_color: Color,
}

impl TextBufferArea {
    pub fn new(
        buffer: Arc<RwLock<Buffer>>, rect: Rect, default_color: Color, ctx: &egui::Context,
        clip_rect: egui::Rect,
    ) -> Self {
        let ppi = ctx.pixels_per_point();
        let rect = rect * ppi;
        let clip_rect = clip_rect * ppi;
        TextBufferArea { buffer, rect, clip_rect, default_color }
    }
}

impl egui_wgpu::CallbackTrait for GlyphonRendererCallback {
    // egui_wgpu calls prepare() for every paint callback in paint order before
    // calling finish_prepare() or paint() for any of them.
    //
    // Each call claims the next layer slot, stashes the index for the later
    // passes, and enqueues its text areas. No GPU work happens here.
    fn prepare(
        &self, device: &wgpu::Device, _queue: &wgpu::Queue, screen_descriptor: &ScreenDescriptor,
        _egui_encoder: &mut wgpu::CommandEncoder, resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let r: &mut GlyphonRenderCallbackResources = resources.get_mut().unwrap();
        r.pending_resolution = Resolution {
            width: screen_descriptor.size_in_pixels[0],
            height: screen_descriptor.size_in_pixels[1],
        };
        // First prepare() of a new frame: trim layers to last frame's count
        // (dropping any TextRenderers above the high-water mark), then reset
        // the counter. pending was already cleared by drain() in finish_prepare.
        if r.frame_reset {
            r.layers.truncate(r.next_layer);
            r.next_layer = 0;
            r.frame_reset = false;
        }
        let idx = r.next_layer;
        r.next_layer += 1;
        self.layer.store(idx, Ordering::Relaxed);
        r.ensure_layer(idx, device);
        r.layers[idx].pending.extend(self.buffers.iter().cloned());
        Vec::new()
    }

    // finish_prepare() is called for every callback in the same order as
    // prepare(), after all prepare() calls are done. This is where the actual
    // GPU work happens: each callback uploads its layer's glyphs to the atlas.
    fn finish_prepare(
        &self, device: &wgpu::Device, queue: &wgpu::Queue,
        _egui_encoder: &mut wgpu::CommandEncoder, resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let r: &mut GlyphonRenderCallbackResources = resources.get_mut().unwrap();
        let idx = self.layer.load(Ordering::Relaxed);

        // The last callback in the frame (idx == next_layer - 1) arms the
        // reset flag so the first prepare() of the next frame starts clean.
        if idx + 1 == r.next_layer {
            r.frame_reset = true;
        }

        // drain() clears pending while keeping the allocation for next frame,
        // avoiding a reallocation on every extend() in prepare().
        let pending: Vec<_> = r.layers[idx].pending.drain(..).collect();
        if pending.is_empty() {
            return Vec::new();
        }

        let resolution = r.pending_resolution;
        let bufrefs: Vec<_> = pending.iter().map(|b| b.buffer.read().unwrap()).collect();
        let text_areas: Vec<_> = pending
            .iter()
            .enumerate()
            .map(|(i, b)| TextArea {
                custom_glyphs: &[],
                buffer: bufrefs.get(i).unwrap(),
                left: b.rect.left(),
                top: b.rect.top(),
                scale: 1.0,
                bounds: TextBounds {
                    left: b.clip_rect.left() as i32,
                    top: b.clip_rect.top() as i32,
                    right: b.clip_rect.right() as i32,
                    bottom: b.clip_rect.bottom() as i32,
                },
                default_color: b.default_color,
            })
            .collect();

        // Atlas trim and viewport update only need to happen once per frame.
        if idx == 0 {
            r.text_atlas.trim();
            r.viewport.update(queue, resolution);
        }

        let layer = r.layers.get_mut(idx).unwrap();
        layer
            .renderer
            .prepare(
                device,
                queue,
                r.font_system.lock().unwrap().deref_mut(),
                &mut r.text_atlas,
                &r.viewport,
                text_areas,
                &mut r.swash_cache,
            )
            .unwrap();

        Vec::new()
    }

    // paint() is called in paint order (back-to-front), interleaved with egui's
    // own shape rendering. Each callback renders only its own layer, so glyphon
    // text lands at exactly the right depth relative to surrounding egui shapes.
    fn paint(
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
        let r: &GlyphonRenderCallbackResources = resources.get().unwrap();
        let idx = self.layer.load(Ordering::Relaxed);
        if let Some(layer) = r.layers.get(idx) {
            layer
                .renderer
                .render(&r.text_atlas, &r.viewport, render_pass)
                .unwrap();
        }
    }
}
