//! Shared image texture cache.
//!
//! [`ImageKey`] → `Arc<Mutex<ImageState>>` lookups, double-buffered so
//! images that stop appearing in the frame get evicted (and their textures
//! freed) after one grace frame. Matches the lifecycle pattern used by
//! [`GlyphonCache`](super::glyphon_cache::GlyphonCache).
//!
//! The cache loads lockbook files and http(s) URLs, rasterizes SVG via
//! resvg, and corrects EXIF orientation. Decoded images are uploaded as
//! egui textures. Animated gifs, WebPs and PNGs play: a [`Player`] swaps
//! each frame into the image's one texture as the frame clock reaches it.

use std::collections::{HashMap, HashSet};
use std::io::Cursor;
use std::ops::Deref;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

use egui::{ColorImage, Context, ImageData, TextureId, Vec2};
use epaint::ImageDelta;
use futures::channel::mpsc;
use image::codecs::gif::GifDecoder;
use image::codecs::png::PngDecoder;
use image::codecs::webp::WebPDecoder;
use image::metadata::LoopCount;
use image::{
    AnimationDecoder as _, DynamicImage, Frames, ImageDecoder, ImageEncoder as _, ImageFormat,
    ImageResult,
};
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
        (async || -> Result<Decoded, String> { $block })
    };
}
#[cfg(not(target_arch = "wasm32"))]
macro_rules! async_on_wasm {
    ($block:block) => {
        (|| -> Result<Decoded, String> { $block })
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
    /// Animations that are loaded and still playing; evicted with their texture.
    players: HashMap<ImageKey, Player>,
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
        let now = self.ctx.input(|i| i.time);
        let mut inner = self.inner.lock().unwrap();
        if !inner.began_this_frame {
            inner.began_this_frame = true;
            inner.previous = std::mem::take(&mut inner.current);
            self.advance_players(&mut inner, now);
        }
    }

    /// Shows each animation's next frame once the clock reaches it, and asks
    /// for a repaint when the soonest one is due.
    fn advance_players(&self, inner: &mut Inner, now: f64) {
        let mut ended = Vec::new();
        let mut soonest = f64::INFINITY;
        for (key, player) in &mut inner.players {
            let due = *player.due.get_or_insert(now + player.delay);
            if now >= due {
                // set before looking, so a frame sent meanwhile still repaints
                player.starved.store(true, Ordering::SeqCst);
                match player.frames.try_recv() {
                    Ok(frame) => {
                        player.starved.store(false, Ordering::SeqCst);
                        self.ctx
                            .tex_manager()
                            .write()
                            .set(player.texture, ImageDelta::full(frame.image, Default::default()));
                        // a frame later than one delay sets the clock from now
                        let next = due + frame.delay;
                        player.due = Some(if next > now { next } else { now + frame.delay });
                        player.delay = frame.delay;
                    }
                    Err(mpsc::TryRecvError::Closed) => {
                        ended.push(key.clone());
                        continue;
                    }
                    Err(mpsc::TryRecvError::Empty) => continue,
                }
            }
            soonest = soonest.min(player.due.unwrap_or(now));
        }
        for key in ended {
            inner.players.remove(&key);
        }
        if soonest.is_finite() {
            self.ctx.request_repaint_after_secs((soonest - now) as f32);
        }
    }

    pub fn end_frame(&self) {
        let mut guard = self.inner.lock().unwrap();
        let inner = &mut *guard;
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
                inner.players.remove(&key);
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

    /// The file's pixels, an animation's first frame, and its bytes.
    pub fn copied_image(&self, id: Uuid) -> Result<(ColorImage, Vec<u8>), String> {
        let bytes = self
            .core
            .read_document(id, false)
            .map_err(|e| e.to_string())?;
        let image = decode_with_orientation(&bytes)?;
        let size = [image.width() as usize, image.height() as usize];
        Ok((ColorImage::from_rgba_unmultiplied(size, &image.to_rgba8()), bytes))
    }

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
        let client = self.client.clone();
        let core = self.core.clone();
        let loader = self.loader();

        // viewport width is used to scale SVG rasterization — use the screen
        // width as a reasonable upper bound since we load asynchronously
        // before we know the precise render width
        let viewport_width = self.ctx.content_rect().width();
        let pixels_per_point = self.ctx.pixels_per_point();

        spawn!({
            let span = tracing::trace_span!("ImageCache::load").entered();

            let decode_closure = async_on_wasm!({
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

                decode(image_bytes)
            });

            #[cfg(target_arch = "wasm32")]
            let result = decode_closure().await;
            #[cfg(not(target_arch = "wasm32"))]
            let result = decode_closure();

            let playback = loader.install(&key, &state, result);
            // a player outlives its loader, so dropping the cache closes the channel
            drop((span, loader));
            if let Some(playback) = playback {
                #[cfg(target_arch = "wasm32")]
                play(playback).await;
                #[cfg(not(target_arch = "wasm32"))]
                futures::executor::block_on(play(playback));
            }
        });
    }

    fn loader(&self) -> Loader {
        Loader {
            ctx: self.ctx.clone(),
            inner: self.inner.clone(),
            seq: self.seq.clone(),
            ws_seq: self.ws_seq.clone(),
            persistence: self.persistence.clone(),
        }
    }

    /// Test helper: load `bytes` as `key` on a worker, as a fetch would.
    #[cfg(test)]
    pub fn load_bytes(&self, key: &ImageKey, bytes: Vec<u8>) {
        let state: Arc<Mutex<ImageState>> = Default::default();
        self.inner
            .lock()
            .unwrap()
            .current
            .insert(key.clone(), state.clone());
        let (key, loader) = (key.clone(), self.loader());
        std::thread::spawn(move || {
            let playback = loader.install(&key, &state, decode(bytes));
            drop(loader);
            if let Some(playback) = playback {
                futures::executor::block_on(play(playback));
            }
        });
    }
}

