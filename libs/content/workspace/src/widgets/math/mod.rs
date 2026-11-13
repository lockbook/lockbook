//! LaTeX math, converted to Typst by mitex, typeset by Typst and drawn as
//! SVG off the UI thread, then rasterized at the width it's painted, as
//! diagrams are. One cache per egui context, keyed by source and size; a
//! render's key is what layout hands to paint. A size outlives its picture
//! and textures, which go at the end of a frame that doesn't use them, and a
//! texture in an old color or width paints until its redraw lands, so a
//! row's height never waits on a re-render.

use std::collections::{HashMap, HashSet, VecDeque};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use egui::{Color32, ColorImage, Context, Id, TextureHandle, TextureId, TextureOptions, Vec2};
use lb_rs::spawn;
use resvg::usvg;
use serde::{Deserialize, Serialize};
use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime, Duration};
use typst::layout::{Abs, Frame, FrameItem, Point};
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt as _, World};
use typst_layout::PagedDocument;

use crate::seq::ws_seq;
use crate::widgets::diagram_cache::{fit_width, rasterize};

/// A formula by its source: `display` for `$$…$$`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Formula {
    pub latex: String,
    pub display: bool,
}

/// A formula as layout sees it, in points: its box and how far its baseline
/// sits below the box's top.
#[derive(Clone, Copy, Debug)]
pub struct MathSize {
    pub key: u64,
    pub size: Vec2,
    pub baseline: f32,
}

/// Where a formula's render stands.
pub enum Typeset {
    Pending,
    Ready(MathSize),
    /// It doesn't typeset, for the reason given.
    Failed {
        key: u64,
        message: String,
    },
}

#[derive(Clone)]
pub struct MathCache {
    inner: Arc<Mutex<Inner>>,
    seq: Arc<AtomicU64>,
    ws_seq: Arc<AtomicU64>,
    ctx: Context,
}

#[derive(Default)]
struct Inner {
    entries: HashMap<u64, State>,
    began_this_frame: bool,
    /// Formulas looked up or painted this frame, and the textures painted,
    /// by width.
    used: HashSet<u64>,
    drawn: HashSet<(u64, u32)>,
    /// What the last whole frame used: a queued render of anything else,
    /// like a formula since typed over, is dropped.
    wanted: HashSet<u64>,
    /// Renders run one at a time, in the order asked for.
    queue: VecDeque<Job>,
    rendering: bool,
}

enum State {
    Pending,
    Ready {
        size: MathSize,
        /// What the picture and textures are drawn in.
        color: Color32,
        /// The picture, while something looks at it; one let go typesets
        /// again.
        tree: Option<Arc<usvg::Tree>>,
        /// By width in device pixels. Fresh until its first frame ends, so
        /// one landing mid-frame survives that frame's end.
        textures: Vec<(u32, TextureHandle, bool)>,
        job: Job,
        rendering: bool,
    },
    Failed(String),
}

#[derive(Clone)]
struct Job {
    key: u64,
    formula: Formula,
    font_size: f32,
    color: Color32,
    /// The texture to draw with it, else one at its natural size.
    width_px: Option<u32>,
    ppp: f32,
}

impl MathCache {
    pub fn of(ctx: &Context) -> Self {
        let ws_seq = ws_seq(ctx);
        // the context keeps all but itself, which would keep it alive
        let (inner, seq, ws_seq) = ctx.data_mut(|d| {
            d.get_temp_mut_or_insert_with(Id::unique("math_cache"), || {
                (Default::default(), Arc::new(AtomicU64::new(0)), ws_seq)
            })
            .clone()
        });
        Self { inner, seq, ws_seq, ctx: ctx.clone() }
    }

    /// Starts a frame: pictures not looked up and textures not painted by
    /// its end are let go.
    pub fn begin_frame(&self) {
        let mut inner = self.lock();
        if !inner.began_this_frame {
            inner.began_this_frame = true;
            inner.used.clear();
            inner.drawn.clear();
        }
    }

    pub fn end_frame(&self) {
        let mut inner = self.lock();
        let Inner { entries, used, drawn, .. } = &mut *inner;
        for (key, state) in entries.iter_mut() {
            let State::Ready { tree, textures, rendering: false, .. } = state else {
                continue;
            };
            if !used.contains(key) {
                *tree = None;
            }
            textures
                .retain_mut(|(w, _, fresh)| std::mem::take(fresh) || drawn.contains(&(*key, *w)));
        }
        inner.wanted = std::mem::take(&mut inner.used);
        inner.began_this_frame = false;
    }

