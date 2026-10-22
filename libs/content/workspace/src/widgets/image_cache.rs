//! Shared image texture cache.
//!
//! [`ImageKey`] → `Arc<Mutex<ImageState>>` lookups, double-buffered so
//! images that stop appearing in the frame get evicted (and their textures
//! freed) after one grace frame. Matches the lifecycle pattern used by
//! [`GlyphonCache`](super::glyphon_cache::GlyphonCache).
//!
//! The cache loads lockbook files and http(s) URLs, rasterizes SVG via
//! resvg, and corrects EXIF orientation. Decoded images are uploaded as
//! egui textures.

use std::collections::{HashMap, HashSet};
use std::io::Cursor;
use std::ops::Deref;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};

use egui::{ColorImage, Context, TextureId, Vec2};
use image::{DynamicImage, ImageDecoder, ImageEncoder as _};
use lb_rs::blocking::Lb;
use lb_rs::{Uuid, spawn};
use resvg::tiny_skia::Pixmap;
use resvg::usvg::{self, Transform};

use crate::file_cache::FileCache;
use crate::seq::ws_seq;
use crate::tab::markdown_editor::HttpClient;
use crate::workspace::WsPersistentStore;

/// Wraps a block in an `async` closure on wasm, a plain closure on native.
/// Allows the same source to await async HTTP on wasm while running
/// synchronously on native (where `spawn!` backs onto a worker thread).
#[cfg(target_arch = "wasm32")]
macro_rules! async_on_wasm {
    ($block:block) => {
        (async || -> Result<TextureId, String> { $block })
    };
}
#[cfg(not(target_arch = "wasm32"))]
macro_rules! async_on_wasm {
    ($block:block) => {
        (|| -> Result<TextureId, String> { $block })
    };
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ImageState {
    #[default]
    Loading,
    Loaded(TextureId),
    Failed(String),
}

/// The image an embed shows: the file it names, however the note names it,
/// or the URL it fetches.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ImageKey {
    File(Uuid),
    Web(String),
}

impl ImageKey {
    /// The key image dimensions persist under.
    fn persisted(&self) -> String {
        match self {
            ImageKey::File(id) => format!("lb://{id}"),
            ImageKey::Web(url) => url.clone(),
        }
    }
}

#[derive(Default)]
struct Inner {
    current: HashMap<ImageKey, Arc<Mutex<ImageState>>>,
    previous: HashMap<ImageKey, Arc<Mutex<ImageState>>>,
    began_this_frame: bool,
    /// Images that wait the reader asked to see, by the note showing them.
    shown_anyway: HashSet<(Uuid, ImageKey)>,
}

#[derive(Clone)]
pub struct ImageCache {
    inner: Arc<Mutex<Inner>>,
    seq: Arc<AtomicU64>,
    ws_seq: Arc<AtomicU64>,

    ctx: Context,
    client: HttpClient,
    core: Lb,
    files: Arc<RwLock<FileCache>>,
    persistence: WsPersistentStore, // persist image dims
}

impl ImageCache {
    pub fn new(
        ctx: Context, client: HttpClient, core: Lb, files: Arc<RwLock<FileCache>>,
        persistence: WsPersistentStore,
    ) -> Self {
        let ws_seq = ws_seq(&ctx);
        Self {
            inner: Default::default(),
            seq: Arc::new(AtomicU64::new(0)),
            ws_seq,
            ctx,
            client,
            core,
            files,
            persistence,
        }
    }

    /// Changes whenever what an embed shows may: a load, a reveal, or the
    /// switch that lets web images load.
    pub fn seq(&self) -> u64 {
        let web = self.persistence.get_contact_linked_sites() as u64;
        self.seq.load(Ordering::Relaxed) * 2 + web
    }

    pub fn begin_frame(&self) {
        let mut inner = self.inner.lock().unwrap();
        if !inner.began_this_frame {
            inner.began_this_frame = true;
            inner.previous = std::mem::take(&mut inner.current);
        }
    }

