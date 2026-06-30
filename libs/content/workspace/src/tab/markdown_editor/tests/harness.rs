//! Headless `Editor` driver for unit and property tests. Configures the
//! editor with `ext: "md"` so the markdown render path fires, and
//! exposes a `push` + `enter_frame` step that mirrors the Android FFI
//! surface.

use crate::file_cache::FileCache;
use crate::resolvers::EmbedResolver;
use crate::resolvers::link::{LinkResolver, LinkState, ResolvedLink};
use crate::tab::ExtendedInput as _;
use crate::tab::markdown_editor::input::Event;
use crate::tab::markdown_editor::{Editor, MdConfig, MdResources};
use crate::theme::palette_v2::{Mode, Theme, ThemeExt as _};
use crate::workspace::WsPersistentStore;
use egui::{Context, Pos2, RawInput, Rect, Ui, Vec2};
use lb_rs::Uuid;
use lb_rs::blocking::Lb;
use lb_rs::model::core_config::ClientType;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};

/// `Lb::init` is heavy (network probe, on-disk store setup); property
/// tests should call this once and reuse the result via
/// [`TestEditor::with_lb`].
pub fn build_lb() -> Lb {
    Lb::init(lb_rs::model::core_config::Config {
        writeable_path: format!("/tmp/{}", Uuid::new_v4()),
        logs: false,
        stdout_logs: false,
        colored_logs: false,
        background_work: false,
        client_type: ClientType::Unknown,
    })
    .unwrap()
}

const SCREEN_SIZE: Vec2 = Vec2::new(800., 600.);

pub struct TestEditor {
    pub editor: Editor,
    pending: Vec<Event>,
}

impl TestEditor {
    pub fn new(md: &str) -> Self {
        Self::with_lb(build_lb(), md)
    }

    /// Reuses a caller-provided `Lb` so property-test loops avoid the
    /// per-iteration `Lb::init` cost.
    pub fn with_lb(lb: Lb, md: &str) -> Self {
        Self::with_embeds(lb, md, Box::new(()))
    }

    /// Build with a custom `EmbedResolver`. Used by tests that exercise
    /// image-load layout behavior.
    pub fn with_embeds(lb: Lb, md: &str, embeds: Box<dyn EmbedResolver>) -> Self {
        let mut harness = Self { editor: build_editor(lb, md, "md", embeds), pending: vec![] };
        harness.enter_frame();
        harness
    }

    /// Build with a [`TestEmbeds`] embed resolver and return a handle so
    /// the caller can complete fake loads via [`TestEmbeds::complete`].
    pub fn with_test_embeds(lb: Lb, md: &str) -> (Self, Arc<TestEmbeds>) {
        let embeds = Arc::new(TestEmbeds::default());
        let mut harness =
            Self { editor: build_editor(lb, md, "md", Box::new(embeds.clone())), pending: vec![] };
        harness.enter_frame();
        (harness, embeds)
    }

    /// Wrap a caller-built `Editor` so tests that need a custom resolver
    /// or other non-default construction can still drive it through the
    /// harness's `enter_frame` API.
    pub fn from_editor(editor: super::super::Editor) -> Self {
        let mut harness = Self { editor, pending: vec![] };
        harness.enter_frame();
        harness
    }

    pub fn push(&mut self, event: Event) {
        self.pending.push(event);
    }

    /// Runs a full headless egui frame through `Editor::show`,
    /// processing all queued markdown events. Requests focus so
    /// `reveal_ranges` tracks the selection (`handle_input` skips it
    /// when unfocused).
    pub fn enter_frame(&mut self) {
        self.enter_frame_at(SCREEN_SIZE);
    }

    /// Same as [`enter_frame`] but with a caller-provided viewport size —
    /// lets property tests vary width (and thus exercise `width_seq`).
    pub fn enter_frame_at(&mut self, size: Vec2) {
        self.enter_frame_inner(size, true);
    }

    /// Frame with no focus (`reveal_selection = None`) and a viewport
    /// tall enough to defeat scroll virtualization. Together these
    /// pin `wrap_lines` across the sweep — required by tests that
    /// move the cursor and need layout to stay fixed.
    pub fn enter_frame_unfocused(&mut self) {
        self.enter_frame_inner(Vec2::new(SCREEN_SIZE.x, 100_000.0), false);
    }

    /// Run a frame and return egui's paint output (`shapes` are in z-order) so
    /// tests can inspect what was actually drawn.
    pub fn enter_frame_output(&mut self) -> egui::FullOutput {
        self.enter_frame_inner(SCREEN_SIZE, true)
    }

    fn enter_frame_inner(&mut self, size: Vec2, focused: bool) -> egui::FullOutput {
        let ctx = self.editor.edit.renderer.ctx.clone();
        let pending = std::mem::take(&mut self.pending);
        let screen_rect = Rect::from_min_size(Pos2::ZERO, size);
        ctx.run(RawInput { screen_rect: Some(screen_rect), ..Default::default() }, |ctx| {
            ctx.set_lb_theme(Theme::default(Mode::Dark));
            crate::register_font_system(ctx);
            if focused {
                self.editor.focus(ctx);
            } else {
                self.editor.surrender_focus(ctx);
            }
            for event in &pending {
                ctx.push_markdown_event(event.clone());
            }
            egui::CentralPanel::default().show(ctx, |ui| {
                self.editor.show(ui);
            });
        })
    }

