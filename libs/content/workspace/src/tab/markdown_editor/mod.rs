use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use web_time::Instant;

use crate::file_cache::FileCache;
use crate::resolvers::{EmbedResolver, LinkResolver};
use bounds::Bounds;
use comrak::Options;
use comrak::nodes::AstNode;
use egui::os::OperatingSystem;
use egui::{Context, EventFilter, Id, Pos2, Rect, Vec2};
use galleys::Galleys;
use input::cursor::CursorState;
use input::mutation::EventState;
use lb_rs::Uuid;
use lb_rs::model::text::buffer::Buffer;
use lb_rs::model::text::offset_types::DocCharOffset;
use serde::{Deserialize, Serialize};
use widget::block::LayoutCache;
use widget::block::leaf::code_block::SyntaxHighlightCache;
use widget::emoji_completions::EmojiCompletions;
use widget::find::Find;
use widget::link_completions::LinkCompletions;
use widget::toolbar::Toolbar;

pub mod bounds;
mod debug_ast;
mod galleys;
pub mod input;
pub mod output;
mod show;
mod theme;
pub mod widget;

pub use input::Event;

use crate::TextBufferArea;
use crate::tab::markdown_editor::widget::toolbar::ToolbarPersistence;
use crate::workspace::WsPersistentStore;

#[derive(Clone, Debug, Default)]
pub struct Response {
    // state changes
    pub text_updated: bool,
    pub selection_updated: bool,
    pub scroll_updated: bool,
    pub open_camera: bool,

    // Used to restrict iOS TextInteraction area
    pub find_widget_height: f32,
}

#[derive(Clone)]
pub struct MdLabel<E: EmbedResolver = (), L: LinkResolver = ()> {
    // config
    pub ctx: Context,
    pub layout: MdLayout,
    pub ext: String,
    pub touch_mode: bool,
    dark_mode: bool,

    // document
    pub buffer: Buffer,
    pub bounds: Bounds,

    // capabilities
    pub embed_resolver: E,
    pub link_resolver: L,

    // frame inputs (populated by host before rendering)
    pub interactive: bool,
    pub reveal_ranges: Vec<(DocCharOffset, DocCharOffset)>,
    pub text_highlight_range: Option<(DocCharOffset, DocCharOffset)>,
    pub galley_required_ranges: Vec<(DocCharOffset, DocCharOffset)>,

    // frame outputs (consumed by host after rendering)
    pub galleys: Galleys,
    pub text_areas: Vec<TextBufferArea>,
    pub render_events: Vec<input::Event>,
    pub touch_consuming_rects: Vec<Rect>,

    // caches
    pub layout_cache: LayoutCache,
    pub syntax: SyntaxHighlightCache,

    // viewport
    top_left: Pos2,
    width: f32,
    height: f32,

    // debug
    pub debug: bool,
    pub frame_times: [Instant; 10],
    pub frame_times_idx: usize,
}

#[derive(Clone)]
pub struct MdEdit<E: EmbedResolver, L: LinkResolver> {
    pub renderer: MdLabel<E, L>,

    // dependencies
    pub persistence: WsPersistentStore,
    pub files: Arc<RwLock<FileCache>>,

    // input
    pub file_id: Uuid,
    pub id_salt: Id,
    pub readonly: bool,
    pub phone_mode: bool,
    pub initialized: bool,

    // internal systems
    pub cursor: CursorState,
    pub event: EventState,
    embed_resolver_last_processed: u64,

    // widgets
    pub toolbar: Toolbar,
    pub find: Find,
    pub emoji_completions: EmojiCompletions,
    pub link_completions: LinkCompletions,

    // selection state
    /// During drag operations, stores the selection that would be applied
    /// without actually updating the buffer selection (which would affect syntax reveal)
    pub in_progress_selection: Option<(DocCharOffset, DocCharOffset)>,

    // misc
    pub scroll_area_velocity: Vec2,
    pub virtual_keyboard_shown: bool,
    scroll_to_cursor: bool,
    scroll_to_find_match: bool,
    pub unprocessed_scroll: Option<Instant>,