    pub fn end_frame(&self) {
        let mut inner = self.inner.lock().unwrap();
        // Unused Loaded textures are freed. Failed is dropped so a later
        // lookup can retry. Loading stays so we don't spawn a second
        // in-flight request.
        let texture_manager = self.ctx.tex_manager();
        let mut keep = Vec::new();
        for (key, state) in inner.previous.drain() {
            let (keep_loading, free_id) = match state.lock().unwrap().deref() {
                ImageState::Loading => (true, None),
                ImageState::Loaded(id) => (false, Some(*id)),
                ImageState::Failed(_) => (false, None),
            };
            if keep_loading {
                keep.push((key, state));
            } else if let Some(id) = free_id {
                texture_manager.write().free(id);
            }
        }
        inner.current.extend(keep);
        inner.began_this_frame = false;
    }

    /// Look up or spawn a load for `key`. `user_activity` indicates whether
    /// this read should count toward suggested docs.
    ///
    /// Call between `begin_frame` and `end_frame`. Keys not looked up during
    /// a frame are evicted at `end_frame` time.
    pub fn get_or_load(&self, key: &ImageKey, user_activity: bool) -> Arc<Mutex<ImageState>> {
        let mut inner = self.inner.lock().unwrap();
        if let Some(state) = inner.current.get(key) {
            return state.clone();
        }
        if let Some(state) = inner.previous.remove(key) {
            inner.current.insert(key.clone(), state.clone());
            return state;
        }

        let state: Arc<Mutex<ImageState>> = Default::default();
        self.spawn_load(key, user_activity, state.clone());
        inner.current.insert(key.clone(), state.clone());
        state
    }

    /// Real texture dimensions for `key`, if known.
    pub fn dims(&self, key: &ImageKey) -> Option<Vec2> {
        self.persistence
            .image_dims(&key.persisted())
            .map(|[w, h]| Vec2::new(w, h))
    }

    /// Whether `key` is a web image the reader hasn't let the app fetch:
    /// fetching tells the site who opened the note, and when.
    pub fn off_the_web(&self, key: &ImageKey) -> bool {
        // the URLs a load would fetch
        let fetched = matches!(key, ImageKey::Web(url) if url.starts_with("http://") || url.starts_with("https://"));
        fetched && !self.persistence.get_contact_linked_sites()
    }

    /// Whether the reader asked to see `key` in `note` though it waits.
    pub fn shown_anyway(&self, note: Uuid, key: &ImageKey) -> bool {
        self.inner
            .lock()
            .unwrap()
            .shown_anyway
            .contains(&(note, key.clone()))
    }

    /// The reader asked to see `key` in `note` though it waits.
    pub fn show_anyway(&self, note: Uuid, key: &ImageKey) {
        self.inner
            .lock()
            .unwrap()
            .shown_anyway
            .insert((note, key.clone()));
        self.seq
            .store(self.ws_seq.fetch_add(1, Ordering::Relaxed), Ordering::Relaxed);
        self.ctx.request_repaint();
    }

    pub fn files(&self) -> Arc<RwLock<FileCache>> {
        self.files.clone()
    }

    pub fn pixels_per_point(&self) -> f32 {
        self.ctx.pixels_per_point()
    }

    pub fn color_image(&self, id: Uuid) -> Result<ColorImage, String> {
        let bytes = self
            .core
            .read_document(id, false)
            .map_err(|e| e.to_string())?;
        let image = decode_with_orientation(&bytes)?;
        let size = [image.width() as usize, image.height() as usize];
        Ok(ColorImage::from_rgba_unmultiplied(size, &image.to_rgba8()))
    }

