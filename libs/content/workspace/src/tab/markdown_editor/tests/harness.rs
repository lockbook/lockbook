//! Headless `Editor` driver for unit and property tests. Configures the
//! editor with `ext: "md"` so the markdown render path fires, and
//! exposes a `push` + `enter_frame` step that mirrors the Android FFI
//! surface.

use crate::file_cache::FileCache;
use crate::link_preview::LinkPreviews;
use crate::resolvers::EmbedResolver;
use crate::resolvers::FileCacheLinkResolver;
use crate::resolvers::link::{LinkResolver, LinkState, ResolvedLink};
use crate::tab::ExtendedInput as _;
use crate::tab::markdown_editor::input::Event;
use crate::tab::markdown_editor::{Editor, MdConfig, MdResources};
use crate::theme::palette_v2::{Mode, Theme, ThemeExt as _};
use crate::workspace::WsPersistentStore;
use egui::{Pos2, RawInput, Rect, Ui, Vec2};
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
        client_name: "tests".into(),
        build_info: None,
        client_type: ClientType::Unknown,
    })
    .unwrap()
}

const SCREEN_SIZE: Vec2 = Vec2::new(800., 600.);

pub struct TestEditor {
    pub editor: Editor,
    pending: Vec<Event>,
    /// Text the last frame put on the clipboard.
    pub copied: Option<String>,
    /// Every URL a frame asked the platform to open.
    pub opened: Vec<String>,
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
        let mut harness = Self {
            editor: build_editor(lb, md, embeds, super::super::test_egui_ctx(), None),
            pending: vec![],
            copied: None,
            opened: vec![],
        };
        harness.enter_frame();
        harness
    }

    /// Build with a [`TestEmbeds`] embed resolver and return a handle so
    /// the caller can complete fake loads via [`TestEmbeds::complete`].
    pub fn with_test_embeds(lb: Lb, md: &str) -> (Self, Arc<TestEmbeds>) {
        let embeds = Arc::new(TestEmbeds::default());
        let mut harness = Self {
            editor: build_editor(
                lb,
                md,
                Box::new(embeds.clone()),
                super::super::test_egui_ctx(),
                None,
            ),
            pending: vec![],
            copied: None,
            opened: vec![],
        };
        harness.enter_frame();
        (harness, embeds)
    }

    /// Another tab in the same app as `other`: same context, so same shared caches.
    pub fn beside(other: &Self, md: &str) -> Self {
        let ctx = other.editor.edit.renderer.ctx.clone();
        // tabs share the workspace's settings and its previews
        let renderer = &other.editor.edit.renderer;
        let shared = (other.editor.persistence.clone(), renderer.previews.clone());
        Self::from_editor(build_editor(build_lb(), md, Box::new(()), ctx, Some(shared)))
    }

    /// Build as the note `file_id` in `files`, resolving links against them.
    pub fn in_files(files: FileCache, file_id: Uuid, md: &str) -> Self {
        let files = Arc::new(RwLock::new(files));
        let resolver = FileCacheLinkResolver::new(files.clone(), file_id).creating_notes(true);
        let ctx = super::super::test_egui_ctx();
        let (persistence, previews) = store_and_previews(&ctx);
        let editor = Editor::new(
            md,
            file_id,
            None,
            MdResources {
                ctx: ctx.clone(),
                previews,
                core: build_lb(),
                persistence,
                link_resolver: Box::new(resolver),
                embeds: Box::new(()),
                files,
            },
            MdConfig { readonly: false, ext: "md".into() },
        );
        Self::from_editor(editor)
    }

    /// Wrap a caller-built `Editor` so tests that need a custom resolver
    /// or other non-default construction can still drive it through the
    /// harness's `enter_frame` API.
    pub fn from_editor(editor: super::super::Editor) -> Self {
        let mut harness = Self { editor, pending: vec![], copied: None, opened: vec![] };
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

    fn enter_frame_inner(&mut self, size: Vec2, focused: bool) {
        let ctx = self.editor.edit.renderer.ctx.clone();
        let pending = std::mem::take(&mut self.pending);
        let screen_rect = Rect::from_min_size(Pos2::ZERO, size);
        let mut out = ctx.run_pass(
            RawInput { screen_rect: Some(screen_rect), ..Default::default() },
            |ctx| {
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
                ctx.root_ui(|ui| {
                    egui::CentralPanel::default().show(ui, |ui| {
                        self.editor.show(ui);
                    })
                });
            },
        );
        out.textures_delta.clear();
        self.record(out.platform_output);
    }

    fn record(&mut self, output: egui::PlatformOutput) {
        self.copied = None;
        for command in output.commands {
            match command {
                egui::OutputCommand::CopyText(text) => self.copied = Some(text),
                egui::OutputCommand::OpenUrl(url) => self.opened.push(url.url),
                _ => {}
            }
        }
    }

    pub fn get_text(&self) -> &str {
        &self.editor.edit.renderer.buffer.current.text
    }

    /// Inject raw egui key/text events for one frame *without* forcing editor
    /// focus, so focus stays where a widget (e.g. find) parked it.
    pub fn enter_frame_with_input(&mut self, events: Vec<egui::Event>) {
        self.enter_frame_with_input_held(events, egui::Modifiers::NONE);
    }

    /// Like [`enter_frame_with_input`] with `modifiers` held for the frame
    /// (`ctx.input().modifiers`), as hosts report for keys they translate
    /// themselves.
    pub fn enter_frame_with_input_held(
        &mut self, events: Vec<egui::Event>, modifiers: egui::Modifiers,
    ) {
        let ctx = self.editor.edit.renderer.ctx.clone();
        let pending = std::mem::take(&mut self.pending);
        let screen_rect = Rect::from_min_size(Pos2::ZERO, SCREEN_SIZE);
        let events = std::iter::once(egui::Event::ModifiersChanged(modifiers))
            .chain(events)
            .collect();
        let mut out = ctx.run_pass(
            RawInput { screen_rect: Some(screen_rect), events, ..Default::default() },
            |ctx| {
                ctx.set_lb_theme(Theme::default(Mode::Dark));
                crate::register_font_system(ctx);
                for event in &pending {
                    ctx.push_markdown_event(event.clone());
                }
                ctx.root_ui(|ui| {
                    egui::CentralPanel::default().show(ui, |ui| {
                        self.editor.show(ui);
                    })
                });
            },
        );
        out.textures_delta.clear();
        self.record(out.platform_output);
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
    /// Worst-case resolver: allocates its rect. Real resolvers are paint-only,
    /// but callers must survive an allocating one — embeds paint for
    /// off-screen neighbor rows, where an allocation drags the layout cursor
    /// off the viewport (#4892). Callers pass a non-advancing child ui.
    fn show(&self, ui: &mut Ui, _url: &str, rect: Rect, _rounding: egui::CornerRadius) {
        ui.allocate_rect(rect, egui::Sense::hover());
    }
    fn prefetch(&self, _url: &str) {}
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
    fn prefetch(&self, url: &str) {
        (**self).prefetch(url)
    }
    fn seq(&self) -> u64 {
        (**self).seq()
    }
}

/// `LinkResolver` for tests that exercise link-title fetches. Resolves
/// every URL to itself (External) so the renderer's `get_link_title`
/// path is reached and the layout cache's `link_titles` map gets
/// queried — gives the previews' seq an actual job in tests.
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

/// A fresh settings store and the link previews that read it.
pub fn store_and_previews(ctx: &egui::Context) -> (WsPersistentStore, LinkPreviews) {
    let persistence =
        WsPersistentStore::new(false, format!("/tmp/{}", Uuid::new_v4()).into(), true);
    let previews = LinkPreviews::new(ctx.clone(), Default::default(), persistence.clone());
    (persistence, previews)
}

fn build_editor(
    core: Lb, md: &str, embeds: Box<dyn EmbedResolver>, ctx: egui::Context,
    shared: Option<(WsPersistentStore, LinkPreviews)>,
) -> Editor {
    let files = Arc::new(RwLock::new(FileCache::empty()));
    let (persistence, previews) = shared.unwrap_or_else(|| store_and_previews(&ctx));
    Editor::new(
        md,
        Uuid::new_v4(),
        None,
        MdResources {
            ctx,
            previews,
            core,
            persistence,
            link_resolver: Box::new(TestLinks),
            embeds,
            files,
        },
        MdConfig { readonly: false, ext: "md".into() },
    )
}
