use std::ops::DerefMut as _;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, RwLock};

use egui::Rect;
use egui_wgpu_renderer::egui_wgpu::{self, Renderer, ScreenDescriptor};
use egui_wgpu_renderer::wgpu::{self, Device, MultisampleState, Queue, TextureFormat};
use glyphon::{
    AttrsList, Buffer, Color, ColorMode, FontSystem, Resolution, SwashCache, TextArea, TextAtlas,
    TextBounds, TextRenderer, Viewport, fontdb,
};

use super::glyphon_cache::GlyphonCache;

pub fn register_font_system(ctx: &egui::Context) -> Arc<Mutex<FontSystem>> {
    // Idempotent: callers (workspace boot, test harness per-frame) may
    // invoke this repeatedly. Reusing the existing FontSystem +
    // GlyphonCache preserves the shaped-buffer cache across frames.
    if let Some(font_system) = ctx.data(|d| d.get_temp::<Arc<Mutex<FontSystem>>>(egui::Id::NULL)) {
        return font_system;
    }

    let font_system = Arc::new(Mutex::new(new_font_system()));
    ctx.data_mut(|d| d.insert_temp(egui::Id::NULL, Arc::clone(&font_system)));

    let glyphon_cache = Arc::new(Mutex::new(GlyphonCache::new()));
    ctx.data_mut(|d| d.insert_temp(egui::Id::NULL, glyphon_cache));

    font_system
}

/// Points the registered font system at the chosen families and drops the
/// text shaped in the old ones.
pub fn reload_font_system(ctx: &egui::Context) {
    if let Some(font_system) = ctx.data(|d| d.get_temp::<Arc<Mutex<FontSystem>>>(egui::Id::NULL)) {
        crate::font::refresh(font_system.lock().unwrap().db_mut());
    }
    if let Some(cache) = ctx.data(|d| d.get_temp::<Arc<Mutex<GlyphonCache>>>(egui::Id::NULL)) {
        *cache.lock().unwrap() = GlyphonCache::new();
    }
}

/// Bundled fonts, then the system's.
pub fn new_font_system() -> FontSystem {
    let mut db = fontdb::Database::new();
    crate::font::load(&mut db);
    let fallback = crate::font::Fallback::default();
    FontSystem::new_with_locale_and_db_and_fallback("en-US".into(), db, fallback)
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
    layers: Vec<GlyphonLayer>,
    next_layer: usize,
    frame_reset: bool,
    pub pending_resolution: Resolution,
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

    /// A copy whose every color, spans included, carries `opacity` of its alpha.
    pub fn faded(&self, opacity: f32, font_system: &mut FontSystem) -> Self {
        let fade = |c: Color| Color::rgba(c.r(), c.g(), c.b(), (c.a() as f32 * opacity) as u8);
        let mut buffer = self.buffer.read().unwrap().clone();
        for line in &mut buffer.lines {
            let old = line.attrs_list();
            let mut faded = AttrsList::new(
                &old.defaults()
                    .color(fade(old.defaults().color_opt.unwrap_or(self.default_color))),
            );
            for (range, attrs) in old.spans_iter() {
                let attrs = attrs.as_attrs();
                let color = fade(attrs.color_opt.unwrap_or(self.default_color));
                faded.add_span(range.clone(), &attrs.color(color));
            }
            line.set_attrs_list(faded);
        }
        buffer.shape_until_scroll(font_system, false);
        Self {
            buffer: Arc::new(RwLock::new(buffer)),
            rect: self.rect,
            clip_rect: self.clip_rect,
            default_color: fade(self.default_color),
        }
    }
}

/// Text painted for print rather than the GPU, in paint order among the
/// other shapes.
pub struct PrintText(pub Vec<TextBufferArea>);

/// Marks a context while a [`crate::tab::Presentation::Print`] tab draws in
/// it: its text paints as [`PrintText`].
#[derive(Clone, Copy)]
pub struct Printing;

/// Whether `ctx` is drawing a tab for print.
pub fn printing(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp::<Printing>(egui::Id::NULL))
        .is_some()
}

/// Paints `areas` clipped to `rect` at `painter`'s opacity: through
/// glyphon, or as [`PrintText`] while printing.
pub fn paint_text(painter: &egui::Painter, rect: Rect, mut areas: Vec<TextBufferArea>) {
    let opacity = painter.opacity();
    if opacity == 0.0 {
        return;
    }
    if opacity < 1.0 {
        let font_system = register_font_system(painter.ctx());
        let mut font_system = font_system.lock().unwrap();
        areas = areas
            .iter()
            .map(|area| area.faded(opacity, &mut font_system))
            .collect();
    }
    if printing(painter.ctx()) {
        painter.add(egui::PaintCallback { rect, callback: Arc::new(PrintText(areas)) });
    } else {
        painter.add(egui_wgpu::Callback::new_paint_callback(
            rect,
            GlyphonRendererCallback::new(areas),
        ));
    }
}

impl egui_wgpu::CallbackTrait for GlyphonRendererCallback {
    fn prepare(
        &self, device: &wgpu::Device, _queue: &wgpu::Queue, screen_descriptor: &ScreenDescriptor,
        _egui_encoder: &mut wgpu::CommandEncoder, resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let r: &mut GlyphonRenderCallbackResources = resources.get_mut().unwrap();
        r.pending_resolution = Resolution {
            width: screen_descriptor.size_in_pixels[0],
            height: screen_descriptor.size_in_pixels[1],
        };
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

    fn finish_prepare(
        &self, device: &wgpu::Device, queue: &wgpu::Queue,
        _egui_encoder: &mut wgpu::CommandEncoder, resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let r: &mut GlyphonRenderCallbackResources = resources.get_mut().unwrap();
        let idx = self.layer.load(Ordering::Relaxed);

        if idx + 1 == r.next_layer {
            r.frame_reset = true;
        }

        let pending: Vec<_> = std::mem::take(&mut r.layers[idx].pending);
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