    /// Put `key`'s image on the clipboard once it's read and decoded, off
    /// the UI thread.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn copy_image(&self, key: &ImageKey) {
        let key = key.clone();
        let (ctx, client, core) = (self.ctx.clone(), self.client.clone(), self.core.clone());
        spawn!({
            let read = || -> Result<ColorImage, String> {
                let (bytes, is_svg) = match &key {
                    ImageKey::File(id) => {
                        let bytes = core.read_document(*id, false).map_err(|e| e.to_string())?;
                        let file = core.get_file_by_id(*id).map_err(|e| e.to_string())?;
                        (bytes, file.name.ends_with(".svg"))
                    }
                    ImageKey::Web(url) => {
                        let bytes = download_image(&client, url)?;
                        let is_svg = url.ends_with(".svg") || bytes_look_like_svg(&bytes);
                        (bytes, is_svg)
                    }
                };
                let bytes = if is_svg {
                    rasterize_svg(&bytes, matches!(key, ImageKey::File(_)), 2048.0, 1.0)?
                } else {
                    bytes
                };
                let image = decode_with_orientation(&bytes)?;
                let size = [image.width() as usize, image.height() as usize];
                Ok(ColorImage::from_rgba_unmultiplied(size, &image.to_rgba8()))
            };
            match read() {
                Ok(image) => {
                    ctx.copy_image(image);
                    ctx.request_repaint();
                }
                Err(err) => tracing::warn!("couldn't copy image: {err}"),
            }
        });
    }

    #[cfg(target_arch = "wasm32")]
    pub fn copy_image(&self, _key: &ImageKey) {}

    /// Whether `key`'s texture is decoded and ready to paint this session — the
    /// real `Loaded` state, not just persisted dims (which outlive the texture).
    pub fn is_loaded(&self, key: &ImageKey) -> bool {
        let inner = self.inner.lock().unwrap();
        inner
            .current
            .get(key)
            .or_else(|| inner.previous.get(key))
            .map(|s| matches!(&*s.lock().unwrap(), ImageState::Loaded(_)))
            .unwrap_or(false)
    }

    /// Test helper: whether any load of `key` has started.
    #[cfg(test)]
    pub fn has_state(&self, key: &ImageKey) -> bool {
        let inner = self.inner.lock().unwrap();
        inner.current.contains_key(key) || inner.previous.contains_key(key)
    }

    /// Test helper: directly install a `Loaded`/`Failed` state for `key`,
    /// populate dims from texture metadata (when `Ok`), and bump `seq` —
    /// mimicking a worker thread completing a load. The dims insert
    /// happens before the state flip to match the spawn_load ordering.
    #[cfg(test)]
    pub fn complete_load(&self, key: &ImageKey, result: Result<TextureId, String>) {
        let state = match result {
            Ok(tid) => {
                if let Some(meta) = self.ctx.tex_manager().read().meta(tid) {
                    let [w, h] = meta.size;
                    self.persistence
                        .merge_image_dims(HashMap::from([(key.persisted(), [w as f32, h as f32])]));
                }
                ImageState::Loaded(tid)
            }
            Err(err) => ImageState::Failed(err),
        };
        let mut inner = self.inner.lock().unwrap();
        inner
            .current
            .insert(key.clone(), Arc::new(Mutex::new(state)));
        self.seq
            .store(self.ws_seq.fetch_add(1, Ordering::Relaxed), Ordering::Relaxed);
    }

    fn spawn_load(&self, key: &ImageKey, user_activity: bool, state: Arc<Mutex<ImageState>>) {
        let key = key.clone();
        let ctx = self.ctx.clone();
        let client = self.client.clone();
        let core = self.core.clone();
        let embeds_seq = self.seq.clone();
        let ws_seq = self.ws_seq.clone();
        let persistence = self.persistence.clone();

        // viewport width is used to scale SVG rasterization — use the screen
        // width as a reasonable upper bound since we load asynchronously
        // before we know the precise render width
        let viewport_width = ctx.content_rect().width();
        let pixels_per_point = ctx.pixels_per_point();

        spawn!({
            let _span = tracing::trace_span!("ImageCache::load").entered();
            let texture_manager = ctx.tex_manager();

            let texture_closure = async_on_wasm!({
                let (image_bytes, is_svg) = match &key {
                    ImageKey::File(id) => {
                        let bytes = core
                            .read_document(*id, user_activity)
                            .map_err(|e| e.to_string())?;
                        let file = core.get_file_by_id(*id).map_err(|e| e.to_string())?;
                        (bytes, file.name.ends_with(".svg"))
                    }
                    ImageKey::Web(url) => {
                        if !url.starts_with("http://") && !url.starts_with("https://") {
                            return Err(format!("image not found: {url}"));
                        }
                        #[cfg(target_arch = "wasm32")]
                        let bytes = download_image(&client, url)
                            .await
                            .map_err(|e| e.to_string())?;
                        #[cfg(not(target_arch = "wasm32"))]
                        let bytes = download_image(&client, url).map_err(|e| e.to_string())?;
                        // Servers can return SVG without a `.svg` suffix
                        // (placehold.co's default); sniff the bytes too.
                        let is_svg = url.ends_with(".svg") || bytes_look_like_svg(&bytes);
                        (bytes, is_svg)
                    }
                };

                let image_bytes = if is_svg {
                    let is_lockbook_svg = matches!(key, ImageKey::File(_));
                    rasterize_svg(&image_bytes, is_lockbook_svg, viewport_width, pixels_per_point)?
                } else {
                    image_bytes
                };

                let image = decode_with_orientation(&image_bytes)?;
                let size_pixels = [image.width() as usize, image.height() as usize];
                let color_image =
                    ColorImage::from_rgba_unmultiplied(size_pixels, &image.to_rgba8());
                let image_data = egui::ImageData::Color(color_image.into());
                Ok(texture_manager
                    .write()
                    .alloc(key.persisted(), image_data, Default::default()))
            });

            #[cfg(target_arch = "wasm32")]
            let result = texture_closure().await;
            #[cfg(not(target_arch = "wasm32"))]
            let result = texture_closure();

            match result {
                Ok(texture_id) => {
                    if let Some(meta) = texture_manager.read().meta(texture_id) {
                        let [w, h] = meta.size;
                        persistence.merge_image_dims(HashMap::from([(
                            key.persisted(),
                            [w as f32, h as f32],
                        )]));
                    }
                    *state.lock().unwrap() = ImageState::Loaded(texture_id);
                }
                Err(err) => {
                    *state.lock().unwrap() = ImageState::Failed(err);
                }
            }

            embeds_seq.store(ws_seq.fetch_add(1, Ordering::Relaxed), Ordering::Relaxed);
            ctx.request_repaint();
        });
    }
}