    // outputs from drawing a frame need an additional frame to process before reporting
    next_resp: Response,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct MdPersistence {
    toolbar: ToolbarPersistence,
    file: HashMap<Uuid, MdFilePersistence>,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct MdFilePersistence {
    scroll_offset: f32,
    selection: (DocCharOffset, DocCharOffset),
}

#[derive(Clone)]
pub struct MdLayout {
    pub margin: f32,
    pub max_width: f32,
    pub inline_padding: f32,
    pub annotation_font_size: f32,
    pub row_height: f32,
    pub block_padding: f32,
    pub indent: f32,
    pub bullet_radius: f32,
    pub row_spacing: f32,
    pub block_spacing: f32,
    pub completion_font_size: f32,
    pub completion_line_height: f32,
    pub completion_row_height: f32,
    pub completion_corner_radius: u8,
}

impl MdLayout {
    pub fn mobile() -> Self {
        Self {
            margin: 45.0,
            max_width: 1000.0,
            inline_padding: 3.0,
            annotation_font_size: 12.0,
            row_height: 16.0,
            block_padding: 10.0,
            indent: 26.0,
            bullet_radius: 2.0,
            row_spacing: 6.0,
            block_spacing: 14.0,
            completion_font_size: 14.0,
            completion_line_height: 16.0,
            completion_row_height: 24.0,
            completion_corner_radius: 4,
        }
    }

    pub fn desktop() -> Self {
        Self {
            margin: 45.0,
            max_width: 1000.0,
            inline_padding: 3.0,
            annotation_font_size: 12.0,
            row_height: 16.0,
            block_padding: 10.0,
            indent: 26.0,
            bullet_radius: 2.0,
            row_spacing: 6.0,
            block_spacing: 12.0,
            completion_font_size: 14.0,
            completion_line_height: 16.0,
            completion_row_height: 24.0,
            completion_corner_radius: 4,
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub type HttpClient = reqwest::Client;

#[cfg(not(target_arch = "wasm32"))]
pub type HttpClient = reqwest::blocking::Client;

impl<E: EmbedResolver, L: LinkResolver> MdLabel<E, L> {
    pub fn new(ctx: Context, ext: String, embed_resolver: E, link_resolver: L) -> Self {
        let dark_mode = ctx.style().visuals.dark_mode;
        let touch_mode = matches!(ctx.os(), OperatingSystem::Android | OperatingSystem::IOS);
        let layout = if touch_mode { MdLayout::mobile() } else { MdLayout::desktop() };

        Self {
            ctx,
            layout,
            dark_mode,
            ext,
            touch_mode,
            interactive: false,
            bounds: Default::default(),
            buffer: "".into(),
            galleys: Default::default(),
            text_areas: Default::default(),
            render_events: Default::default(),
            reveal_ranges: Default::default(),
            text_highlight_range: None,
            galley_required_ranges: Default::default(),
            embed_resolver,
            link_resolver,
            layout_cache: Default::default(),
            syntax: Default::default(),
            debug: false,
            frame_times: [Instant::now(); 10],
            frame_times_idx: 0,
            touch_consuming_rects: Default::default(),
            top_left: Default::default(),
            width: Default::default(),
            height: Default::default(),
        }
    }

    pub fn plaintext_mode(&self) -> bool {
        self.ext.to_lowercase() != "md"
    }

    pub(crate) fn comrak_options() -> Options<'static> {
        let mut options = Options::default();
        options.parse.smart = true;
        options.parse.ignore_setext = true;
        options.extension.alerts = true;
        options.extension.autolink = true;
        options.extension.description_lists = false; // todo: is this a good way to power workspace-wide term definitions?
        options.extension.footnotes = false;
        options.extension.front_matter_delimiter = Some("---".to_string());
        options.extension.greentext = false;
        options.extension.header_ids = None; // intended for HTML renderers
        options.extension.highlight = true;
        options.extension.math_code = true; // rendered as code for now
        options.extension.math_dollars = true; // rendered as code for now
        options.extension.multiline_block_quotes = false; // todo
        options.extension.shortcodes = true;
        options.extension.spoiler = true;
        options.extension.strikethrough = true;
        options.extension.subscript = true;
        options.extension.superscript = true;
        options.extension.table = true;
        options.extension.tagfilter = false; // intended for HTML renderers
        options.extension.tasklist = true;
        options.extension.underline = true;
        options.extension.wikilinks_title_after_pipe = true; // matches obsidian
        options.extension.wikilinks_title_before_pipe = false; // would not match obsidian
        options.render.escaped_char_spans = true;
        options
    }

    /// Recompute document bounds after a text change. Call before render().
    pub fn prepare<'ast>(&mut self, root: &'ast AstNode<'ast>) {
        self.bounds.inline_paragraphs.clear();
        self.layout_cache.invalidate_text_change();
        self.calc_source_lines();
        self.compute_bounds(root);
        self.bounds.inline_paragraphs.sort();
        self.calc_words();
    }

    /// Parse and render a markdown string at the given position and width.
    /// Returns shaped text areas for GPU submission via GlyphonRendererCallback.
    pub fn render(
        &mut self, ui: &mut egui::Ui, top_left: egui::Pos2, md: &str, width: f32,
    ) -> Vec<TextBufferArea> {
        use lb_rs::model::text::offset_types::IntoRangeExt as _;
        use lb_rs::model::text::operation_types::Operation;

        self.dark_mode = ui.style().visuals.dark_mode;
        self.width = width;
        self.buffer = lb_rs::model::text::buffer::Buffer::from(md);
        self.buffer.queue(vec![Operation::Select(
            self.buffer.current.segs.last_cursor_position().into_range(),
        )]);
        self.buffer.update();

        let arena = comrak::Arena::new();
        let options = Self::comrak_options();
        let text_with_newline = self.buffer.current.text.to_string() + "\n";
        let root = comrak::parse_document(&arena, &text_with_newline, &options);

        self.prepare(root);

        self.galleys.galleys.clear();
        self.bounds.wrap_lines.clear();
        self.touch_consuming_rects.clear();

        let height = self.height(root, &[root]);
        let rect = egui::Rect::from_min_size(top_left, egui::Vec2::new(width, height));

        self.show_block(
            &mut ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(*ui.layout())),
            root,
            top_left,
            &[root],
        );

        self.galleys.galleys.sort_by_key(|g| g.range);

        std::mem::take(&mut self.text_areas)
    }

    /// Compute the height of a markdown string at the given width without rendering.
    pub fn label_height(&mut self, md: &str, width: f32) -> f32 {
        use lb_rs::model::text::offset_types::IntoRangeExt as _;
        use lb_rs::model::text::operation_types::Operation;

        self.width = width;
        self.buffer = lb_rs::model::text::buffer::Buffer::from(md);
        self.buffer.queue(vec![Operation::Select(
            self.buffer.current.segs.last_cursor_position().into_range(),
        )]);
        self.buffer.update();

        let arena = comrak::Arena::new();
        let options = Self::comrak_options();
        let text_with_newline = self.buffer.current.text.to_string() + "\n";
        let root = comrak::parse_document(&arena, &text_with_newline, &options);

        self.calc_source_lines();
        self.compute_bounds(root);
        self.bounds.inline_paragraphs.sort();
        self.calc_words();

        self.height(root, &[root])
    }
}

impl<E: EmbedResolver, L: LinkResolver> MdEdit<E, L> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        md: &str, file_id: Uuid, ctx: Context, persistence: WsPersistentStore,
        files: Arc<RwLock<FileCache>>, ext: String, readonly: bool, tablet_or_desktop: bool,
        embed_resolver: E, link_resolver: L,
    ) -> Self {
        let label_renderer = MdLabel::new(ctx.clone(), "md".into(), (), ());

        let mut renderer = MdLabel::new(ctx, ext, embed_resolver, link_resolver);
        renderer.buffer = md.into();
        let phone_mode = renderer.touch_mode && !tablet_or_desktop;

        Self {
            renderer,

            persistence,
            files,

            file_id,
            id_salt: Id::NULL,
            readonly,
            phone_mode,
            initialized: Default::default(),

            cursor: Default::default(),
            event: Default::default(),
            embed_resolver_last_processed: 0,

            toolbar: Toolbar::new(label_renderer),
            find: Default::default(),
            emoji_completions: Default::default(),
            link_completions: Default::default(),

            in_progress_selection: None,

            scroll_area_velocity: Default::default(),
            virtual_keyboard_shown: cfg!(target_os = "android"),
            scroll_to_cursor: Default::default(),
            scroll_to_find_match: Default::default(),
            unprocessed_scroll: Default::default(),

            next_resp: Default::default(),
        }
    }

