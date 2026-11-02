//! Mermaid diagrams, rendered off the UI thread and kept as textures. One
//! cache per egui context, so notes, previews, and chat share renders.
//!
//! A render is keyed by source and theme. Its natural size outlives its
//! parsed tree and textures, which go at the end of a frame that doesn't use
//! them, so a block's height never waits on a re-render.

use std::collections::{HashMap, HashSet, VecDeque};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use egui::{Color32, ColorImage, Context, Id, TextureHandle, TextureId, TextureOptions, Vec2};
use glyphon::cosmic_text::{
    CacheKey, CacheKeyFlags, Command, SwashCache, SwashContent, SwashImage,
};
use glyphon::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping, Style, Weight};
use lb_rs::spawn;
use merman::svg::{
    CssOverridePolicy, DeterministicTextMeasurer, HostTheme, HostThemeAppearance,
    MeasurementProfileId, Presentation, PresentationProfile, SvgOutputPolicy, SvgPipelinePreset,
    TextMeasurementPolicy, TextMeasurementProfile, TextMeasurementProfileIdentity, TextStyle,
    ThemeRole,
};
use merman::{OperationControl, RenderOutput, RenderRequest, Renderer, SvgEnvironment, SvgRequest};
use resvg::tiny_skia::{Pixmap, Transform};
use resvg::usvg;

use crate::seq::ws_seq;
use crate::theme::palette_v2::{Theme, ThemeExt as _};

/// A diagram as layout sees it, in points.
#[derive(Clone, Debug, PartialEq)]
pub enum DiagramSize {
    Pending,
    Ready(Vec2),
    Failed(String),
}

#[derive(Clone)]
pub struct DiagramCache {
    inner: Arc<Mutex<Inner>>,
    seq: Arc<AtomicU64>,
    ws_seq: Arc<AtomicU64>,
    ctx: Context,
}

#[derive(Default)]
struct Inner {
    /// By source and theme.
    entries: HashMap<u64, State>,
    /// The theme renders take, read at a frame's first lookup, and its hash.
    theme: Option<DiagramTheme>,
    theme_key: u64,
    began_this_frame: bool,
    /// Diagrams looked up this frame, and the textures drawn, by width.
    used: HashSet<u64>,
    drawn: HashSet<(u64, u32)>,
    /// What the last whole frame looked up: a queued render of anything
    /// else, like a source since typed over, is dropped.
    wanted: HashSet<u64>,
    /// Renders run one at a time, in the order asked for.
    queue: VecDeque<Job>,
    rendering: bool,
    /// The render running, stopped once nothing looks at it.
    running: Option<(u64, OperationControl)>,
}

enum State {
    Pending,
    Ready {
        size: Vec2,
        tree: Option<Arc<usvg::Tree>>,
        /// By width in device pixels: one source painted at two widths keeps
        /// both. Fresh until its first frame ends, so one landing mid-frame
        /// survives that frame's end.
        textures: Vec<(u32, TextureHandle, bool)>,
        rastering: bool,
    },
    Failed(String),
}

/// A render to run, and the width to raster it at, if one is waiting.
struct Job {
    key: u64,
    source: String,
    theme: DiagramTheme,
    width_px: Option<u32>,
}

/// Colors and appearance a render depends on, as merman takes them.
#[derive(Clone, Hash)]
struct DiagramTheme {
    dark: bool,
    roles: Vec<(usize, String)>,
    series: Vec<String>,
    background: [u8; 3],
}