    /// Bumps whenever a render lands; layout depends on it.
    pub fn seq(&self) -> u64 {
        self.seq.load(Ordering::Relaxed)
    }

    /// The formula's size, queueing its render if it's new.
    pub fn size(&self, formula: &Formula, text_size: f32, color: Color32) -> Typeset {
        let (key, font_size) = key(formula, text_size);
        let mut inner = self.lock();
        inner.used.insert(key);
        match inner.entries.get(&key) {
            Some(State::Ready { size, .. }) => return Typeset::Ready(*size),
            Some(State::Failed(message)) => {
                return Typeset::Failed { key, message: message.clone() };
            }
            Some(State::Pending) => return Typeset::Pending,
            None => {}
        }
        inner.entries.insert(key, State::Pending);
        let ppp = self.ctx.pixels_per_point();
        let job = Job { key, formula: formula.clone(), font_size, color, width_px: None, ppp };
        self.queue(inner, job);
        if cfg!(test) { self.size(formula, text_size, color) } else { Typeset::Pending }
    }

    /// The formula's size at `key` in points, once it has typeset.
    pub fn natural_size(&self, key: u64) -> Option<Vec2> {
        match self.lock().entries.get(&key) {
            Some(State::Ready { size, .. }) => Some(size.size),
            _ => None,
        }
    }

    /// The texture to paint `width_px` device pixels wide in `color`, if
    /// any. One of another width or color paints while this one draws.
    pub fn texture(&self, key: u64, color: Color32, width_px: u32) -> Option<TextureId> {
        let mut inner = self.lock();
        inner.used.insert(key);
        let Inner { entries, drawn, .. } = &mut *inner;
        let Some(State::Ready { color: drawn_in, tree, textures, job, rendering, .. }) =
            entries.get_mut(&key)
        else {
            return None;
        };
        let same_color = *drawn_in == color;
        let exact = same_color && textures.iter().any(|(w, ..)| *w == width_px);
        let shown = textures
            .iter_mut()
            .min_by_key(|(w, ..)| w.abs_diff(width_px))
            .map(|(w, texture, _)| {
                drawn.insert((key, *w));
                texture.id()
            });
        if exact || std::mem::replace(rendering, true) {
            return shown;
        }
        match tree.clone().filter(|_| same_color) {
            Some(tree) => {
                drop(inner);
                self.spawn_raster(key, tree, width_px);
            }
            None => {
                (job.color, job.width_px) = (color, Some(width_px));
                let job = job.clone();
                self.queue(inner, job);
            }
        }
        shown
    }

    /// `formula` in `color` on `background` with a margin, at twice the size
    /// it has at `text_size`, for the clipboard: from the tree its texture
    /// came from when there is one, as a diagram's is.
    pub fn image(
        &self, formula: &Formula, text_size: f32, color: Color32, background: Color32,
    ) -> Option<ColorImage> {
        const MARGIN: f32 = 16.0;
        let (key, font_size) = key(formula, text_size);
        let cached = match self.lock().entries.get(&key) {
            Some(State::Ready { size, color: c, tree: Some(tree), .. }) if *c == color => {
                Some((tree.clone(), size.size))
            }
            _ => None,
        };
        let (tree, size) = match cached {
            Some(cached) => cached,
            None => render(formula, font_size, color)
                .map(|(t, s, _)| (t, s))
                .ok()?,
        };
        let width = fit_width(&tree, (size.x * 2.0) as u32, 4096);
        let margin = (MARGIN * width as f32 / size.x).round() as u32;
        rasterize(&tree, width, Some((background, margin))).ok()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap()
    }

    /// Queues a render, starting the render thread if it's idle.
    fn queue(&self, mut inner: std::sync::MutexGuard<'_, Inner>, job: Job) {
        inner.queue.push_back(job);
        if !std::mem::replace(&mut inner.rendering, true) {
            drop(inner);
            let this = self.clone();
            run(move || {
                while let Some(job) = this.next_job() {
                    this.run_job(job);
                }
            });
        }
    }

