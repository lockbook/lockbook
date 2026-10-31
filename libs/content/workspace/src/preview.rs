//! A tab drawn off screen to an image: widget tiles, landing previews.
//!
//! A preview owns a private egui context dressed like the live one, builds
//! its tab from a bundle of its own (image textures and link previews belong
//! to a context), pumps frames until the tab settles, and renders the last
//! one through an egui renderer of its own on the live device: texture ids
//! are per context, so one renderer can't serve two.

use std::sync::{Arc, Mutex};

use egui::epaint::ClippedShape;
use egui::os::OperatingSystem;
use egui::{Context, Id, RawInput, Rect, TexturesDelta, Vec2, ViewportId, pos2};
use egui_wgpu_renderer::egui_wgpu::{self, RendererOptions};
use egui_wgpu_renderer::wgpu::{self, Device, Queue};
use egui_wgpu_renderer::{OFFSCREEN_FORMAT, OffscreenFrame, render_offscreen};
use glyphon::FontSystem;
use web_time::{Duration, Instant};

use crate::link_preview::LinkPreviews;
use crate::tab::markdown_editor::HttpClient;
use crate::tab::{
    ContentState, Destination, Doc, Place, Presentation, Reveal, Tab, TabContent, TabFailure,
    TabResources,
};
use crate::theme::palette_v2::{Theme, ThemeExt as _};
use crate::widgets::glyphon_cache::GlyphonCache;
use crate::widgets::image_cache::ImageCache;

/// How long a preview waits for its tab to settle: a worker that never
/// finishes, not the design, is what this bounds.
const SETTLE_BOUND: Duration = Duration::from_secs(5);

const SAMPLES: u32 = 4;

/// What a preview is of and how it's drawn.
#[derive(Clone, Debug)]
pub struct PreviewRequest {
    pub place: Place,
    /// The size the tab lays out at, in the host's points.
    pub size: Vec2,
    /// Pixels per point: the host's, or less for a thumbnail.
    pub scale: f32,
    /// Room around the content, in points: an embed's host pads it.
    pub padding: f32,
    /// An open note's text, unsaved edits and all, instead of what's saved.
    pub text: Option<String>,
}

/// How a private context looks: the live one's zoom and platform, in the
/// theme and mode the host asks for.
#[derive(Clone, Copy)]
pub struct Look {
    pub theme: Theme,
    pub zoom: f32,
    pub os: OperatingSystem,
}

impl Look {
    pub fn of(ctx: &Context) -> Self {
        Self { theme: ctx.get_lb_theme(), zoom: ctx.zoom_factor(), os: ctx.os() }
    }
}

/// A tab on a private context, drawn frame by frame.
pub struct Headless {
    pub ctx: Context,
    pub res: TabResources,
    pub tab: Tab,
    fragment: Option<crate::tab::Fragment>,
    padding: f32,
    /// The image's size in points of the private context, and its pixels per
    /// point.
    points: Vec2,
    pixels_per_point: f32,
    started: Instant,
    frames: usize,
    textures: TexturesDelta,
    shapes: Vec<ClippedShape>,
}

