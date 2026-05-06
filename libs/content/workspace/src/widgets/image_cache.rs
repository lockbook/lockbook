//! Shared image texture cache.
//!
//! URL → `Arc<Mutex<ImageState>>` lookups, double-buffered so images that
//! stop appearing in the frame get evicted (and their textures freed) after
//! one grace frame. Matches the lifecycle pattern used by
//! [`GlyphonCache`](super::glyphon_cache::GlyphonCache).
//!
//! The cache handles URL-to-bytes resolution for both lockbook-internal
//! (`lb://uuid` and relative paths) and http(s) URLs, SVG rasterization via
//! resvg, and EXIF orientation correction. Decoded images are uploaded as
//! egui textures.

use std::collections::HashMap;
use std::io::Cursor;
use std::ops::Deref;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};

use egui::{ColorImage, Context, TextureId};
use image::{DynamicImage, ImageDecoder};
use lb_rs::blocking::Lb;
use lb_rs::{Uuid, spawn};
use resvg::tiny_skia::Pixmap;
use resvg::usvg::{self, Transform};

use crate::file_cache::{FileCache, FilesExt as _, ResolvedLink};
use crate::seq::ws_seq;
use crate::tab::markdown_editor::HttpClient;

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

#[derive(Default)]
struct Inner {
    current: HashMap<String, Arc<Mutex<ImageState>>>,
    previous: HashMap<String, Arc<Mutex<ImageState>>>,
    began_this_frame: bool,
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
}

impl ImageCache {
    pub fn new(ctx: Context, client: HttpClient, core: Lb, files: Arc<RwLock<FileCache>>) -> Self {
        let ws_seq = ws_seq(&ctx);
        Self {
            inner: Default::default(),
            seq: Arc::new(AtomicU64::new(0)),
            ws_seq,
            ctx,
            client,
            core,
            files,
        }
    }

    pub fn seq(&self) -> u64 {
        self.seq.load(Ordering::Relaxed)
    }

    pub fn ctx(&self) -> &Context {
        &self.ctx
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
        // free textures for any URLs that weren't looked up this frame
        let texture_manager = self.ctx.tex_manager();
        for (_, state) in inner.previous.drain() {
            if let ImageState::Loaded(texture_id) = state.lock().unwrap().deref() {
                texture_manager.write().free(*texture_id);
            }
        }
        inner.began_this_frame = false;
    }

    /// Look up or spawn a load for the given URL. `from_file_id` is used to
    /// resolve relative paths and `lb://` URIs against the lockbook file tree.
    /// `user_activity` indicates whether this read should count toward suggested docs.
    ///
    /// Call between `begin_frame` and `end_frame`. URLs not looked up during
    /// a frame are evicted at `end_frame` time.
    pub fn get_or_load(
        &self, url: &str, from_file_id: Uuid, user_activity: bool,
    ) -> Arc<Mutex<ImageState>> {
        let mut inner = self.inner.lock().unwrap();
        if let Some(state) = inner.current.get(url) {
            return state.clone();
        }
        if let Some(state) = inner.previous.remove(url) {
            inner.current.insert(url.to_string(), state.clone());
            return state;
        }

        let state: Arc<Mutex<ImageState>> = Default::default();
        self.spawn_load(
            url,
            from_file_id,
            user_activity,
            state.clone(),
            self.seq.clone(),
            self.ws_seq.clone(),
        );
        inner.current.insert(url.to_string(), state.clone());
        state
    }

    fn spawn_load(
        &self, url: &str, from_file_id: Uuid, user_activity: bool, state: Arc<Mutex<ImageState>>,
        embeds_seq: Arc<AtomicU64>, ws_seq: Arc<AtomicU64>,
    ) {
        let url = url.to_string();
        let ctx = self.ctx.clone();
        let client = self.client.clone();
        let core = self.core.clone();

        let maybe_lb_id = {
            let guard = self.files.read().unwrap();
            let from_id = guard.get_by_id(from_file_id).map(|f| f.parent);
            from_id.and_then(|from_id| match guard.resolve_link(&url, from_id)? {
                ResolvedLink::File(id) => Some(id),
                ResolvedLink::External(_) => None,
            })
        };

        // viewport width is used to scale SVG rasterization — use the screen
        // width as a reasonable upper bound since we load asynchronously
        // before we know the precise render width
        let viewport_width = ctx.screen_rect().width();
        let pixels_per_point = ctx.pixels_per_point();

        spawn!({
            let texture_manager = ctx.tex_manager();

            let texture_closure = async_on_wasm!({
                let image_bytes = if let Some(id) = maybe_lb_id {
                    core.read_document(id, user_activity)
                        .map_err(|e| e.to_string())?
                } else {
                    if !url.starts_with("http://") && !url.starts_with("https://") {
                        return Err(format!("image not found: {url}"));
                    }
                    #[cfg(target_arch = "wasm32")]
                    {
                        download_image(&client, &url)
                            .await
                            .map_err(|e| e.to_string())?
                    }
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        download_image(&client, &url).map_err(|e| e.to_string())?
                    }
                };

                let is_svg = if let Some(id) = maybe_lb_id {
                    let file = core.get_file_by_id(id).map_err(|e| e.to_string())?;
                    file.name.ends_with(".svg")
                } else {
                    url.ends_with(".svg")
                };

                let image_bytes = if is_svg {
                    rasterize_svg(
                        &image_bytes,
                        maybe_lb_id.is_some(),
                        viewport_width,
                        pixels_per_point,
                    )?
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
                    .alloc(url.clone(), image_data, Default::default()))
            });

            #[cfg(target_arch = "wasm32")]
            let result = texture_closure().await;
            #[cfg(not(target_arch = "wasm32"))]
            let result = texture_closure();

            match result {
                Ok(texture_id) => {
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

fn rasterize_svg(
    bytes: &[u8], is_lockbook_svg: bool, viewport_width: f32, pixels_per_point: f32,
) -> Result<Vec<u8>, String> {
    let tree = usvg::Tree::from_data(bytes, &Default::default(), &Default::default())
        .map_err(|e| e.to_string())?;

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
    let transform =
        if scale < 1.0 { base_transform.post_scale(scale, scale) } else { base_transform };

    let mut pix_map = Pixmap::new(pix_w, pix_h)
        .ok_or("failed to create pixmap")
        .map_err(|e| e.to_string())?;
    resvg::render(&tree, transform, &mut pix_map.as_mut());
    pix_map.encode_png().map_err(|e| e.to_string())
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

#[cfg(not(target_arch = "wasm32"))]
fn download_image(client: &HttpClient, url: &str) -> Result<Vec<u8>, reqwest::Error> {
    let response = client.get(url).send()?.bytes()?.to_vec();
    Ok(response)
}

#[cfg(target_arch = "wasm32")]
async fn download_image(client: &HttpClient, url: &str) -> Result<Vec<u8>, reqwest::Error> {
    let response = client.get(url).send().await?.bytes().await?.to_vec();
    Ok(response)
}