    /// The next queued render someone still looks at, or none, stopping the
    /// render thread.
    fn next_job(&self) -> Option<Job> {
        let mut inner = self.lock();
        while let Some(job) = inner.queue.pop_front() {
            if inner.used.contains(&job.key) || inner.wanted.contains(&job.key) {
                return Some(job);
            }
            match inner.entries.get_mut(&job.key) {
                Some(State::Ready { rendering, .. }) => *rendering = false,
                _ => {
                    inner.entries.remove(&job.key);
                }
            }
        }
        inner.rendering = false;
        None
    }

    fn run_job(&self, job: Job) {
        let rendered =
            render(&job.formula, job.font_size, job.color).map(|(tree, size, baseline)| {
                let width_px = job.width_px.unwrap_or((size.x * job.ppp).round() as u32);
                let texture = self
                    .upload(job.key, &tree, width_px)
                    .map(|t| (width_px, t, true));
                (tree, MathSize { key: job.key, size, baseline }, texture)
            });
        let mut inner = self.lock();
        let Some(state) = inner.entries.get_mut(&job.key) else { return };
        let landed_size = matches!(state, State::Pending);
        match (state, rendered) {
            (
                State::Ready { color, tree, textures, job: last, rendering, .. },
                Ok((t, _, texture)),
            ) => {
                if *color != job.color {
                    textures.clear();
                    *color = job.color;
                }
                *tree = Some(t);
                textures.extend(texture);
                *last = job;
                *rendering = false;
            }
            (state, Ok((tree, size, texture))) => {
                let color = job.color;
                let textures = texture.into_iter().collect();
                *state =
                    State::Ready { size, color, tree: Some(tree), textures, job, rendering: false };
            }
            (state, Err(message)) => *state = State::Failed(message),
        }
        drop(inner);
        if landed_size {
            self.seq
                .store(self.ws_seq.fetch_add(1, Ordering::Relaxed), Ordering::Relaxed);
        }
        self.ctx.request_repaint();
    }

    fn spawn_raster(&self, key: u64, tree: Arc<usvg::Tree>, width_px: u32) {
        let this = self.clone();
        run(move || {
            let texture = this.upload(key, &tree, width_px);
            let mut inner = this.lock();
            if let Some(State::Ready { textures, rendering, .. }) = inner.entries.get_mut(&key) {
                *rendering = false;
                textures.extend(texture.map(|t| (width_px, t, true)));
            }
            drop(inner);
            this.ctx.request_repaint();
        });
    }

    fn upload(&self, key: u64, tree: &usvg::Tree, width_px: u32) -> Option<TextureHandle> {
        let max_side = self.ctx.input(|i| i.max_texture_side) as u32;
        let image = rasterize(tree, fit_width(tree, width_px, max_side), None).ok()?;
        Some(
            self.ctx
                .load_texture(format!("math-{key:016x}"), image, TextureOptions::LINEAR),
        )
    }
}

/// Off the UI thread, except in tests, which want renders to land in order.
fn run(job: impl FnOnce() + Send + 'static) {
    if cfg!(test) {
        job();
    } else {
        spawn!(job());
    }
}

const TEXT_FONT: &[u8] = include_bytes!("NewCM10-Regular.otf");

/// How much larger than the text's size math draws so its lowercase stands
/// as tall as the text's.
fn x_height_match() -> f32 {
    static MATH: OnceLock<Option<f32>> = OnceLock::new();
    let math = *MATH.get_or_init(|| crate::font::face_letter_height(TEXT_FONT, 0, 'x'));
    crate::font::x_height(false)
        .zip(math)
        .map_or(1., |(text, math)| text / math)
}

/// The formula as SVG, parsed, with its size and baseline in points.
fn render(
    formula: &Formula, font_size: f32, color: Color32,
) -> Result<(Arc<usvg::Tree>, Vec2, f32), String> {
    let typst_math = mitex::convert_math(&formula.latex, None)?;
    let source = main_source(&typst_math, formula.display, font_size, color);
    let mut world = world().lock().unwrap();
    world.main = Source::new(file_id("/main.typ"), source);
    let doc: PagedDocument = typst::compile(&*world).output.map_err(|errs| {
        errs.iter()
            .map(|e| e.message.to_string())
            .collect::<Vec<_>>()
            .join("; ")
    })?;
    drop(world);
    let page = doc.pages().first().ok_or("no page")?;
    let baseline = marker_baseline(&page.frame, Point::zero()).ok_or("no baseline")?;
    let size = Vec2::new(page.frame.width().to_pt() as f32, page.frame.height().to_pt() as f32);
    let svg = typst_svg::svg(page, &Default::default());
    let tree = usvg::Tree::from_str(&svg, &Default::default()).map_err(|e| e.to_string())?;
    Ok((Arc::new(tree), size, baseline.to_pt() as f32))
}