    pub fn id(&self) -> Id {
        Id::new(self.file_id).with(self.id_salt)
    }

    pub fn focus(&self, ctx: &Context) {
        ctx.memory_mut(|m| {
            m.request_focus(self.id());
        });
    }

    pub fn focus_lock(&mut self, ctx: &Context) {
        ctx.memory_mut(|m| {
            m.set_focus_lock_filter(
                self.id(),
                EventFilter {
                    tab: true,
                    horizontal_arrows: true,
                    vertical_arrows: true,
                    escape: true,
                },
            );
        });
    }

    pub fn focused(&self, ctx: &Context) -> bool {
        ctx.memory(|m| m.has_focus(self.id()))
    }

    pub fn surrender_focus(&self, ctx: &Context) {
        ctx.memory_mut(|m| {
            m.surrender_focus(self.id());
        });
    }

    pub fn plaintext_mode(&self) -> bool {
        self.renderer.plaintext_mode()
    }

    pub(crate) fn comrak_options() -> Options<'static> {
        MdLabel::<E, L>::comrak_options()
    }
}

#[cfg(test)]
impl MdEdit<(), ()> {
    pub(crate) fn test(md: &str) -> Self {
        let files = Arc::new(RwLock::new(FileCache::empty()));
        Self::new(
            md,
            Uuid::new_v4(),
            Context::default(),
            WsPersistentStore::new(false, format!("/tmp/{}", Uuid::new_v4()).into()),
            files,
            String::new(),
            false,
            true,
            (),
            (),
        )
    }
}