impl Headless {
    /// Builds `req`'s tab on a private context looking like `look`, sharing
    /// `live`'s core, files, and persistence. Reads the document from core
    /// on the calling thread.
    pub fn open(live: &TabResources, look: Look, req: &PreviewRequest) -> Self {
        let ctx = Context::default();
        let mut fonts = egui::FontDefinitions::default();
        crate::register_fonts(&mut fonts);
        ctx.set_fonts(fonts);
        crate::theme::visuals::init(&ctx);
        ctx.set_lb_theme(look.theme);
        ctx.set_zoom_factor(look.zoom);
        ctx.set_os(look.os);
        share_font_system(&ctx, crate::register_font_system(&live.ctx));

        let http = HttpClient::default();
        let res = TabResources {
            ctx: ctx.clone(),
            images: ImageCache::new(
                ctx.clone(),
                http.clone(),
                live.core.clone(),
                Arc::clone(&live.files),
                live.persistence.clone(),
            ),
            previews: LinkPreviews::new(ctx.clone(), http, live.persistence.clone()),
            watched: false,
            ..live.clone()
        };

        let dest = &req.place.dest;
        let doc = match (dest, &req.text) {
            (Destination::File(id), Some(text)) => {
                res.name(*id)
                    .map(|name| Doc { name, hmac: None, bytes: text.clone().into_bytes() })
            }
            _ => None,
        };
        let salt = Id::unique(lb_rs::Uuid::new_v4());
        let content = match TabContent::open(dest, doc, &res, salt, true) {
            Ok(content) => ContentState::Open(content),
            Err(failure) => ContentState::Failed(failure),
        };
        let mut tab = Tab::new(dest.clone(), content, salt, true);
        tab.presentation = Presentation::Embedded;

        let pixels_per_point = req.scale * look.zoom;
        Self {
            ctx,
            res,
            tab,
            fragment: req.place.fragment.clone(),
            padding: req.padding / look.zoom,
            points: req.size / look.zoom,
            pixels_per_point,
            started: Instant::now(),
            frames: 0,
            textures: Default::default(),
            shapes: Default::default(),
        }
    }

    /// Draws one frame, landing on the fragment after the first.
    pub fn frame(&mut self) {
        self.frame_with(vec![]);
    }

    /// [`Self::frame`] with input.
    pub fn frame_with(&mut self, events: Vec<egui::Event>) {
        if self.frames == 1 {
            self.tab.reveal = Some(Reveal { fragment: self.fragment.clone(), flash: false });
        }
        let mut input = RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), self.points)),
            time: Some(self.started.elapsed().as_secs_f64()),
            events,
            ..Default::default()
        };
        input
            .viewports
            .entry(ViewportId::ROOT)
            .or_default()
            .native_pixels_per_point = Some(self.pixels_per_point / self.ctx.zoom_factor());

        let glyphon = self.glyphon_cache();
        if let Some(cache) = &glyphon {
            cache.lock().unwrap().begin_frame();
        }
        self.res.images.begin_frame();
        let bg = self.ctx.get_lb_theme().neutral_bg();
        let tab = &mut self.tab;
        let padding = self.padding;
        let out = self.ctx.run_pass(input, |ctx| {
            ctx.root_ui(|root| {
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE.fill(bg))
                    .show(root, |ui| {
                        let rect = ui.max_rect().shrink(padding);
                        ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| tab.show(ui))
                    });
            });
        });
        self.res.images.end_frame();
        if let Some(cache) = &glyphon {
            cache.lock().unwrap().end_frame();
        }

        self.textures.append(out.textures_delta);
        self.shapes = out.shapes;
        self.pixels_per_point = out.pixels_per_point;
        self.frames += 1;
    }

    /// Draws frames until the tab settles, or long enough that it won't;
    /// returns whether it settled.
    pub fn settle(&mut self) -> bool {
        loop {
            self.frame();
            if self.frames > 1 && self.tab.settled() {
                return true;
            }
            if self.started.elapsed() > SETTLE_BOUND {
                return false;
            }
            std::thread::sleep(Duration::from_millis(4));
        }
    }

    /// Settles the tab and takes its frame; why not, if it failed to open
    /// or never settled, which a host shouldn't keep as the tab's image.
    pub fn settled_frame(&mut self) -> Result<PreviewFrame, String> {
        let settled = self.settle();
        if let Some(failure) = self.failure() {
            return Err(failure.msg());
        }
        if !settled {
            return Err("the tab didn't settle".into());
        }
        Ok(self.take_frame())
    }

    /// The last frame, ready for a renderer of its own: every texture the
    /// context has made comes with it, since the renderer starts empty.
    pub fn take_frame(&mut self) -> PreviewFrame {
        let shapes = std::mem::take(&mut self.shapes);
        let size = (self.points * self.pixels_per_point)
            .round()
            .max(Vec2::splat(1.0));
        let [r, g, b, a] = self
            .ctx
            .get_lb_theme()
            .neutral_bg()
            .to_normalized_gamma_f32();
        PreviewFrame {
            paint_jobs: self.ctx.tessellate(shapes, self.pixels_per_point),
            textures: self.textures.clone(),
            size_in_pixels: [size.x as u32, size.y as u32],
            pixels_per_point: self.pixels_per_point,
            clear: wgpu::Color { r: r as f64, g: g as f64, b: b as f64, a: a as f64 },
        }
    }

    /// Why the tab didn't open, if it didn't.
    pub fn failure(&self) -> Option<&TabFailure> {
        match &self.tab.content {
            ContentState::Failed(failure) => Some(failure),
            _ => None,
        }
    }

    fn glyphon_cache(&self) -> Option<Arc<Mutex<GlyphonCache>>> {
        self.ctx
            .data(|d| d.get_temp::<Arc<Mutex<GlyphonCache>>>(Id::NULL))
    }
}