    pub fn get_text(&self) -> &str {
        &self.editor.edit.renderer.buffer.current.text
    }

    /// Inject raw egui key/text events for one frame *without* forcing editor
    /// focus, so focus stays where a widget (e.g. find) parked it.
    pub fn enter_frame_with_input(&mut self, events: Vec<egui::Event>) {
        let ctx = self.editor.edit.renderer.ctx.clone();
        let pending = std::mem::take(&mut self.pending);
        let screen_rect = Rect::from_min_size(Pos2::ZERO, SCREEN_SIZE);
        let _ = ctx.run(
            RawInput { screen_rect: Some(screen_rect), events, ..Default::default() },
            |ctx| {
                ctx.set_lb_theme(Theme::default(Mode::Dark));
                crate::register_font_system(ctx);
                for event in &pending {
                    ctx.push_markdown_event(event.clone());
                }
                egui::CentralPanel::default().show(ctx, |ui| {
                    self.editor.show(ui);
                });
            },
        );
    }

    pub fn has_focus(&self, id: egui::Id) -> bool {
        self.editor.edit.renderer.ctx.memory(|m| m.has_focus(id))
    }
}

/// A press+release pair for one key, for [`TestEditor::enter_frame_with_input`].
pub fn key_press(key: egui::Key, modifiers: egui::Modifiers) -> Vec<egui::Event> {
    vec![
        egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers },
        egui::Event::Key { key, physical_key: None, pressed: false, repeat: false, modifiers },
    ]
}

/// `EmbedResolver` for tests that exercise async-load completion. Per-URL
/// sizes default to placeholder dims; [`Self::complete`] swaps in a new
/// size and bumps `seq` — the same shape as a real worker thread
/// finishing an image decode.
#[derive(Default)]
pub struct TestEmbeds {
    sizes: Mutex<HashMap<String, Vec2>>,
    seq: AtomicU64,
}

impl TestEmbeds {
    /// Replace `url`'s size and bump `seq`. Mimics a real load completing.
    pub fn complete(&self, url: &str, size: Vec2) {
        self.sizes.lock().unwrap().insert(url.to_string(), size);
        self.seq.fetch_add(1, Ordering::Relaxed);
    }
}

impl EmbedResolver for TestEmbeds {
    fn size(&self, url: &str) -> Vec2 {
        self.sizes
            .lock()
            .unwrap()
            .get(url)
            .copied()
            .unwrap_or_else(|| Vec2::splat(200.))
    }
    fn is_loaded(&self, url: &str) -> bool {
        self.sizes.lock().unwrap().contains_key(url)
    }
    fn show(&self, _ui: &mut Ui, _url: &str, _rect: Rect, _rounding: egui::CornerRadius) {}
    fn warm(&self, _url: &str) {}
    fn seq(&self) -> u64 {
        self.seq.load(Ordering::Relaxed)
    }
}

impl EmbedResolver for Arc<TestEmbeds> {
    fn size(&self, url: &str) -> Vec2 {
        (**self).size(url)
    }
    fn is_loaded(&self, url: &str) -> bool {
        (**self).is_loaded(url)
    }
    fn show(&self, ui: &mut Ui, url: &str, rect: Rect, rounding: egui::CornerRadius) {
        (**self).show(ui, url, rect, rounding)
    }
    fn warm(&self, url: &str) {
        (**self).warm(url)
    }
    fn seq(&self) -> u64 {
        (**self).seq()
    }
}

/// `LinkResolver` for tests that exercise link-title fetches. Resolves
/// every URL to itself (External) so the renderer's `get_link_title`
/// path is reached and the layout cache's `link_titles` map gets
/// queried — gives `link_seq` an actual job in tests.
pub struct TestLinks;

impl LinkResolver for TestLinks {
    fn resolve_link(&self, url: &str) -> Option<ResolvedLink> {
        Some(ResolvedLink::External(url.to_string()))
    }
    fn resolve_wikilink(&self, _title: &str) -> Option<lb_rs::Uuid> {
        None
    }
    fn link_state(&self, _url: &str) -> LinkState {
        LinkState::Normal
    }
    fn wikilink_state(&self, _title: &str) -> LinkState {
        LinkState::Normal
    }
}

/// `ext` selects the parse path: `"md"` runs the full markdown
/// pipeline; `""` or `"txt"` renders each line as plain text.
fn build_editor(core: Lb, md: &str, ext: &str, embeds: Box<dyn EmbedResolver>) -> Editor {
    let files = Arc::new(RwLock::new(FileCache::empty()));
    let ctx = Context::default();
    Editor::new(
        md,
        Uuid::new_v4(),
        None,
        MdResources {
            ctx,
            core,
            persistence: WsPersistentStore::new(false, format!("/tmp/{}", Uuid::new_v4()).into()),
            link_resolver: Box::new(TestLinks),
            embeds,
            files,
        },
        MdConfig { readonly: false, ext: ext.to_string(), tablet_or_desktop: true },
    )
}