/// Headless editor harness matching the Android FFI surface so tests read
/// like the Kotlin call sites (`replace`, `setSelection`, `enterFrame`, …).
#[cfg(test)]
mod test {
    use super::*;
    use crate::tab::ExtendedInput as _;
    use crate::theme::palette_v2::{Mode, Theme, ThemeExt as _};
    use egui::RawInput;
    use input::{Event, Location, Region};
    use lb_rs::model::text::offset_types::DocCharOffset;

    struct TestEditor {
        editor: MdEdit<(), ()>,
        pending: Vec<Event>,
    }

    impl TestEditor {
        fn new(md: &str) -> Self {
            let mut harness = Self { editor: MdEdit::test(md), pending: vec![] };
            harness.enter_frame();
            harness
        }

        /// Workspace.replace(start, end, text)
        fn replace(&mut self, start: usize, end: usize, text: &str) {
            self.pending.push(Event::Replace {
                region: Region::BetweenLocations {
                    start: Location::DocCharOffset(DocCharOffset(start)),
                    end: Location::DocCharOffset(DocCharOffset(end)),
                },
                text: text.to_string(),
                advance_cursor: true,
            });
        }

        /// Workspace.enterFrame() — runs a full headless egui frame through
        /// MdEdit::show(), processing all pending events.
        fn enter_frame(&mut self) {
            let ctx = self.editor.renderer.ctx.clone();
            let pending = std::mem::take(&mut self.pending);
            let _ = ctx.run(RawInput::default(), |ctx| {
                ctx.set_lb_theme(Theme::default(Mode::Dark));
                crate::register_font_system(ctx);
                for event in &pending {
                    ctx.push_markdown_event(event.clone());
                }
                egui::CentralPanel::default().show(ctx, |ui| {
                    self.editor.show(ui);
                });
            });
        }

        fn get_text(&self) -> &str {
            &self.editor.renderer.buffer.current.text
        }

        fn get_selection(&self) -> (usize, usize) {
            let sel = self.editor.renderer.buffer.current.selection;
            (sel.0.0, sel.1.0)
        }
    }

    /// Android autocorrect: the IME deletes the old word then inserts the
    /// replacement, all computed against pre-deletion offsets. The buffer's
    /// OT adjusts the stale insert position so it lands where the deletion
    /// happened.
    ///
    /// Reproduces the sequence from Android logs:
    ///   APPLY REPL 6 9          (delete "teh")
    ///   APPLY REPL 9 9 "the"    (insert at old position 9)
    ///   END FRAME
    #[test]
    fn android_autocorrect() {
        let mut ws = TestEditor::new("hello teh world");

        ws.replace(6, 9, ""); // delete "teh"    → "hello  world"
        ws.replace(9, 9, "the"); // insert at 9     → stale, OT adjusts to 6
        ws.enter_frame();

        assert_eq!(ws.get_text(), "hello the world");
        assert_eq!(ws.get_selection(), (9, 9)); // cursor after "the"
    }
}