impl DiagramTheme {
    fn new(theme: &Theme) -> Self {
        let hex = |c: Color32| format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b());
        let bg = theme.neutral_bg();
        let fg = theme.fg();
        let note = bg.lerp_to_gamma(fg.yellow, 0.18);
        let roles = [
            (ThemeRole::Canvas, bg),
            (ThemeRole::Surface, theme.neutral_bg_secondary()),
            (ThemeRole::SurfaceAlt, theme.neutral_bg_tertiary()),
            (ThemeRole::SurfaceMuted, theme.neutral_bg_secondary()),
            (ThemeRole::Text, theme.neutral_fg()),
            (ThemeRole::SubtleText, theme.neutral_fg_secondary()),
            (ThemeRole::Border, theme.neutral()),
            (ThemeRole::Line, theme.neutral_fg_secondary()),
            (ThemeRole::EdgeLabelBackground, bg),
            (ThemeRole::ClusterBackground, bg.lerp_to_gamma(theme.neutral_bg_secondary(), 0.5)),
            (ThemeRole::ClusterBorder, theme.neutral_bg_tertiary()),
            (ThemeRole::NoteBackground, note),
            (ThemeRole::NoteBorder, fg.yellow),
            (ThemeRole::NoteText, theme.neutral_fg()),
            (ThemeRole::ActorBackground, theme.neutral_bg_secondary()),
            (ThemeRole::ActorBorder, theme.neutral()),
            (ThemeRole::ActorText, theme.neutral_fg()),
            (ThemeRole::ActivationBackground, theme.neutral_bg_tertiary()),
            (ThemeRole::ActivationBorder, theme.neutral()),
            (ThemeRole::Error, fg.red),
            (ThemeRole::Warning, fg.yellow),
            (ThemeRole::Success, fg.green),
        ];
        let roles = roles
            .into_iter()
            .map(|(role, c)| (ThemeRole::ALL.iter().position(|r| *r == role).unwrap(), hex(c)))
            .collect();
        let series = [theme.accent(), fg.green, fg.yellow, fg.magenta, fg.cyan, fg.red, fg.blue]
            .into_iter()
            .map(hex)
            .collect();
        Self { dark: theme.dark(), roles, series, background: [bg.r(), bg.g(), bg.b()] }
    }

    fn host_theme(&self) -> HostTheme {
        let appearance =
            if self.dark { HostThemeAppearance::Dark } else { HostThemeAppearance::Light };
        let mut theme = HostTheme::new()
            .with_appearance(appearance)
            .try_with_font_family("sans-serif")
            .expect("generic family");
        for (role, color) in &self.roles {
            theme = theme
                .try_with_role(ThemeRole::ALL[*role], color)
                .expect("hex color");
        }
        theme
            .try_with_series_palette(self.series.clone())
            .expect("hex colors")
    }
}

impl Inner {
    fn key(&self, source: &str) -> u64 {
        hash(&(source, self.theme_key))
    }

    fn theme(&self) -> &DiagramTheme {
        self.theme.as_ref().expect("read by a lookup")
    }

    /// The next queued render someone still looks at.
    fn next_job(&mut self) -> Option<Job> {
        while let Some(job) = self.queue.pop_front() {
            if self.used.contains(&job.key) || self.wanted.contains(&job.key) {
                return Some(job);
            }
            self.let_go(job.key);
        }
        None
    }

    /// Forgets a render nobody looks at, so a later lookup queues it again.
    fn let_go(&mut self, key: u64) {
        match self.entries.get_mut(&key) {
            Some(State::Ready { rastering, .. }) => *rastering = false,
            _ => {
                self.entries.remove(&key);
            }
        }
    }
}

impl DiagramCache {
    pub fn of(ctx: &Context) -> Self {
        let ws_seq = ws_seq(ctx);
        ctx.data_mut(|d| {
            d.get_temp_mut_or_insert_with(Id::unique("diagram_cache"), || Self {
                inner: Default::default(),
                seq: Arc::new(AtomicU64::new(0)),
                ws_seq,
                ctx: ctx.clone(),
            })
            .clone()
        })
    }

    /// Starts a frame: what isn't looked up or drawn by its end is let go,
    /// all but sizes, which layout keeps. See [`ImageCache`](super::image_cache::ImageCache).
    pub fn begin_frame(&self) {
        let mut inner = self.lock();
        if !inner.began_this_frame {
            inner.began_this_frame = true;
            inner.used.clear();
            inner.drawn.clear();
            inner.theme = None;
        }
    }

    pub fn end_frame(&self) {
        let mut inner = self.lock();
        let Inner { entries, used, drawn, .. } = &mut *inner;
        for (key, state) in entries.iter_mut() {
            let State::Ready { tree, textures, rastering: false, .. } = state else {
                continue;
            };
            if !used.contains(key) {
                *tree = None;
            }
            textures
                .retain_mut(|(w, _, fresh)| std::mem::take(fresh) || drawn.contains(&(*key, *w)));
        }
        inner.wanted = std::mem::take(&mut inner.used);
        if let Some((key, control)) = &inner.running {
            if !inner.wanted.contains(key) {
                control.cancel();
            }
        }
        inner.began_this_frame = false;
    }

    /// Bumps whenever a render lands; layout depends on it.
    pub fn seq(&self) -> u64 {
        self.seq.load(Ordering::Relaxed)
    }

    pub fn size(&self, source: &str) -> DiagramSize {
        let mut inner = self.lookup();
        let key = inner.key(source);
        inner.used.insert(key);
        if let Some(state) = inner.entries.get(&key) {
            return match state {
                State::Pending => DiagramSize::Pending,
                State::Ready { size, .. } => DiagramSize::Ready(*size),
                State::Failed(e) => DiagramSize::Failed(e.clone()),
            };
        }
        inner.entries.insert(key, State::Pending);
        self.queue(inner, key, source, None);
        if cfg!(test) { self.size(source) } else { DiagramSize::Pending }
    }