/// A 1pt glyph before the formula marks the line's baseline.
const MARKER_SIZE: f64 = 1.0;

/// The cache key of `formula` at `text_size`, and the font size it sets in.
fn key(formula: &Formula, text_size: f32) -> (u64, f32) {
    let font_size = text_size * x_height_match();
    let mut h = DefaultHasher::new();
    (formula, font_size.to_bits()).hash(&mut h);
    (h.finish(), font_size)
}

fn main_source(typst_math: &str, display: bool, font_size: f32, color: Color32) -> String {
    let math = if display { format!("display({typst_math})") } else { typst_math.into() };
    let math = math.replace('\\', "\\\\").replace('"', "\\\"");
    let [r, g, b, a] = color.to_srgba_unmultiplied();
    format!(
        r#"#import "/specs/mod.typ": mitex-scope
#set page(width: auto, height: auto, margin: 0pt, fill: none)
#set text(font: "New Computer Modern", size: {font_size}pt, fill: rgb({r}, {g}, {b}, {a}), top-edge: "bounds", bottom-edge: "bounds")
#show math.equation: set text(font: "New Computer Modern Math")
#text(size: {MARKER_SIZE}pt, fill: rgb(0, 0, 0, 0))[x]#math.equation(block: false, eval("$" + "{math}" + "$", scope: mitex-scope))
"#
    )
}

fn marker_baseline(frame: &Frame, origin: Point) -> Option<Abs> {
    for (pos, item) in frame.items() {
        let at = origin + *pos;
        match item {
            FrameItem::Text(t) if t.size == Abs::pt(MARKER_SIZE) => return Some(at.y),
            FrameItem::Group(g) => {
                let shift = Point::new(g.transform.tx, g.transform.ty);
                if let Some(y) = marker_baseline(&g.frame, at + shift) {
                    return Some(y);
                }
            }
            _ => {}
        }
    }
    None
}

/// One world for every render; each render swaps in its own main source.
fn world() -> &'static Mutex<MathWorld> {
    static WORLD: OnceLock<Mutex<MathWorld>> = OnceLock::new();
    WORLD.get_or_init(|| {
        let fonts: Vec<Font> = [&include_bytes!("NewCMMath-Regular.otf")[..], TEXT_FONT]
            .iter()
            .flat_map(|data| Font::iter(Bytes::new(*data)))
            .collect();
        Mutex::new(MathWorld {
            library: LazyHash::new(Library::default()),
            book: LazyHash::new(FontBook::from_fonts(&fonts)),
            fonts,
            main: Source::detached(""),
        })
    })
}

fn file_id(path: &str) -> FileId {
    RootedPath::new(VirtualRoot::Project, VirtualPath::new(path).unwrap()).intern()
}

struct MathWorld {
    library: LazyHash<Library>,
    book: LazyHash<FontBook>,
    fonts: Vec<Font>,
    main: Source,
}

impl World for MathWorld {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }
    fn book(&self) -> &LazyHash<FontBook> {
        &self.book
    }
    fn main(&self) -> FileId {
        self.main.id()
    }
    fn source(&self, id: FileId) -> FileResult<Source> {
        if id == self.main.id() {
            return Ok(self.main.clone());
        }
        mitex_spec_gen::TYPST_SPECS
            .iter()
            .find(|(p, _)| file_id(&format!("/specs/{p}")) == id)
            .map(|(_, text)| Source::new(id, text.to_string()))
            .ok_or_else(|| FileError::NotFound(id.vpath().get_without_slash().into()))
    }
    fn file(&self, id: FileId) -> FileResult<Bytes> {
        Err(FileError::NotFound(id.vpath().get_without_slash().into()))
    }
    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.get(index).cloned()
    }
    fn today(&self, _: Option<Duration>) -> Option<Datetime> {
        None
    }
}