/// What a finished load needs from the cache.
struct Loader {
    ctx: Context,
    inner: Arc<Mutex<Inner>>,
    seq: Arc<AtomicU64>,
    ws_seq: Arc<AtomicU64>,
    persistence: WsPersistentStore,
}

impl Loader {
    /// Shows a decoded image: its first frame becomes its texture. Returns
    /// the rest of an animation, for [`play`] to feed the texture.
    fn install(
        &self, key: &ImageKey, state: &Mutex<ImageState>, result: Result<Decoded, String>,
    ) -> Option<Playback> {
        let mut playback = None;
        let shown = result.map(|decoded| {
            let (image, rest) = match decoded {
                Decoded::Still(image) => (image, None),
                Decoded::Animated(first, rest) => (first.image, Some((first.delay, rest))),
            };
            let [w, h] = image.size;
            self.persistence
                .merge_image_dims(HashMap::from([(key.persisted(), [w as f32, h as f32])]));
            let texture = self.ctx.tex_manager().write().alloc(
                key.persisted(),
                ImageData::Color(image),
                Default::default(),
            );
            if let Some((delay, rest)) = rest {
                let (tx, frames) = mpsc::channel(1);
                let starved = Arc::new(AtomicBool::new(false));
                let player = Player { texture, frames, due: None, delay, starved: starved.clone() };
                self.inner
                    .lock()
                    .unwrap()
                    .players
                    .insert(key.clone(), player);
                playback = Some(Playback { rest, tx, starved, ctx: self.ctx.clone() });
            }
            texture
        });
        *state.lock().unwrap() = match shown {
            Ok(texture) => ImageState::Loaded(texture),
            Err(err) => ImageState::Failed(err),
        };
        self.seq
            .store(self.ws_seq.fetch_add(1, Ordering::Relaxed), Ordering::Relaxed);
        self.ctx.request_repaint();
        playback
    }
}

/// A decoded image: a still, or an animation's first frame and the rest.
enum Decoded {
    Still(Arc<ColorImage>),
    Animated(Frame, Rest),
}

/// One frame of an animation, ready to upload.
struct Frame {
    image: Arc<ColorImage>,
    /// Seconds it stays up.
    delay: f64,
}

impl From<image::Frame> for Frame {
    fn from(frame: image::Frame) -> Self {
        let delay = Duration::from(frame.delay()).as_secs_f64();
        let buffer = frame.into_buffer();
        let size = [buffer.width() as usize, buffer.height() as usize];
        let image = ColorImage::from_rgba_unmultiplied(size, buffer.as_raw());
        // browsers play the shortest delays at a tenth of a second
        Frame { image: Arc::new(image), delay: if delay <= 0.01 { 0.1 } else { delay } }
    }
}

/// An animation past its first frame: the frames still to decode, and the
/// bytes to decode them again for the next play.
struct Rest {
    bytes: Arc<[u8]>,
    frames: Frames<'static>,
    next: Frame,
    /// Times it plays in all; `None` plays forever.
    plays: Option<u32>,
}