/// Sniff for SVG bytes — `<?xml` or `<svg` after optional BOM and
/// leading whitespace.
fn bytes_look_like_svg(bytes: &[u8]) -> bool {
    let mut s = bytes;
    if let [0xEF, 0xBB, 0xBF, rest @ ..] = s {
        s = rest;
    }
    while let [first, rest @ ..] = s {
        if first.is_ascii_whitespace() {
            s = rest;
        } else {
            break;
        }
    }
    s.starts_with(b"<?xml") || s.starts_with(b"<svg")
}

fn rasterize_svg(
    bytes: &[u8], is_lockbook_svg: bool, viewport_width: f32, pixels_per_point: f32,
) -> Result<Vec<u8>, String> {
    let tree = usvg::Tree::from_data(bytes, &Default::default()).map_err(|e| e.to_string())?;

    let (w, h, base_transform) = if is_lockbook_svg {
        // lockbook drawings don't have meaningful dimensions; use bounding box
        let bb = tree.root().abs_bounding_box();
        (bb.width(), bb.height(), Transform::identity().post_translate(-bb.left(), -bb.top()))
    } else {
        let size = tree.size();
        (size.width(), size.height(), Transform::default())
    };

    let scale = (viewport_width / w).min(1.0) * pixels_per_point;
    let pix_w = (w * scale) as u32;
    let pix_h = (h * scale) as u32;
    // Pixmap is `w * scale` wide; the transform must match or the SVG
    // draws at natural size into a too-big pixmap's top-left.
    let transform = base_transform.post_scale(scale, scale);

    let mut pix_map = Pixmap::new(pix_w, pix_h)
        .ok_or("failed to create pixmap")
        .map_err(|e| e.to_string())?;
    resvg::render(&tree, transform, &mut pix_map.as_mut());
    pix_map.encode_png().map_err(|e| e.to_string())
}

pub fn encode_png(color_image: &ColorImage) -> Result<Vec<u8>, String> {
    let pixels: Vec<u8> = color_image
        .pixels
        .iter()
        .flat_map(|px| px.to_srgba_unmultiplied())
        .collect();

    let mut png = Vec::new();
    image::codecs::png::PngEncoder::new(&mut png)
        .write_image(
            &pixels,
            color_image.width() as u32,
            color_image.height() as u32,
            image::ExtendedColorType::Rgba8,
        )
        .map_err(|e| e.to_string())?;
    Ok(png)
}