    /// The texture to paint at `width_px` device pixels wide, if any. Shows
    /// one of another width while one of this width renders.
    pub fn texture(&self, source: &str, width_px: u32) -> Option<TextureId> {
        let mut inner = self.lookup();
        let key = inner.key(source);
        inner.used.insert(key);
        let Inner { entries, drawn, .. } = &mut *inner;
        let State::Ready { tree, textures, rastering, .. } = entries.get_mut(&key)? else {
            return None;
        };
        // this width, or the nearest while it renders
        let exact = textures.iter().any(|(w, ..)| *w == width_px);
        let shown = textures
            .iter_mut()
            .min_by_key(|(w, ..)| w.abs_diff(width_px))
            .map(|(w, texture, _)| {
                drawn.insert((key, *w));
                texture.id()
            });
        if exact || *rastering {
            return shown;
        }
        *rastering = true;
        match tree.clone() {
            Some(tree) => {
                drop(inner);
                self.spawn_raster(key, tree, width_px);
            }
            None => self.queue(inner, key, source, Some(width_px)),
        }
        shown
    }

    /// The diagram on the theme background with a margin, at twice its
    /// natural size, for the clipboard; and that picture's natural size.
    pub fn image(&self, source: &str) -> Option<(ColorImage, Vec2)> {
        let (cached, theme) = {
            let inner = self.lookup();
            let cached = match inner.entries.get(&inner.key(source)) {
                Some(State::Ready { tree: Some(tree), .. }) => Some(tree.clone()),
                _ => None,
            };
            (cached, inner.theme().clone())
        };
        let tree = match cached {
            Some(tree) => tree,
            None => {
                catch_unwind(AssertUnwindSafe(|| render(source, &theme, OperationControl::new())))
                    .ok()?
                    .ok()?
                    .0
            }
        };
        const MARGIN: f32 = 16.0;
        let natural = Vec2::new(tree.size().width(), tree.size().height());
        let width = fit_width(&tree, (natural.x * 2.0) as u32, 4096);
        let margin = (MARGIN * width as f32 / natural.x).round() as u32;
        let [r, g, b] = theme.background;
        let image = rasterize(&tree, width, Some((Color32::from_rgb(r, g, b), margin))).ok()?;
        Some((image, natural + Vec2::splat(2.0 * MARGIN)))
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap()
    }