/// A loaded animation's frame clock. Its texture shows the current frame;
/// [`play`] decodes ahead into `frames`.
struct Player {
    texture: TextureId,
    frames: mpsc::Receiver<Frame>,
    /// Frame-clock time the current frame comes down; set on the first tick.
    due: Option<f64>,
    /// How long the current frame stays up.
    delay: f64,
    /// The clock found no frame ready; the next one sent repaints.
    starved: Arc<AtomicBool>,
}

struct Playback {
    rest: Rest,
    tx: mpsc::Sender<Frame>,
    starved: Arc<AtomicBool>,
    ctx: Context,
}

/// Decodes an animation's frames into its player's channel, as far ahead as
/// the channel holds, until the player is evicted or the plays run out.
async fn play(playback: Playback) {
    use futures::SinkExt as _;

    let Playback { rest: Rest { bytes, mut frames, next, plays }, mut tx, starved, ctx } = playback;
    let mut played = 1;
    let mut pending = Some(next);
    loop {
        let frame = match pending.take() {
            Some(frame) => frame,
            None => match frames.next() {
                Some(Ok(frame)) => frame.into(),
                Some(Err(err)) => {
                    tracing::warn!(%err, "animation stopped");
                    return;
                }
                None if plays.is_some_and(|n| played >= n) => return,
                None => {
                    played += 1;
                    match animation(&bytes) {
                        Ok(Some((again, _))) => frames = again,
                        _ => return,
                    }
                    continue;
                }
            },
        };
        if tx.send(frame).await.is_err() {
            return; // evicted
        }
        if starved.swap(false, Ordering::SeqCst) {
            ctx.request_repaint();
        }
    }
}

/// Decodes `bytes` as an animation if it has more than one frame, else as a
/// still with its EXIF orientation applied.
fn decode(bytes: Vec<u8>) -> Result<Decoded, String> {
    let bytes: Arc<[u8]> = bytes.into();
    if let Some((mut frames, plays)) = animation(&bytes).map_err(|e| e.to_string())? {
        let first: Frame = frames
            .next()
            .ok_or("image has no frames")?
            .map_err(|e| e.to_string())?
            .into();
        // a file cut short after its first frame shows that frame
        return Ok(match frames.next() {
            Some(Ok(next)) => {
                Decoded::Animated(first, Rest { bytes, frames, next: next.into(), plays })
            }
            _ => Decoded::Still(first.image),
        });
    }
    let image = decode_with_orientation(&bytes)?;
    let size = [image.width() as usize, image.height() as usize];
    Ok(Decoded::Still(Arc::new(ColorImage::from_rgba_unmultiplied(size, &image.to_rgba8()))))
}

/// The frames of an animated gif, WebP or PNG, and the times it plays
/// (`None` forever). `None` for any other image.
fn animation(bytes: &Arc<[u8]>) -> ImageResult<Option<(Frames<'static>, Option<u32>)>> {
    let plays = |count| match count {
        LoopCount::Infinite => None,
        LoopCount::Finite(n) => Some(u32::from(n)),
    };
    let reader = Cursor::new(bytes.clone());
    Ok(match image::guess_format(bytes)? {
        ImageFormat::Gif => Some((GifDecoder::new(reader)?.into_frames(), gif_plays(bytes))),
        ImageFormat::WebP => {
            let decoder = WebPDecoder::new(reader)?;
            if !decoder.has_animation() {
                return Ok(None);
            }
            let plays = plays(decoder.loop_count());
            Some((decoder.into_frames(), plays))
        }
        ImageFormat::Png => {
            let decoder = PngDecoder::new(reader)?;
            if !decoder.is_apng()? {
                return Ok(None);
            }
            let decoder = decoder.apng()?;
            let plays = plays(decoder.loop_count());
            Some((decoder.into_frames(), plays))
        }
        _ => None,
    })
}