/// Decode image bytes into a `DynamicImage` with EXIF orientation applied.
pub fn decode_with_orientation(image_bytes: &[u8]) -> Result<DynamicImage, String> {
    let reader = image::ImageReader::new(Cursor::new(image_bytes))
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    let mut decoder = reader.into_decoder().map_err(|e| e.to_string())?;
    let orientation = decoder.orientation().map_err(|e| e.to_string())?;
    let mut img = DynamicImage::from_decoder(decoder).map_err(|e| e.to_string())?;
    img.apply_orientation(orientation);
    Ok(img)
}

// Routed through `crate::egress` so images, favicons, and link previews share
// the same SSRF/HTTPS/size guards (these URLs arrive via shared documents).
#[cfg(not(target_arch = "wasm32"))]
fn download_image(client: &HttpClient, url: &str) -> Result<Vec<u8>, String> {
    crate::egress::fetch_bytes(client, url, crate::egress::MAX_IMAGE_BYTES)
        .map_err(|e| e.to_string())
}

#[cfg(target_arch = "wasm32")]
async fn download_image(client: &HttpClient, url: &str) -> Result<Vec<u8>, String> {
    crate::egress::fetch_bytes(client, url, crate::egress::MAX_IMAGE_BYTES)
        .await
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::{bytes_look_like_svg, rasterize_svg};

    /// At ppi>1 the SVG fills the whole upscaled pixmap, not just a
    /// top-left `1/ppi × 1/ppi` quadrant.
    #[test]
    fn svg_rasterization_fills_pixmap_at_hidpi() {
        let svg_str = "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"180\" height=\"120\" viewBox=\"0 0 180 120\"><rect width=\"100%\" height=\"100%\" fill=\"#DDDDDD\"/></svg>";
        let png = rasterize_svg(svg_str.as_bytes(), false, 1920.0, 2.0).unwrap();
        let img = image::ImageReader::new(std::io::Cursor::new(&png))
            .with_guessed_format()
            .unwrap()
            .decode()
            .unwrap()
            .to_rgba8();
        assert_eq!(img.dimensions(), (360, 240));
        assert!(img.get_pixel(350, 120)[3] > 0, "far-right pixel transparent");
    }

    /// Synthetic stand-in for the placehold.co/180x120 response —
    /// rasterized at ppi=2 should yield a 360×240 PNG.
    #[test]
    fn placehold_co_svg_dims() {
        let svg_str = "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"180\" height=\"120\" viewBox=\"0 0 180 120\"><rect width=\"100%\" height=\"100%\" fill=\"#DDDDDD\"/></svg>";
        let png_bytes = rasterize_svg(svg_str.as_bytes(), false, 1920.0, 2.0).unwrap();
        let img = image::ImageReader::new(std::io::Cursor::new(&png_bytes))
            .with_guessed_format()
            .unwrap()
            .decode()
            .unwrap();
        assert_eq!((img.width(), img.height()), (360, 240));
    }

    #[test]
    fn svg_byte_sniff() {
        assert!(bytes_look_like_svg(b"<svg xmlns=\"...\"></svg>"));
        assert!(bytes_look_like_svg(b"<?xml version=\"1.0\"?><svg/>"));
        assert!(bytes_look_like_svg(b"   <svg/>"));
        assert!(bytes_look_like_svg(b"\xEF\xBB\xBF<svg/>"));
        assert!(bytes_look_like_svg(b"\xEF\xBB\xBF\n<?xml ?><svg/>"));
        assert!(!bytes_look_like_svg(b"\x89PNG\r\n\x1a\n"));
        assert!(!bytes_look_like_svg(b"GIF89a"));
        assert!(!bytes_look_like_svg(b"\xFF\xD8\xFF"));
        assert!(!bytes_look_like_svg(b""));
        assert!(!bytes_look_like_svg(b"<html><svg/></html>"));
    }
}