    /// The lock, for a lookup: a frame's first reads its theme.
    fn lookup(&self) -> std::sync::MutexGuard<'_, Inner> {
        let mut inner = self.lock();
        if inner.theme.is_none() {
            let theme = DiagramTheme::new(&self.ctx.get_lb_theme());
            let theme_key = hash(&theme);
            if inner.theme_key != theme_key {
                inner.theme_key = theme_key;
                // every size changes with the theme
                self.bump();
            }
            inner.theme = Some(theme);
        }
        inner
    }

    /// Sizes changed: heights that depend on them are stale.
    fn bump(&self) {
        self.seq
            .store(self.ws_seq.fetch_add(1, Ordering::Relaxed), Ordering::Relaxed);
    }

    /// Queues a render, starting the render thread if it's idle.
    fn queue(
        &self, mut inner: std::sync::MutexGuard<'_, Inner>, key: u64, source: &str,
        width_px: Option<u32>,
    ) {
        let theme = inner.theme().clone();
        inner
            .queue
            .push_back(Job { key, source: source.to_string(), theme, width_px });
        if !std::mem::replace(&mut inner.rendering, true) {
            drop(inner);
            let this = self.clone();
            run(move || {
                while let Some((job, control)) = this.next_job() {
                    this.run_job(job, control);
                }
            });
        }
    }

    /// The next render to run, or none, stopping the render thread.
    fn next_job(&self) -> Option<(Job, OperationControl)> {
        let mut inner = self.lock();
        let job = inner.next_job();
        inner.rendering = job.is_some();
        let control = OperationControl::new();
        inner.running = job.as_ref().map(|job| (job.key, control.clone()));
        job.map(|job| (job, control))
    }

    fn run_job(&self, Job { key, source, theme, width_px }: Job, control: OperationControl) {
        // a panic fails its diagram, not the render thread
        let rendered = catch_unwind(AssertUnwindSafe(|| {
            let (tree, size) = render(&source, &theme, control.clone())?;
            let texture = width_px.and_then(|w| Some((w, self.upload(key, &tree, w)?, true)));
            Ok((tree, size, texture))
        }))
        .unwrap_or_else(|_| Err("The diagram could not be drawn".into()));
        let mut inner = self.lock();
        if control.is_cancelled() {
            return inner.let_go(key);
        }
        let Some(state) = inner.entries.get_mut(&key) else { return };
        let resized = match (state, rendered) {
            // a re-render of an evicted tree: the size is known already
            (State::Ready { tree, textures, rastering, .. }, Ok((new_tree, _, texture))) => {
                *tree = Some(new_tree);
                *rastering = false;
                textures.extend(texture);
                false
            }
            (state, Ok((tree, size, texture))) => {
                let textures = texture.into_iter().collect();
                *state = State::Ready { size, tree: Some(tree), textures, rastering: false };
                true
            }
            (state, Err(e)) => {
                *state = State::Failed(e);
                true
            }
        };
        drop(inner);
        if resized {
            self.bump();
        }
        self.ctx.request_repaint();
    }

    fn spawn_raster(&self, key: u64, tree: Arc<usvg::Tree>, width_px: u32) {
        let this = self.clone();
        run(move || {
            let texture = this.upload(key, &tree, width_px);
            let mut inner = this.lock();
            if let Some(State::Ready { textures, rastering, .. }) = inner.entries.get_mut(&key) {
                *rastering = false;
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
                .load_texture(format!("diagram-{key:x}"), image, TextureOptions::LINEAR),
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

fn hash(value: &impl Hash) -> u64 {
    let mut h = DefaultHasher::new();
    value.hash(&mut h);
    h.finish()
}

/// Parsed SVG and its natural size in points.
fn render(
    source: &str, theme: &DiagramTheme, control: OperationControl,
) -> Result<(Arc<usvg::Tree>, Vec2), String> {
    let _span = tracing::trace_span!("Diagram::render", len = source.len()).entered();
    let merman_span = tracing::trace_span!("Diagram::merman").entered();
    let resolved = Presentation::new()
        .with_profile(PresentationProfile::MermanModern)
        .with_theme(theme.host_theme())
        .resolve();
    let renderer = Renderer::new().with_engine(resolved.materialize_engine(merman::Engine::new()));
    let output = SvgOutputPolicy {
        preset: SvgPipelinePreset::ResvgSafe,
        css_override_policy: CssOverridePolicy::StripExistingImportant,
        root_background_color: None,
        ..SvgOutputPolicy::default()
    };
    let request = SvgRequest {
        environment: SvgEnvironment::deterministic()
            .with_text_measurement_policy(fonts().measurement.clone()),
        pipeline: Some(output.pipeline()),
        presentation: resolved.render_policy(),
        options: merman::svg::SvgRenderOptions {
            diagram_id: Some("lb-diagram".to_string()),
            ..Default::default()
        },
        ..Default::default()
    };
    let svg = match renderer.render(RenderRequest::svg(source, control, request)) {
        Ok(RenderOutput::Svg(Some(svg))) => svg.into_parts().0,
        Ok(_) => return Err("Not a Mermaid diagram".into()),
        Err(e) => return Err(e.to_string()),
    };
    drop(merman_span);

    let fonts = fonts();
    let svg = {
        let _span = tracing::trace_span!("Diagram::outline_labels").entered();
        fonts.shaper.lock().unwrap().outline_labels(&svg, &fonts.db)
    };
    let _parse = tracing::trace_span!("Diagram::usvg_parse", len = svg.len()).entered();
    let options = usvg::Options { fontdb: fonts.db.clone(), ..Default::default() };
    let tree = usvg::Tree::from_str(&svg, &options).map_err(|e| e.to_string())?;
    let size = Vec2::new(tree.size().width(), tree.size().height());
    Ok((Arc::new(tree), size))
}

/// An attribute, by local name, set on `node` or an ancestor.
fn inherited_attribute(node: roxmltree::Node, name: &str) -> Option<String> {
    node.ancestors().filter(|n| n.is_element()).find_map(|n| {
        n.attributes()
            .find(|a| a.name() == name)
            .map(|a| a.value().to_string())
    })
}

/// A color glyph's pixels as base64 PNG.
fn color_png(image: &SwashImage) -> String {
    use base64::Engine as _;
    let p = image.placement;
    let mut png = Vec::new();
    if let Some(rgba) = image::RgbaImage::from_raw(p.width, p.height, image.data.clone()) {
        let _ = rgba.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png);
    }
    base64::prelude::BASE64_STANDARD.encode(png)
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// `width_px`, or less so neither side passes `max_side`.
fn fit_width(tree: &usvg::Tree, width_px: u32, max_side: u32) -> u32 {
    let size = tree.size();
    let tallest = max_side as f32 * size.width() / size.height();
    (width_px.min(max_side) as f32).min(tallest).max(1.0) as u32
}

/// The diagram `width_px` wide, on a `matte` of a color and a margin in
/// pixels if one is given.
fn rasterize(
    tree: &usvg::Tree, width_px: u32, matte: Option<(Color32, u32)>,
) -> Result<ColorImage, String> {
    let _span = tracing::trace_span!("Diagram::raster", width_px).entered();
    let scale = width_px as f32 / tree.size().width();
    let height_px = (tree.size().height() * scale).ceil() as u32;
    let margin = matte.map_or(0, |(_, m)| m);
    let size = [width_px + 2 * margin, height_px + 2 * margin];
    let mut pixmap = Pixmap::new(size[0], size[1]).ok_or("empty diagram")?;
    if let Some((c, _)) = matte {
        pixmap.fill(resvg::tiny_skia::Color::from_rgba8(c.r(), c.g(), c.b(), 255));
    }
    let transform =
        Transform::from_scale(scale, scale).post_translate(margin as f32, margin as f32);
    resvg::render(tree, transform, &mut pixmap.as_mut());
    Ok(ColorImage::from_rgba_premultiplied([size[0] as usize, size[1] as usize], pixmap.data()))
}

/// The app's fonts and the shaper that both measures labels and draws them,
/// so boxes fit their text: labels are the app's sans, whatever family a
/// diagram names, with the editor's fallback for what it lacks.
struct Fonts {
    db: Arc<usvg::fontdb::Database>,
    shaper: Arc<Mutex<Shaper>>,
    measurement: TextMeasurementPolicy,
}

fn fonts() -> &'static Fonts {
    static FONTS: OnceLock<Fonts> = OnceLock::new();
    FONTS.get_or_init(|| {
        let mut db = usvg::fontdb::Database::new();
        crate::font::load(&mut db);
        // labels left as text fall back to serif
        let sans = db.family_name(&usvg::fontdb::Family::SansSerif).to_string();
        db.set_serif_family(sans);
        // a clone keeps the face ids, so the shaper's picks name faces in `db`
        let fallback = crate::font::Fallback::default();
        let shaper = Arc::new(Mutex::new(Shaper {
            fonts: FontSystem::new_with_locale_and_db_and_fallback(
                "en-US".into(),
                db.clone(),
                fallback,
            ),
            ems: HashMap::new(),
            swash: SwashCache::new(),
        }));
        let measure = shaper.clone();
        let measurer = DeterministicTextMeasurer::default()
            .with_width_callback(move |text, style| measure.lock().unwrap().width(text, style));
        let id = TextMeasurementProfileIdentity::new(
            MeasurementProfileId::new("lockbook.app-fonts").expect("valid id"),
            "1".to_string(),
        )
        .expect("valid identity");
        let measurement = TextMeasurementPolicy::uniform(TextMeasurementProfile::new(id, measurer));
        Fonts { db: Arc::new(db), shaper, measurement }
    })
}

/// How [`Shaper::outline_labels`] marks labels, numbered.
const LABEL_ID: &str = "lb-label-";

/// How a label draws, as resvg reads its attributes and stylesheet, from
/// its first span.
struct LabelStyle {
    size: f32,
    weight: u16,
    italic: bool,
    /// How far along its width a line hangs from its x.
    anchor: f32,
    baseline: usvg::DominantBaseline,
    /// The CSS fill its glyphs take.
    fill: String,
}

/// Each label's style under `group`, by its mark; labels that draw nothing
/// are left out.
fn label_styles(group: &usvg::Group, styles: &mut HashMap<String, LabelStyle>) {
    for node in group.children() {
        match node {
            usvg::Node::Group(group) => label_styles(group, styles),
            usvg::Node::Text(text) => {
                let Some(chunk) = text.chunks().first() else { continue };
                let Some(span) = chunk.spans().first() else { continue };
                let Some(fill) = span.fill() else { continue };
                let usvg::Paint::Color(c) = fill.paint() else { continue };
                let mark = span
                    .font()
                    .families()
                    .iter()
                    .find_map(|family| match family {
                        usvg::FontFamily::Named(name) if name.starts_with(LABEL_ID) => Some(name),
                        _ => None,
                    });
                let Some(mark) = mark else { continue };
                styles.insert(
                    mark.clone(),
                    LabelStyle {
                        size: span.font_size().get(),
                        weight: span.font().weight(),
                        italic: span.font().style() != usvg::FontStyle::Normal,
                        anchor: match chunk.anchor() {
                            usvg::TextAnchor::Start => 0.,
                            usvg::TextAnchor::Middle => 0.5,
                            usvg::TextAnchor::End => 1.,
                        },
                        baseline: span.dominant_baseline(),
                        fill: format!(
                            "fill:#{:02x}{:02x}{:02x};fill-opacity:{}",
                            c.red,
                            c.green,
                            c.blue,
                            fill.opacity().get()
                        ),
                    },
                );
            }
            _ => {}
        }
    }
}

/// Ascent, descent (down, positive), and x-height at [`SHAPE_SIZE`].
struct SansMetrics {
    ascent: f32,
    descent: f32,
    x_height: f32,
}

/// The size text is shaped at; widths scale from it.
const SHAPE_SIZE: f32 = 100.0;

struct Shaper {
    fonts: FontSystem,
    /// Widths in ems by text, weight, italic, and points.
    ems: HashMap<(String, u16, bool, u32), f64>,
    swash: SwashCache,
}

impl Shaper {
    fn width(&mut self, text: &str, style: &TextStyle) -> f64 {
        let weight = match style.font_weight.as_deref().map(str::trim) {
            Some("bold" | "bolder") => 700,
            Some(w) => w.parse().unwrap_or(400),
            None => 400,
        };
        let italic = style
            .font_style
            .as_deref()
            .is_some_and(|s| s.contains("italic"));
        let points = style.font_size.max(1.0) as f32;
        let key = (text.to_string(), weight, italic, points.to_bits());
        let em = match self.ems.get(&key) {
            Some(em) => *em,
            None => {
                let buffer = self.shape(text, weight, italic, points);
                let width = buffer.layout_runs().map(|r| r.line_w).fold(0.0, f32::max);
                let em = width as f64 / SHAPE_SIZE as f64;
                self.ems.insert(key, em);
                em
            }
        };
        em * style.font_size.max(1.0)
    }

    /// `svg` with each label's text replaced by the outlines of the glyphs it
    /// was measured with, placed as SVG would place the text. A label laid
    /// out some way this doesn't follow stays text for resvg.
    fn outline_labels(&mut self, svg: &str, db: &Arc<usvg::fontdb::Database>) -> String {
        // mark every label so resvg's reading of the stylesheet can be had by
        // label: as a font family, since a label placed by `<use>` loses its id
        let Ok(doc) = roxmltree::Document::parse(svg) else {
            return svg.to_string();
        };
        let mut named = svg.to_string();
        let marks: Vec<(usize, String)> = doc
            .descendants()
            .filter(|n| n.has_tag_name("text"))
            .enumerate()
            .map(|(i, text)| match text.attribute_node("style") {
                Some(style) => (style.range_value().end, format!(";font-family:{LABEL_ID}{i}")),
                None => (
                    text.range().start + "<text".len(),
                    format!(" style=\"font-family:{LABEL_ID}{i}\""),
                ),
            })
            .collect();
        for (at, mark) in marks.into_iter().rev() {
            named.insert_str(at, &mark);
        }
        // any family, so a font the app lacks doesn't drop the label
        let options = usvg::Options {
            fontdb: db.clone(),
            font_resolver: usvg::FontResolver {
                select_font: Box::new(|font, db| {
                    let families = [usvg::fontdb::Family::SansSerif];
                    db.query(&usvg::fontdb::Query {
                        families: &families,
                        weight: usvg::fontdb::Weight(font.weight()),
                        style: font.style().into(),
                        ..Default::default()
                    })
                }),
                ..Default::default()
            },
            ..Default::default()
        };
        let Ok(tree) = usvg::Tree::from_str(&named, &options) else {
            return svg.to_string();
        };
        let mut styles = HashMap::new();
        label_styles(tree.root(), &mut styles);

        let Ok(doc) = roxmltree::Document::parse(&named) else {
            return svg.to_string();
        };
        let mut edits = Vec::new();
        for (i, text) in doc
            .descendants()
            .filter(|n| n.has_tag_name("text"))
            .enumerate()
        {
            let style = styles.get(&format!("{LABEL_ID}{i}"));
            if let Some(group) = style.and_then(|style| self.outline_text(text, style)) {
                edits.push((text.range(), group));
            }
        }
        let mut svg = named;
        for (range, group) in edits.into_iter().rev() {
            svg.replace_range(range, &group);
        }
        svg
    }

    /// A `<text>` as a group of paths: its lines placed by `x`, `y`, `dy`,
    /// `text-anchor`, and `dominant-baseline`, each line's glyphs one path.
    fn outline_text(&mut self, text: roxmltree::Node, style: &LabelStyle) -> Option<String> {
        let LabelStyle { size, weight, italic, anchor, baseline, ref fill } = *style;
        let preserve = inherited_attribute(text, "space").as_deref() == Some("preserve");
        let length = |value: Option<&str>| -> Option<f32> {
            let value = value?.trim();
            match value.strip_suffix("em") {
                Some(em) => em.parse::<f32>().ok().map(|em| em * size),
                None => value.trim_end_matches("px").parse().ok(),
            }
        };

        // lines: where each starts, at the baseline, and its text
        let (mut x, mut y) =
            (length(text.attribute("x")).unwrap_or(0.), length(text.attribute("y")).unwrap_or(0.));
        // the text's `dy` moves its first character, unless the tspan holding
        // that character gives its own
        let mut first_dy = length(text.attribute("dy"));
        let mut lines: Vec<(f32, f32, String)> = Vec::new();
        for child in text.children() {
            let content = if child.is_text() {
                y += first_dy.take().unwrap_or(0.);
                child.text().unwrap_or_default()
            } else if child.has_tag_name("tspan") {
                // a tspan holding only text, which starts a line where it's placed
                if child.children().any(|c| !c.is_text())
                    || child.attribute("dx").is_some()
                    || child.attribute("style").is_some()
                {
                    return None;
                }
                let placed = child.attribute("x").is_some() || child.attribute("y").is_some();
                if child.attribute("dy").is_some() && !placed && !lines.is_empty() {
                    return None;
                }
                x = length(child.attribute("x")).unwrap_or(x);
                let first_dy = first_dy.take();
                y = length(child.attribute("y")).unwrap_or(y)
                    + length(child.attribute("dy")).or(first_dy).unwrap_or(0.);
                if placed {
                    lines.push((x, y, String::new()));
                }
                child.text().unwrap_or_default()
            } else if child.is_comment() {
                continue;
            } else {
                return None;
            };
            match lines.last_mut() {
                Some(line) => line.2 += content,
                None => lines.push((x, y, content.to_string())),
            }
        }

        let sans = self.sans_metrics(weight, italic)?;
        let baseline = match baseline {
            usvg::DominantBaseline::Central => sans.ascent - (sans.ascent + sans.descent) / 2.,
            usvg::DominantBaseline::Middle => sans.x_height / 2.,
            usvg::DominantBaseline::Hanging => sans.ascent * 0.8,
            usvg::DominantBaseline::TextBeforeEdge => sans.ascent,
            usvg::DominantBaseline::TextAfterEdge | usvg::DominantBaseline::Ideographic => {
                -sans.descent
            }
            usvg::DominantBaseline::Mathematical => sans.ascent / 2.,
            _ => 0.,
        } * size
            / SHAPE_SIZE;

        let mut paths = String::new();
        for (x, y, content) in lines {
            let content = if preserve {
                content.replace(['\n', '\t'], " ")
            } else {
                content.split_whitespace().collect::<Vec<_>>().join(" ")
            };
            if content.is_empty() {
                continue;
            }
            let (d, images, width) = self.glyphs(&content, weight, italic, size);
            let scale = size / SHAPE_SIZE;
            let left = x - anchor * width * scale;
            // inline, so no stylesheet rule for the label's box reaches its glyphs
            paths += &format!(
                "<g transform=\"translate({left} {}) scale({scale})\"><path style=\"{fill};stroke:none\" d=\"{d}\"/>{images}</g>",
                y + baseline
            );
        }
        let transform = text
            .attribute("transform")
            .map(|t| format!(" transform=\"{}\"", escape(t).replace('"', "&quot;")))
            .unwrap_or_default();
        Some(format!("<g{transform}>{paths}</g>"))
    }

    /// `text`'s glyphs at [`SHAPE_SIZE`] from the baseline: their outlines as
    /// path data, color ones as images, and the line's width.
    fn glyphs(
        &mut self, text: &str, weight: u16, italic: bool, points: f32,
    ) -> (String, String, f32) {
        use std::fmt::Write as _;
        let buffer = self.shape(text, weight, italic, points);
        let (mut d, mut images, mut width) = (String::new(), String::new(), 0f32);
        for line in buffer.layout_runs() {
            width = width.max(line.line_w);
            for glyph in line.glyphs {
                let mut key = glyph.physical((0., 0.), 1.).cache_key;
                key.flags |= CacheKeyFlags::DISABLE_HINTING;
                let x = glyph.x + glyph.font_size * glyph.x_offset;
                let y = glyph.y - glyph.font_size * glyph.y_offset;
                if self.is_color(key) {
                    if let Some(image) = self.swash.get_image(&mut self.fonts, key) {
                        if image.content == SwashContent::Color {
                            let p = image.placement;
                            let png = color_png(image);
                            let _ = write!(
                                images,
                                "<image x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" href=\"data:image/png;base64,{png}\"/>",
                                x + p.left as f32,
                                y - p.top as f32,
                                p.width,
                                p.height
                            );
                            continue;
                        }
                    }
                }
                let Some(commands) = self.swash.get_outline_commands(&mut self.fonts, key) else {
                    continue;
                };
                // font outlines run up; SVG runs down
                let at = |px: f32, py: f32| (x + px, y - py);
                for command in commands {
                    let _ = match *command {
                        Command::MoveTo(p) => write!(d, "M{} {}", at(p.x, p.y).0, at(p.x, p.y).1),
                        Command::LineTo(p) => write!(d, "L{} {}", at(p.x, p.y).0, at(p.x, p.y).1),
                        Command::QuadTo(c, p) => {
                            write!(
                                d,
                                "Q{} {} {} {}",
                                at(c.x, c.y).0,
                                at(c.x, c.y).1,
                                at(p.x, p.y).0,
                                at(p.x, p.y).1
                            )
                        }
                        Command::CurveTo(c1, c2, p) => write!(
                            d,
                            "C{} {} {} {} {} {}",
                            at(c1.x, c1.y).0,
                            at(c1.x, c1.y).1,
                            at(c2.x, c2.y).0,
                            at(c2.x, c2.y).1,
                            at(p.x, p.y).0,
                            at(p.x, p.y).1
                        ),
                        Command::Close => write!(d, "Z"),
                    };
                }
            }
        }
        (d, images, width)
    }

    /// Whether `key`'s face draws in color (emoji).
    fn is_color(&mut self, key: CacheKey) -> bool {
        let Some(font) = self.fonts.get_font(key.font_id, key.font_weight) else {
            return false;
        };
        let face = font.as_swash();
        [*b"COLR", *b"sbix", *b"CBDT"]
            .into_iter()
            .any(|tag| face.table(u32::from_be_bytes(tag)).is_some())
    }

    /// The sans's metrics at [`SHAPE_SIZE`], for placing baselines.
    fn sans_metrics(&mut self, weight: u16, italic: bool) -> Option<SansMetrics> {
        let buffer = self.shape("a", weight, italic, SHAPE_SIZE);
        let glyph = buffer
            .layout_runs()
            .flat_map(|line| line.glyphs.iter())
            .next()?
            .clone();
        let font = self.fonts.get_font(glyph.font_id, glyph.font_weight)?;
        let metrics = font.as_swash().metrics(&[]);
        let scale = SHAPE_SIZE / metrics.units_per_em as f32;
        Some(SansMetrics {
            ascent: metrics.ascent * scale,
            descent: metrics.descent * scale,
            x_height: metrics.x_height * scale,
        })
    }

    /// `text` shaped at [`SHAPE_SIZE`], spaced as the editor spaces the sans
    /// at `points`.
    fn shape(&mut self, text: &str, weight: u16, italic: bool, points: f32) -> Buffer {
        use unicode_segmentation::UnicodeSegmentation as _;
        let fonts = &mut self.fonts;
        let mut buffer = Buffer::new(fonts, Metrics::new(SHAPE_SIZE, SHAPE_SIZE * 1.2));
        buffer.set_size(None, None);
        let attrs = Attrs::new()
            .family(Family::SansSerif)
            .weight(Weight(weight))
            .style(if italic { Style::Italic } else { Style::Normal });
        let spans = text
            .graphemes(true)
            .map(|g| (g, crate::font::track(attrs.clone(), g, points)));
        buffer.set_rich_text(spans, &attrs, Shaping::Advanced, None);
        buffer.shape_until_scroll(fonts, false);
        buffer
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::palette_v2::{Mode, Theme};

    /// A frame keeps what it draws; one that doesn't use a diagram lets all
    /// but its size go; drawing it again brings it back.
    #[test]
    fn frames_keep_what_they_draw() {
        let ctx = Context::default();
        ctx.set_lb_theme(Theme::default(Mode::Light));
        let cache = DiagramCache::of(&ctx);
        let source = "flowchart LR\n    A --> B";
        let frame = |f: &dyn Fn() -> Option<TextureId>| {
            cache.begin_frame();
            let drawn = f();
            cache.end_frame();
            drawn
        };
        let draw = || {
            assert!(matches!(cache.size(source), DiagramSize::Ready(_)));
            cache.texture(source, 200)
        };

        // the texture lands mid-frame, after the lookup that asked for it
        assert_eq!(frame(&draw), None);
        let texture = frame(&draw).expect("kept until drawn");
        assert_eq!(frame(&draw), Some(texture));

        // a frame without it: only the size stays
        frame(&|| None);
        let state = |c: &DiagramCache| {
            let inner = c.lock();
            match &inner.entries[&inner.key(source)] {
                State::Ready { tree, textures, .. } => (tree.is_some(), textures.len()),
                _ => panic!("size kept"),
            }
        };
        assert_eq!(state(&cache), (false, 0));

        assert_eq!(frame(&draw), None);
        assert!(frame(&draw).is_some());

        // a raster that lands after its width is no longer asked for goes
        frame(&|| cache.texture(source, 400));
        frame(&draw);
        assert_eq!(state(&cache), (true, 1));
    }
}