/// Times a gif plays, as Firefox and WebKit read its loop block: once
/// without one, else one more than its count; `None` forever.
fn gif_plays(bytes: &[u8]) -> Option<u32> {
    let repeat = gif::DecodeOptions::new()
        .read_info(bytes)
        .map_or(gif::Repeat::Finite(0), |decoder| decoder.repeat());
    match repeat {
        gif::Repeat::Infinite => None,
        gif::Repeat::Finite(n) => Some(u32::from(n) + 1),
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
    use std::sync::{Arc, RwLock};
    use std::time::{Duration, Instant};

    use egui::{Color32, ImageData, RawInput, TextureId};
    use lb_rs::Uuid;

    use super::{ImageCache, ImageKey, bytes_look_like_svg, rasterize_svg};
    use crate::file_cache::FileCache;
    use crate::workspace::WsPersistentStore;

    /// A 4×4 gif of solid frames, each with its delay in centiseconds.
    fn solid_gif(frames: &[(Color32, u16)], repeat: Option<gif::Repeat>) -> Vec<u8> {
        let mut out = Vec::new();
        let mut encoder = gif::Encoder::new(&mut out, 4, 4, &[]).unwrap();
        if let Some(repeat) = repeat {
            encoder.set_repeat(repeat).unwrap();
        }
        for &(color, delay) in frames {
            let mut pixels = color.to_array().repeat(16);
            let mut frame = gif::Frame::from_rgba(4, 4, &mut pixels);
            frame.delay = delay;
            encoder.write_frame(&frame).unwrap();
        }
        drop(encoder);
        out
    }

    /// A gif plays as encoded: each frame goes up once the delays before it
    /// have passed and not before, in order, for as many plays as its loop
    /// block asks, however late the frame clock ticks. A delay of 0 plays as
    /// 10 centiseconds.
    #[test]
    fn gif_plays_on_the_frame_clock() {
        let frames = [(Color32::RED, 2), (Color32::GREEN, 0), (Color32::BLUE, 5)];
        let delay = |cs: u16| if cs == 0 { 0.1 } else { cs as f64 / 100.0 };
        // (loop block, plays checked, whether it stops after them)
        let cases = [
            (None, 1, true),
            (Some(gif::Repeat::Finite(1)), 2, true),
            (Some(gif::Repeat::Infinite), 3, false),
        ];
        for (repeat, plays, stops) in cases {
            let ctx = crate::tab::markdown_editor::test_egui_ctx();
            let persistence =
                WsPersistentStore::new(false, format!("/tmp/{}", Uuid::new_v4()).into(), true);
            let images = ImageCache::new(
                ctx.clone(),
                Default::default(),
                crate::tab::markdown_editor::tests::harness::build_lb(),
                Arc::new(RwLock::new(FileCache::empty())),
                persistence,
            );
            let key = ImageKey::Web("https://example.com/solid.gif".into());
            images.load_bytes(&key, solid_gif(&frames, repeat));

            // the colors uploaded in one frame at `time`; the font atlas
            // aside, the gif's is the only texture
            let run = |time: f64| -> Vec<Color32> {
                let mut out =
                    ctx.run_pass(RawInput { time: Some(time), ..Default::default() }, |_| {
                        images.begin_frame();
                        images.get_or_load(&key, false);
                        images.end_frame();
                    });
                let shown = out
                    .textures_delta
                    .set
                    .iter()
                    .filter(|(id, _)| **id != TextureId::default())
                    .flat_map(|(_, deltas)| deltas)
                    .map(|delta| match &delta.image {
                        ImageData::Color(image) => image.pixels[0],
                    })
                    .collect();
                out.textures_delta.clear();
                shown
            };
            // frames at `time` until one uploads, waiting on the decoder
            let run_until_upload = |time: f64| -> Vec<Color32> {
                let start = Instant::now();
                loop {
                    let shown = run(time);
                    if !shown.is_empty() || start.elapsed() > Duration::from_secs(5) {
                        return shown;
                    }
                    std::thread::sleep(Duration::from_millis(1));
                }
            };

            // the first frame goes up as the texture, then the clock starts
            assert_eq!(run_until_upload(0.0), vec![frames[0].0]);
            run(0.0);

            let mut due = 0.0;
            let mut shown_delay = delay(frames[0].1);
            for (i, &(color, cs)) in frames.iter().cycle().enumerate().skip(1) {
                if i == plays * frames.len() {
                    break;
                }
                due += shown_delay;
                // a frame shown late leaves the next one's time alone
                let late = if i % 2 == 1 { 0.004 } else { 0.0 };
                assert!(run(due - 0.001).is_empty(), "frame {i} early");
                assert_eq!(run_until_upload(due + late), vec![color], "frame {i}");
                shown_delay = delay(cs);
            }
            if stops {
                std::thread::sleep(Duration::from_millis(20));
                assert!(run(due + 10.0).is_empty());
                assert!(run(due + 20.0).is_empty());
            }
        }
    }

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