impl Drop for Headless {
    /// The texture history only ever feeds copies to renderers.
    fn drop(&mut self) {
        self.textures.clear();
    }
}

/// A frame drawn off screen, tessellated, with every texture change since
/// its context began.
pub type PreviewFrame = OffscreenFrame;

/// The live device and queue, and the font system the live context shapes
/// with, which every context shares.
#[derive(Clone)]
pub struct PreviewGpu {
    pub device: Device,
    pub queue: Queue,
    pub font_system: Arc<Mutex<FontSystem>>,
}

impl PreviewGpu {
    /// Renders `frame` through a renderer of its own, returning RGBA rows.
    pub fn render(&self, frame: PreviewFrame) -> Result<(Vec<u8>, [u32; 2]), String> {
        let mut renderer = egui_wgpu::Renderer::new(
            &self.device,
            OFFSCREEN_FORMAT,
            RendererOptions { msaa_samples: SAMPLES, ..Default::default() },
        );
        crate::register_render_callback_resources(
            &self.device,
            &self.queue,
            OFFSCREEN_FORMAT,
            &mut renderer,
            Arc::clone(&self.font_system),
            SAMPLES,
        );
        let size = frame.size_in_pixels;
        let mut pixels = render_offscreen(&self.device, &self.queue, &mut renderer, frame, SAMPLES)
            .ok_or("the GPU returned no pixels")?;
        // the background is opaque; edges blended over the clear aren't
        for px in pixels.chunks_mut(4) {
            px[3] = 255;
        }
        Ok((pixels, size))
    }
}

/// Encodes RGBA rows as a PNG.
pub fn encode_png(pixels: Vec<u8>, [width, height]: [u32; 2]) -> Result<Vec<u8>, String> {
    let image = image::RgbaImage::from_raw(width, height, pixels).ok_or("bad image size")?;
    let mut png = Vec::new();
    image
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    Ok(png)
}

/// Draws `req` off screen until it settles and returns it as a PNG.
pub fn render_png(
    live: &TabResources, look: Look, req: &PreviewRequest, gpu: &PreviewGpu,
) -> Result<Vec<u8>, String> {
    let frame = Headless::open(live, look, req).settled_frame()?;
    let (pixels, size) = gpu.render(frame)?;
    encode_png(pixels, size)
}

/// Lets `ctx` shape text with `font_system`, which another context made,
/// with a glyphon cache of its own.
pub fn share_font_system(ctx: &Context, font_system: Arc<Mutex<FontSystem>>) {
    ctx.data_mut(|d| d.insert_temp(Id::NULL, font_system));
    ctx.data_mut(|d| d.insert_temp(Id::NULL, Arc::new(Mutex::new(GlyphonCache::new()))));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tab::{Fragment, Place};
    use crate::test_utils::ws::TestWs;

    pub(crate) fn gpu(font_system: Arc<Mutex<FontSystem>>) -> PreviewGpu {
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let adapter = futures::executor::block_on(instance.request_adapter(&Default::default()))
            .expect("no adapter");
        let (device, queue) =
            futures::executor::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                required_limits: adapter.limits(),
                ..Default::default()
            }))
            .expect("no device");
        PreviewGpu { device, queue, font_system }
    }

    /// A note rendered at a heading shows that heading's section from the
    /// image's top and nothing before it.
    #[test]
    fn renders_a_note_at_a_heading() {
        let mut t = TestWs::new();
        let text = "# First\n\nfirst body\n\n# Second\n\nsecond body\n\n# Third\n\nthird body\n";
        let id = t.doc("note.md", text);
        let gpu = gpu(crate::register_font_system(&t.ctx));
        let render = |t: &TestWs, fragment: Option<Fragment>| {
            let req = PreviewRequest {
                place: Place::at(Destination::File(id), fragment),
                size: Vec2::new(300.0, 200.0),
                scale: 2.0,
                padding: 0.0,
                text: None,
            };
            let png = render_png(&t.ws.resources(), Look::of(&t.ctx), &req, &gpu).unwrap();
            image::load_from_memory(&png).unwrap().to_rgba8()
        };
        let top = render(&t, None);
        let second = render(&t, Some(Fragment::Heading("second".into())));
        assert_eq!(second.dimensions(), (600, 400));
        if let Ok(dir) = std::env::var("PREVIEW_PNG_DIR") {
            top.save(format!("{dir}/top.png")).unwrap();
            second.save(format!("{dir}/second.png")).unwrap();
        }
        let inked = |img: &image::RgbaImage| {
            let bg = img.get_pixel(0, 0);
            img.pixels().filter(|p| *p != bg).count()
        };
        assert!(inked(&top) > 100 && inked(&second) > 100);
        assert_ne!(top, second);
        // the next section doesn't paint below this one
        let bg = second.get_pixel(0, 0);
        assert!((300..400).all(|y| (0..600).all(|x| second.get_pixel(x, y) == bg)));
    }

    /// A document that won't open fails its preview at once, rather than
    /// after waiting out the bound for a tab that can't settle.
    #[test]
    fn a_failed_preview_fails_fast() {
        let mut t = TestWs::new();
        let id = t.doc("archive.zip", "");
        t.ws.core
            .write_document(id, &[0, 159, 146, 150, 0, 1])
            .unwrap();
        let req = PreviewRequest {
            place: Place::at(Destination::File(id), Some(Fragment::Heading("x".into()))),
            size: Vec2::new(100.0, 100.0),
            scale: 1.0,
            padding: 0.0,
            text: None,
        };
        let started = Instant::now();
        let mut headless = Headless::open(&t.ws.resources(), Look::of(&t.ctx), &req);
        assert!(headless.settled_frame().is_err());
        assert!(started.elapsed() < SETTLE_BOUND / 2);
    }

    /// Embeds that cycle are refused and nest no deeper than the cap, so a
    /// note embedding itself, or two notes each other, settles.
    #[test]
    fn embed_cycles_settle() {
        let mut t = TestWs::new();
        let a = t.doc(
            "A.md",
            "# A

![[A]]

![[B]]
",
        );
        t.doc(
            "B.md",
            "# B

![[A]]
",
        );
        let req = PreviewRequest {
            place: Place::new(Destination::File(a)),
            size: Vec2::new(300.0, 600.0),
            scale: 1.0,
            padding: 0.0,
            text: None,
        };
        let mut headless = Headless::open(&t.ws.resources(), Look::of(&t.ctx), &req);
        headless.tab.presentation = crate::tab::Presentation::Full;
        assert!(headless.settle());
    }
}
