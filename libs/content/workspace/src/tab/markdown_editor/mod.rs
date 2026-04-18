use std::collections::HashMap;
use std::io::{BufReader, Cursor};
use std::mem;
use std::sync::OnceLock;
use std::sync::{Arc, RwLock};
use web_time::Instant;

use crate::file_cache::FileCache;
use crate::resolvers::{EmbedResolver, LinkResolver};
use bounds::Bounds;
use colored::Colorize as _;
use comrak::nodes::AstNode;
use comrak::{Arena, Options};
use core::time::Duration;
use egui::os::OperatingSystem;
use egui::scroll_area::{ScrollAreaOutput, ScrollBarVisibility, ScrollSource};
use egui::{
    Context, EventFilter, FontData, FontDefinitions, FontFamily, FontTweak, Frame, Id, Margin,
    Pos2, Rect, ScrollArea, Sense, Stroke, Ui, UiBuilder, Vec2, scroll_area,
};
use galleys::Galleys;
use input::cursor::CursorState;
use input::mutation::EventState;
use lb_rs::Uuid;
use lb_rs::blocking::Lb;
use lb_rs::model::file_metadata::DocumentHmac;
use lb_rs::model::text::buffer::Buffer;
use lb_rs::model::text::offset_types::DocCharOffset;
use serde::{Deserialize, Serialize};
use syntect::highlighting::{Theme, ThemeSet};
use syntect::parsing::SyntaxSet;
use syntect_assets::assets::HighlightingAssets;
use widget::block::LayoutCache;
use widget::block::leaf::code_block::SyntaxHighlightCache;
use widget::emoji_completions::EmojiCompletions;
use widget::find::Find;
use widget::link_completions::LinkCompletions;
use widget::toolbar::{MOBILE_TOOL_BAR_SIZE, Toolbar};

static SYNTAX_SET: OnceLock<SyntaxSet> = OnceLock::new();
static SYNTAX_THEME: OnceLock<Theme> = OnceLock::new();

pub fn syntax_set() -> &'static SyntaxSet {
    SYNTAX_SET.get_or_init(|| {
        HighlightingAssets::from_binary()
            .get_syntax_set()
            .unwrap()
            .clone()
    })
}

pub fn syntax_theme() -> &'static Theme {
    SYNTAX_THEME.get_or_init(|| {
        let theme_bytes = include_bytes!("assets/placeholders.tmTheme").as_ref();
        let cursor = Cursor::new(theme_bytes);
        let mut buffer = BufReader::new(cursor);
        ThemeSet::load_from_reader(&mut buffer).unwrap()
    })
}

pub mod bounds;
mod galleys;
pub mod input;
pub mod output;
mod theme;
mod widget;

pub use input::Event;

use crate::TextBufferArea;
use crate::tab::markdown_editor::widget::toolbar::ToolbarPersistence;
use crate::theme::palette_v2::ThemeExt as _;
use crate::workspace::WsPersistentStore;

#[derive(Debug, Default)]
pub struct Response {
    // state changes
    pub text_updated: bool,
    pub selection_updated: bool,
    pub scroll_updated: bool,
    pub open_camera: bool,

    // Used to restrict iOS TextInteraction area
    pub find_widget_height: f32,
}

pub struct MdRender {
    // context
    pub ctx: Context,
    pub layout: MdLayout,
    pub dark_mode: bool,
    pub ext: String,
    pub touch_mode: bool,

    // document
    pub bounds: Bounds,
    pub buffer: Buffer,

    // render output
    pub galleys: Galleys,
    pub text_areas: Vec<TextBufferArea>,
    pub render_events: Vec<input::Event>,
    pub touch_consuming_rects: Vec<Rect>,

    // render input
    pub in_progress_selection: Option<(DocCharOffset, DocCharOffset)>,
    pub find_current_match: Option<(DocCharOffset, DocCharOffset)>,
    pub interactive: bool,
    pub reveal_ranges: Vec<(DocCharOffset, DocCharOffset)>,
    pub text_highlight_range: Option<(DocCharOffset, DocCharOffset)>,

    // capabilities
    pub embeds: Box<dyn EmbedResolver>,
    pub link_resolver: Box<dyn LinkResolver>,
    pub client: HttpClient,
    pub files: Arc<RwLock<FileCache>>,

    // caches
    pub layout_cache: LayoutCache,
    pub syntax: SyntaxHighlightCache,

    // viewport
    pub top_left: Pos2,
    pub width: f32,
    pub height: f32,

    // debug
    pub debug: bool,
    pub frame_times: [Instant; 10],
    pub frame_times_idx: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScrollTarget {
    Cursor,
    FindMatch,
}

/// Editing primitive — an [`MdRender`] plus the interactive state needed to
/// mutate it. Self-contained so it can be used standalone (by a composer or a
/// label+input pair) without needing an [`Editor`]'s surrounding widgets and
/// workspace integration.
pub struct MdEdit {
    pub renderer: MdRender,

    pub cursor: CursorState,
    pub event: EventState,

    /// Capability flag — whether this editor may mutate the buffer. Preview
    /// tabs set this to true; every mutation path in `input/canonical.rs`
    /// early-returns when set.
    pub readonly: bool,

    /// No physical keyboard (phone or iPad compact mode). Used by completion
    /// popups to hide Cmd/Ctrl+N shortcut hints.
    pub phone_mode: bool,

    /// Transient drag selection — `Some` while a drag is in progress; the
    /// rendered cursor/selection falls back to the buffer's own selection
    /// when `None`.
    pub in_progress_selection: Option<(DocCharOffset, DocCharOffset)>,

    /// Frame-scoped single-target scroll intent, consumed at the end of the
    /// scroll area callback.
    pub pending_scroll: Option<ScrollTarget>,

    /// Momentum from the last scroll-area frame; used by `will_consume_touch`
    /// to block touch cursor placement during momentum scroll.
    pub scroll_area_velocity: Vec2,

    /// Document identity — link completions resolve relative paths against
    /// the current file's parent.
    pub file_id: Uuid,

    /// Emoji shortcode completion popup (e.g. `:smile:`).
    pub emoji_completions: EmojiCompletions,

    /// File link / wikilink / image-link completion popup (`[[`, `[`, `![`).
    pub link_completions: LinkCompletions,
}

pub struct Editor {
    pub edit: MdEdit,

    // workspace dependencies
    pub core: Lb,
    pub persistence: WsPersistentStore,

    // document identity
    pub id_salt: Id,
    pub hmac: Option<DocumentHmac>,
    pub initialized: bool,

    embeds_last_seen: u64,

    // interaction widgets (toolbar + find are Editor-owned; completion
    // widgets moved onto MdEdit so a standalone composer inherits them)
    pub toolbar: Toolbar,
    pub find: Find,

    // misc
    pub virtual_keyboard_shown: bool,
    pub unprocessed_scroll: Option<Instant>,

    // outputs from drawing a frame need an additional frame to process before reporting
    next_resp: Response,
}

static PRINT: bool = false;

#[derive(Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct MdPersistence {
    toolbar: ToolbarPersistence,
    file: HashMap<Uuid, MdFilePersistence>,
}

impl MdPersistence {
    pub fn image_dims(&self, file_id: &Uuid) -> HashMap<String, [f32; 2]> {
        self.file
            .get(file_id)
            .map(|f| f.image_dims.clone())
            .unwrap_or_default()
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct MdFilePersistence {
    scroll_offset: f32,
    selection: (DocCharOffset, DocCharOffset),
    #[serde(default)]
    image_dims: HashMap<String, [f32; 2]>,
}

pub struct MdResources {
    pub ctx: Context,
    pub core: Lb,
    pub persistence: WsPersistentStore,
    pub files: Arc<RwLock<FileCache>>,
    pub link_resolver: Box<dyn LinkResolver>,
    pub embeds: Box<dyn EmbedResolver>,
}

pub struct MdConfig {
    pub readonly: bool,
    pub ext: String,
    pub tablet_or_desktop: bool,
}

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

impl MdRender {
    #[cfg(test)]
    pub(crate) fn test(md: &str) -> Self {
        let ctx = Context::default();
        Self {
            ctx,
            layout: MdLayout::desktop(),
            dark_mode: false,
            ext: "md".into(),
            touch_mode: false,
            bounds: Default::default(),
            buffer: md.into(),
            galleys: Default::default(),
            text_areas: Default::default(),
            render_events: Vec::new(),
            touch_consuming_rects: Default::default(),
            reveal_ranges: Vec::new(),
            text_highlight_range: None,
            in_progress_selection: None,
            find_current_match: None,
            interactive: false,
            embeds: Box::new(()),
            link_resolver: Box::new(()),
            client: Default::default(),
            files: Arc::new(RwLock::new(FileCache::empty())),
            layout_cache: Default::default(),
            syntax: Default::default(),
            top_left: Default::default(),
            width: Default::default(),
            height: Default::default(),
            debug: false,
            frame_times: [Instant::now(); 10],
            frame_times_idx: 0,
        }
    }

    pub fn plaintext_mode(&self) -> bool {
        self.ext.to_lowercase() != "md"
    }

    pub fn comrak_options() -> Options<'static> {
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
}

impl Editor {
    pub fn new(
        md: &str, file_id: Uuid, hmac: Option<DocumentHmac>, res: MdResources, cfg: MdConfig,
    ) -> Self {
        let MdResources { ctx, core, persistence, files, link_resolver, embeds } = res;
        let MdConfig { readonly, ext, tablet_or_desktop } = cfg;

        let dark_mode = ctx.style().visuals.dark_mode;
        let touch_mode = matches!(ctx.os(), OperatingSystem::Android | OperatingSystem::IOS);
        let phone_mode = touch_mode && !tablet_or_desktop;
        let layout = if touch_mode { MdLayout::mobile() } else { MdLayout::desktop() };

        let client: HttpClient = Default::default();

        let renderer = MdRender {
            ctx,
            layout,
            dark_mode,
            ext,
            touch_mode,

            bounds: Default::default(),
            buffer: md.into(),

            galleys: Default::default(),
            text_areas: Default::default(),
            render_events: Vec::new(),
            touch_consuming_rects: Default::default(),

            in_progress_selection: None,
            find_current_match: None,
            interactive: false,
            reveal_ranges: Vec::new(),
            text_highlight_range: None,

            embeds,
            link_resolver,
            client,
            files,

            layout_cache: Default::default(),
            syntax: Default::default(),

            top_left: Default::default(),
            width: Default::default(),
            height: Default::default(),

            debug: false,
            frame_times: [Instant::now(); 10],
            frame_times_idx: 0,
        };

        Self {
            edit: MdEdit {
                renderer,
                readonly,
                phone_mode,
                cursor: Default::default(),
                event: Default::default(),
                in_progress_selection: None,
                pending_scroll: None,
                scroll_area_velocity: Default::default(),
                file_id,
                emoji_completions: Default::default(),
                link_completions: Default::default(),
            },

            core,
            persistence,

            id_salt: Id::NULL,
            hmac,
            initialized: Default::default(),

            embeds_last_seen: 0,

            toolbar: Default::default(),
            find: Default::default(),

            // this is used to toggle the mobile toolbar
            virtual_keyboard_shown: cfg!(target_os = "android"),
            unprocessed_scroll: Default::default(),

            next_resp: Default::default(),
        }
    }

    #[cfg(test)]
    pub(crate) fn test(md: &str) -> Self {
        let files = Arc::new(RwLock::new(FileCache::empty()));
        let ctx = Context::default();
        let core = Lb::init(lb_rs::model::core_config::Config {
            writeable_path: format!("/tmp/{}", Uuid::new_v4()),
            logs: false,
            stdout_logs: false,
            colored_logs: false,
            background_work: false,
        })
        .unwrap();
        Self::new(
            md,
            Uuid::new_v4(),
            None,
            MdResources {
                ctx,
                core,
                persistence: WsPersistentStore::new(
                    false,
                    format!("/tmp/{}", Uuid::new_v4()).into(),
                ),
                link_resolver: Box::new(()),
                embeds: Box::new(()),
                files,
            },
            MdConfig { readonly: false, ext: String::new(), tablet_or_desktop: true },
        )
    }

    pub fn id(&self) -> Id {
        Id::new(self.edit.file_id).with(self.id_salt)
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
        self.edit.renderer.plaintext_mode()
    }

    pub fn show(&mut self, ui: &mut Ui) -> Response {
        let mut resp: Response = mem::take(&mut self.next_resp);

        let height = ui.available_size().y.round();
        let width = ui
            .max_rect()
            .width()
            .min(self.edit.renderer.layout.max_width)
            .round();
        let height_updated = self.edit.renderer.height != height;
        let width_updated = self.edit.renderer.width != width;
        self.edit.renderer.height = height;
        self.edit.renderer.width = width;

        let dark_mode = ui.style().visuals.dark_mode;
        if dark_mode != self.edit.renderer.dark_mode {
            self.edit.renderer.syntax.clear();
            self.edit.renderer.dark_mode = dark_mode;
        }

        self.edit.renderer.calc_source_lines();

        let start = web_time::Instant::now();

        let arena = Arena::new();
        let options = MdRender::comrak_options();

        let text_with_newline = self.edit.renderer.buffer.current.text.to_string() + "\n"; // todo: probably not okay but this parser quirky af sometimes
        let mut root = comrak::parse_document(&arena, &text_with_newline, &options);

        let ast_elapsed = start.elapsed();
        let start = web_time::Instant::now();

        if PRINT {
            println!(
                "{}",
                "================================================================================"
                    .bright_black()
            );
            print_ast(root);
        }

        let print_elapsed = start.elapsed();
        let start = web_time::Instant::now();

        // process events
        let prior_selection = self.edit.renderer.buffer.current.selection;
        let embeds_updated = {
            let current = self.edit.renderer.embeds.last_modified();
            let changed = current != self.embeds_last_seen;
            self.embeds_last_seen = current;
            changed
        };

        self.edit.emoji_completions.update_active_state(
            &self.edit.renderer.buffer,
            &self.edit.renderer.bounds.inline_paragraphs,
        );
        {
            let files = self.edit.renderer.files.clone();
            let file_id = self.edit.file_id;
            self.edit.link_completions.update_active_state(
                &self.edit.renderer.buffer,
                &self.edit.renderer.bounds.inline_paragraphs,
                &files,
                file_id,
            );
        }

        // Completion popups get first dibs on nav keys — they consume
        // Up/Down/Enter/Cmd+num before process_events so the editor's key
        // handling never sees them while a popup is open. Escape is observed
        // (not consumed) inside handle_input and fires regardless of editor
        // focus, so the popup can always be dismissed.
        if !self.edit.readonly {
            let focused = self.focused(ui.ctx());
            self.edit.emoji_completions.handle_input(
                ui.ctx(),
                &self.edit.renderer.buffer,
                focused,
                &mut self.edit.event.internal_events,
            );
            let files = self.edit.renderer.files.clone();
            let file_id = self.edit.file_id;
            self.edit.link_completions.handle_input(
                ui.ctx(),
                &self.edit.renderer.buffer,
                &files,
                file_id,
                focused,
                &mut self.edit.event.internal_events,
            );
        }

        let buffer_resp = self.process_events(ui.ctx(), root);
        resp.open_camera = buffer_resp.open_camera;

        if !self.initialized || buffer_resp.text_updated {
            resp.text_updated = true;

            // need to re-parse ast to compute bounds which are referenced by mobile virtual keyboard between frames
            let text_with_newline = self.edit.renderer.buffer.current.text.to_string() + "\n"; // todo: probably not okay but this parser quirky af sometimes
            root = comrak::parse_document(&arena, &text_with_newline, &options);

            self.edit.renderer.bounds.inline_paragraphs.clear();
            self.edit.renderer.layout_cache.invalidate_text_change();

            self.edit.renderer.calc_source_lines();
            self.edit.renderer.compute_bounds(root);
            self.edit.renderer.bounds.inline_paragraphs.sort();

            self.edit.renderer.calc_words();

            // recompute find matches when text changes
            if let Some(term) = self.find.term.clone() {
                self.find.matches = self.find.find_all(&self.edit.renderer.buffer, &term);
                if self.find.matches.is_empty() {
                    self.find.current_match = None;
                } else if let Some(idx) = self.find.current_match {
                    if idx >= self.find.matches.len() {
                        self.find.current_match = Some(self.find.matches.len() - 1);
                    }
                }
            }

            ui.ctx().request_repaint();
        }
        resp.selection_updated = prior_selection
            != self
                .edit
                .in_progress_selection
                .unwrap_or(self.edit.renderer.buffer.current.selection);

        // populate render inputs (find-related inputs are populated later by
        // show_find_centered, after Find::show advances state this frame —
        // otherwise galley_required_ranges/reveal_ranges would lag a frame and
        // scroll_to_find_match would have no galley to scroll to)
        self.edit.renderer.in_progress_selection = self.edit.in_progress_selection;
        self.edit.renderer.reveal_ranges.clear();
        if !self.edit.readonly && self.focused(ui.ctx()) {
            self.edit
                .renderer
                .reveal_ranges
                .push(self.edit.renderer.buffer.current.selection);
        }
        self.edit.renderer.text_highlight_range = self
            .edit
            .emoji_completions
            .search_term_range
            .or(self.edit.link_completions.search_term_range);

        ui.painter().rect_filled(
            ui.max_rect(),
            0.,
            self.edit.renderer.ctx.get_lb_theme().neutral_bg(),
        );
        self.edit.renderer.apply_theme(ui);
        ui.spacing_mut().item_spacing.x = 0.;

        let scroll_area_id = ui
            .vertical(|ui| {
                let scroll_area_id = if self.edit.renderer.touch_mode {
                    self.show_find_centered(ui);

                    // ...then show editor content (or toolbar settings)...
                    let available_width = ui.available_width();
                    let toolbar_height = if !self.edit.readonly
                        && (self.virtual_keyboard_shown || self.toolbar.menu_open)
                    {
                        MOBILE_TOOL_BAR_SIZE
                    } else {
                        0.
                    };
                    let scroll_area_id = ui
                        .allocate_ui(
                            egui::vec2(
                                ui.available_width(),
                                ui.available_height() - toolbar_height,
                            ),
                            |ui| {
                                ui.ctx().style_mut(|style| {
                                    style.spacing.scroll = egui::style::ScrollStyle::solid();
                                    style.spacing.scroll.bar_width = 10.;
                                });

                                if !self.toolbar.menu_open {
                                    // these are computed during render
                                    self.edit.renderer.galleys.galleys.clear();
                                    self.edit.renderer.bounds.wrap_lines.clear();
                                    self.edit.renderer.touch_consuming_rects.clear();

                                    // show editor
                                    let scroll_area_id =
                                        ui.id().with(egui::Id::new(self.edit.file_id));
                                    let scroll_area_offset = ui.data_mut(|d| {
                                        d.get_persisted(scroll_area_id)
                                            .map(|s: scroll_area::State| s.offset)
                                            .unwrap_or_default()
                                            .y
                                    });

                                    let scroll_area_output = self.show_scrollable_editor(ui, root);
                                    self.next_resp.scroll_updated =
                                        scroll_area_output.state.offset.y != scroll_area_offset;
                                    self.edit.scroll_area_velocity =
                                        scroll_area_output.state.velocity();

                                    Some(scroll_area_id)
                                } else {
                                    // show toolbar settings
                                    self.show_toolbar_menu(ui);

                                    None
                                }
                            },
                        )
                        .inner;

                    // ...then show toolbar at the bottom
                    if !self.edit.readonly
                        && (self.virtual_keyboard_shown || self.toolbar.menu_open)
                    {
                        let (_, rect) =
                            ui.allocate_space(egui::vec2(available_width, MOBILE_TOOL_BAR_SIZE));
                        ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
                            self.show_toolbar(root, ui);
                        });
                    }

                    scroll_area_id
                } else {
                    let scroll_area_id = ui.id().with(egui::Id::new(self.edit.file_id));
                    let scroll_area_offset = ui.data_mut(|d| {
                        d.get_persisted(scroll_area_id)
                            .map(|s: scroll_area::State| s.offset)
                            .unwrap_or_default()
                            .y
                    });

                    if !self.edit.readonly {
                        self.show_toolbar(root, ui);
                    }
                    self.show_find_centered(ui);

                    // these are computed during render
                    self.edit.renderer.galleys.galleys.clear();
                    self.edit.renderer.bounds.wrap_lines.clear();
                    self.edit.renderer.touch_consuming_rects.clear();

                    // ...then show editor content
                    let scroll_area_output = self.show_scrollable_editor(ui, root);
                    self.next_resp.scroll_updated =
                        scroll_area_output.state.offset.y != scroll_area_offset;
                    self.edit.scroll_area_velocity = scroll_area_output.state.velocity();

                    Some(scroll_area_id)
                };

                // persistence: read
                if !self.initialized {
                    let persisted = self
                        .persistence
                        .get_markdown()
                        .file
                        .get(&self.edit.file_id)
                        .cloned()
                        .unwrap_or_default();
                    if let Some(scroll_area_id) = scroll_area_id {
                        ui.data_mut(|d| {
                            let state: Option<scroll_area::State> = d.get_persisted(scroll_area_id);
                            if let Some(mut state) = state {
                                state.offset.y = persisted.scroll_offset;
                                d.insert_temp(scroll_area_id, state);
                            }
                        });
                    }
                    // set the selection using low-level API; using internal
                    // events causes touch devices to scroll to cursor on 2nd
                    // frame
                    let (start, end) = persisted.selection;
                    let selection = (
                        start.clamp(
                            0.into(),
                            self.edit
                                .renderer
                                .buffer
                                .current
                                .segs
                                .last_cursor_position(),
                        ),
                        end.clamp(
                            0.into(),
                            self.edit
                                .renderer
                                .buffer
                                .current
                                .segs
                                .last_cursor_position(),
                        ),
                    );
                    self.edit.renderer.buffer.queue(vec![
                        lb_rs::model::text::operation_types::Operation::Select(selection),
                    ]);
                    self.edit.renderer.buffer.update();
                }

                scroll_area_id
            })
            .inner;

        let text_areas = std::mem::take(&mut self.edit.renderer.text_areas);
        if !text_areas.is_empty() {
            ui.painter()
                .add(egui_wgpu_renderer::egui_wgpu::Callback::new_paint_callback(
                    ui.max_rect(),
                    crate::GlyphonRendererCallback::new(text_areas),
                ));
        }
        self.edit.show_emoji_completions(ui);
        self.edit.show_link_completions(ui);

        self.edit.renderer.syntax.garbage_collect();

        let render_elapsed = start.elapsed();

        if self.edit.renderer.debug {
            self.edit.renderer.show_debug_fps(ui);
        }

        if PRINT {
            println!(
                "{}",
                "--------------------------------------------------------------------------------"
                    .bright_black()
            );
            println!("document: {:?}", self.edit.renderer.buffer.current.text);
            println!(
                "{}",
                "--------------------------------------------------------------------------------"
                    .bright_black()
            );
            println!(
                "                                                                 ast: {ast_elapsed:?}"
            );
            println!(
                "                                                               print: {print_elapsed:?}"
            );
            println!(
                "                                                              render: {render_elapsed:?}"
            );
        }

        // post-frame bookkeeping
        let all_selected = self.edit.renderer.buffer.current.selection
            == (0.into(), self.edit.renderer.last_cursor_position());
        if embeds_updated || height_updated || width_updated {
            if embeds_updated {
                self.unprocessed_scroll = Some(Instant::now());
            }
            self.edit.renderer.layout_cache.clear();
            ui.ctx().request_repaint();
        } else if resp.selection_updated {
            let new_selection = self
                .edit
                .in_progress_selection
                .unwrap_or(self.edit.renderer.buffer.current.selection);
            self.edit
                .renderer
                .layout_cache
                .invalidate_reveal_change(prior_selection, new_selection);
            ui.ctx().request_repaint();
        }
        if self.initialized && resp.selection_updated && !all_selected {
            self.edit.pending_scroll = Some(ScrollTarget::Cursor);
            ui.ctx().request_repaint();
        }
        if self.initialized && self.edit.renderer.touch_mode && height_updated {
            self.edit.pending_scroll = Some(ScrollTarget::Cursor);
            ui.ctx().request_repaint();
        }
        if self.next_resp.scroll_updated {
            self.unprocessed_scroll = Some(Instant::now());
            ui.ctx().request_repaint();
        }
        self.edit
            .event
            .internal_events
            .append(&mut self.edit.renderer.render_events);
        if !self.edit.event.internal_events.is_empty() {
            ui.ctx().request_repaint();
        }
        // persistence: write
        let mut persistence_updated = false;
        if resp.selection_updated {
            let mut persistence = self.persistence.data.write().unwrap();
            persistence
                .markdown
                .file
                .entry(self.edit.file_id)
                .and_modify(|f| f.selection = self.edit.renderer.buffer.current.selection)
                .or_insert(MdFilePersistence {
                    scroll_offset: Default::default(),
                    selection: self.edit.renderer.buffer.current.selection,
                    image_dims: Default::default(),
                });
            persistence_updated = true;
        }

        let mut scroll_end_processed = false;
        if let Some(unprocessed_scroll) = self.unprocessed_scroll {
            if unprocessed_scroll.elapsed() > Duration::from_millis(100) {
                if let Some(scroll_area_id) = scroll_area_id {
                    let state: Option<scroll_area::State> = ui.data(|d| d.get_temp(scroll_area_id));
                    let scroll_offset = if let Some(state) = state { state.offset.y } else { 0. };

                    let image_dims = self.edit.renderer.embeds.image_dims();
                    let mut persistence = self.persistence.data.write().unwrap();
                    persistence
                        .markdown
                        .file
                        .entry(self.edit.file_id)
                        .and_modify(|f| {
                            f.scroll_offset = scroll_offset;
                            f.image_dims = image_dims.clone();
                        })
                        .or_insert(MdFilePersistence {
                            scroll_offset,
                            selection: Default::default(),
                            image_dims,
                        });
                    persistence_updated = true;

                    scroll_end_processed = true;
                }
            }
        };

        if scroll_end_processed {
            self.unprocessed_scroll = None;
        }
        if persistence_updated {
            self.persistence.write_to_file();
        }

        // focus editor when first shown or when nothing else has focus
        if !self.initialized || ui.memory(|m| m.focused().is_none()) {
            self.focus(ui.ctx());
        }
        if self.focused(ui.ctx()) {
            self.focus_lock(ui.ctx());
        }

        self.initialized = true;

        resp
    }

    pub fn will_consume_touch(&self, pos: Pos2) -> bool {
        self.edit
            .renderer
            .touch_consuming_rects
            .iter()
            .any(|rect| rect.contains(pos))
            || self.edit.scroll_area_velocity.abs().max_elem() > 0.
            || self.toolbar.menu_open
    }

    fn show_scrollable_editor<'a>(
        &mut self, ui: &mut Ui, root: &'a AstNode<'a>,
    ) -> ScrollAreaOutput<()> {
        let margin: Margin = if cfg!(target_os = "android") {
            Margin::symmetric(0, 60)
        } else {
            Margin::symmetric(0, 15)
        };
        ScrollArea::vertical()
            .scroll_source(if self.edit.renderer.touch_mode {
                ScrollSource::ALL
            } else {
                ScrollSource::SCROLL_BAR | ScrollSource::MOUSE_WHEEL
            })
            .id_salt(self.edit.file_id)
            .scroll_bar_visibility(if self.edit.renderer.touch_mode {
                ScrollBarVisibility::AlwaysVisible
            } else {
                ScrollBarVisibility::VisibleWhenNeeded
            })
            .show(ui, |ui| {
                ui.vertical_centered_justified(|ui| {
                    Frame::canvas(ui.style())
                        .inner_margin(margin)
                        .stroke(Stroke::NONE)
                        .show(ui, |ui| {
                            let scroll_view_height = ui.max_rect().height();
                            ui.allocate_space(Vec2 { x: ui.available_width(), y: 0. });

                            let padding = (ui.available_width() - self.edit.renderer.width) / 2.;

                            self.edit.renderer.top_left = ui.max_rect().min
                                + (padding + self.edit.renderer.layout.margin) * Vec2::X;
                            let height = {
                                let document_height = self.edit.renderer.height(root, &[root]);
                                let unfilled_space = if document_height < scroll_view_height {
                                    scroll_view_height - document_height
                                } else {
                                    0.
                                };
                                let end_of_text_padding = scroll_view_height / 2.;

                                document_height + unfilled_space.max(end_of_text_padding)
                            };
                            let rect = Rect::from_min_size(
                                self.edit.renderer.top_left,
                                Vec2::new(
                                    self.edit.renderer.width
                                        - 2. * self.edit.renderer.layout.margin,
                                    height,
                                ),
                            );

                            ui.ctx().check_for_id_clash(self.id(), rect, ""); // registers this widget so it's not forgotten by next frame
                            let focused = self.focused(ui.ctx());
                            let response = ui.interact(
                                rect,
                                self.id(),
                                if self.edit.renderer.touch_mode {
                                    Sense::click()
                                } else {
                                    Sense::click_and_drag()
                                },
                            );
                            if focused && !self.focused(ui.ctx()) {
                                // interact surrenders focus if we don't have sense focusable, but also if user clicks elsewhere, even on a child
                                self.focus(ui.ctx());
                            }
                            let response_properly_clicked =
                                response.clicked_by(egui::PointerButton::Primary);
                            if response.hovered() || response_properly_clicked {
                                ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Text);
                                // overridable by widgets
                            }

                            ui.advance_cursor_after_rect(rect);

                            ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
                                self.edit.renderer.show_block(
                                    ui,
                                    root,
                                    self.edit.renderer.top_left,
                                    &[root],
                                );
                            });
                        });
                });
                self.edit.renderer.galleys.galleys.sort_by_key(|g| g.range);

                // show selection
                if ui.ctx().os() != OperatingSystem::IOS {
                    let selection = self
                        .edit
                        .in_progress_selection
                        .unwrap_or(self.edit.renderer.buffer.current.selection);
                    let theme = self.edit.renderer.ctx.get_lb_theme();
                    let color = theme.bg().get_color(theme.prefs().primary);
                    self.edit.show_range(
                        ui,
                        selection,
                        color.lerp_to_gamma(theme.neutral_bg(), 0.7),
                    );
                    self.edit.show_offset(ui, selection.1, color);

                    if self.focused(ui.ctx()) {
                        if let Some([top, bot]) = self.edit.cursor_line(selection.1) {
                            let cursor_rect = egui::Rect::from_min_max(top, bot);
                            ui.output_mut(|o| {
                                o.ime = Some(egui::output::IMEOutput {
                                    rect: ui.max_rect(),
                                    cursor_rect,
                                });
                            });
                        }
                    }
                }

                // show find match highlights
                if !self.find.matches.is_empty() {
                    let theme = self.edit.renderer.ctx.get_lb_theme();
                    let highlight_color = theme.neutral_bg_tertiary();
                    let current_color = theme.fg().yellow.lerp_to_gamma(theme.neutral_bg(), 0.5);
                    for (i, &match_range) in self.find.matches.iter().enumerate() {
                        let color = if self.find.current_match == Some(i) {
                            current_color
                        } else {
                            highlight_color
                        };
                        self.edit.show_range(ui, match_range, color);
                    }
                }

                if ui.ctx().os() == OperatingSystem::Android {
                    self.edit.show_selection_handles(ui);
                }
                match self.edit.pending_scroll.take() {
                    Some(ScrollTarget::Cursor) => self.edit.scroll_to_cursor(ui),
                    Some(ScrollTarget::FindMatch) => self.scroll_to_find_match(ui),
                    None => {}
                }
            })
    }

    fn show_find_centered(&mut self, ui: &mut Ui) {
        let available = ui.available_width();
        let content_width = if self.edit.renderer.touch_mode {
            self.edit.renderer.width
        } else {
            self.toolbar_width().min(self.edit.renderer.width)
        };
        let content_left = ui.max_rect().left() + (available - content_width) / 2.;
        let top = ui.cursor().min.y;
        let find_rect =
            Rect::from_min_size(egui::pos2(content_left, top), egui::vec2(content_width, 0.));
        let prev_match = self.find.current_match_range();
        let scope_resp = ui.scope_builder(egui::UiBuilder::new().max_rect(find_rect), |ui| {
            self.find
                .show(&self.edit.renderer.buffer, self.virtual_keyboard_shown, ui)
        });
        let find_output = scope_resp.inner;
        let rendered_rect = scope_resp.response.rect;
        ui.advance_cursor_after_rect(rendered_rect);
        self.next_resp.find_widget_height = rendered_rect.height();

        self.edit.event.internal_events.extend(find_output.events);
        if find_output.scroll_to_match {
            self.edit.pending_scroll = Some(ScrollTarget::FindMatch);
        }

        // match-driven reveal-cache invalidation, mirroring the cursor-selection path
        let new_match = self.find.current_match_range();
        if prev_match != new_match {
            if let Some(old) = prev_match {
                self.edit
                    .renderer
                    .layout_cache
                    .invalidate_reveal_change(old, old);
            }
            if let Some(new) = new_match {
                self.edit
                    .renderer
                    .layout_cache
                    .invalidate_reveal_change(new, new);
            }
        }
        if find_output.closed {
            self.edit.renderer.layout_cache.clear();
        }

        // bridge find state → renderer inputs for this frame's editor render.
        // Must happen after Find::show so galley_required_ranges and
        // reveal_ranges reflect the new current_match; otherwise the match
        // galley isn't built and scroll_to_find_match has nothing to scroll to.
        self.edit.renderer.find_current_match = new_match;
        if let Some(range) = new_match {
            self.edit.renderer.reveal_ranges.push(range);
        }
    }

    fn scroll_to_find_match(&self, ui: &mut Ui) {
        if let Some(idx) = self.find.current_match {
            if let Some(match_range) = self.find.matches.get(idx) {
                let rects = self.edit.range_rects(*match_range);
                if let Some(rect) = rects.first() {
                    ui.scroll_to_rect(rect.expand(rect.height()), Some(egui::Align::Center));
                }
            }
        }
    }
}

pub fn print_ast<'a>(root: &'a AstNode<'a>) {
    print_recursive(root, "");
}

fn print_recursive<'a>(node: &'a AstNode<'a>, indent: &str) {
    let last_child = node.next_sibling().is_none();
    let sourcepos = node.data.borrow().sourcepos;

    if indent.is_empty() {
        println!(
            "{} {:?} {}{}{}",
            if node.data.borrow().value.block() { "□" } else { "☰" }.blue(),
            node.data.borrow().value,
            format!("{sourcepos}").yellow(),
            if node.children().count() > 0 {
                format!(" +{} ", node.children().count())
            } else {
                "".into()
            }
            .blue(),
            if node.children().count() > 0 {
                if !node.data.borrow().value.block() || node.data.borrow().value.contains_inlines()
                {
                    "☰"
                } else {
                    "□"
                }
            } else {
                ""
            }
            .bright_magenta(),
        );
    } else {
        println!(
            "{}{}{} {:?} {}{}{}",
            indent,
            if last_child { "└>" } else { "├>" }.bright_black(),
            if node.data.borrow().value.block() { "□" } else { "☰" }.blue(),
            node.data.borrow().value,
            format!("{sourcepos}").yellow(),
            if node.children().count() > 0 {
                format!(" +{} ", node.children().count())
            } else {
                "".into()
            }
            .blue(),
            if node.children().count() > 0 {
                if !node.data.borrow().value.block() || node.data.borrow().value.contains_inlines()
                {
                    "☰"
                } else {
                    "□"
                }
            } else {
                ""
            }
            .bright_magenta(),
        );
    }
    for child in node.children() {
        print_recursive(
            child,
            &format!("{}{}", indent, if last_child { "  " } else { "│ " }.bright_black()),
        );
    }
}

pub fn register_fonts(fonts: &mut FontDefinitions) {
    let (sans, mono, bold, base_scale) = if cfg!(target_vendor = "apple") {
        (lb_fonts::SF_PRO_TEXT_REGULAR, lb_fonts::SF_MONO_REGULAR, lb_fonts::SF_PRO_TEXT_BOLD, 0.9)
    } else {
        (
            lb_fonts::NOTO_SANS_REGULAR,
            lb_fonts::NOTO_SANS_MONO_REGULAR,
            lb_fonts::NOTO_SANS_BOLD,
            1.,
        )
    };

    let mono_scale = 0.9 * base_scale;
    let mono_y_offset_factor = 0.1;
    let mono_baseline_offset_factor = -0.1;

    let super_y_offset_factor = -1. / 4.;
    let sub_y_offset_factor = 1. / 4.;
    let super_scale = 3. / 4.;
    let sub_scale = 3. / 4.;

    fonts.font_data.insert(
        "sans".to_string(),
        FontData {
            tweak: FontTweak { scale: base_scale, ..FontTweak::default() },
            ..FontData::from_static(sans)
        }
        .into(),
    );
    fonts.font_data.insert("mono".into(), {
        FontData {
            tweak: FontTweak {
                y_offset_factor: mono_y_offset_factor,
                scale: mono_scale,
                baseline_offset_factor: mono_baseline_offset_factor,
                ..Default::default()
            },
            ..FontData::from_static(mono)
        }
        .into()
    });
    fonts.font_data.insert(
        "bold".to_string(),
        FontData {
            tweak: FontTweak { scale: base_scale, ..FontTweak::default() },
            ..FontData::from_static(bold)
        }
        .into(),
    );

    fonts.font_data.insert("sans_super".into(), {
        FontData {
            tweak: FontTweak {
                y_offset_factor: super_y_offset_factor,
                scale: super_scale * base_scale,
                ..Default::default()
            },
            ..FontData::from_static(sans)
        }
        .into()
    });
    fonts.font_data.insert("bold_super".into(), {
        FontData {
            tweak: FontTweak {
                y_offset_factor: super_y_offset_factor,
                scale: super_scale * base_scale,
                ..Default::default()
            },
            ..FontData::from_static(bold)
        }
        .into()
    });
    fonts.font_data.insert("mono_super".into(), {
        FontData {
            tweak: FontTweak {
                y_offset_factor: super_y_offset_factor + mono_y_offset_factor,
                scale: super_scale * mono_scale,
                baseline_offset_factor: mono_baseline_offset_factor,
                ..Default::default()
            },
            ..FontData::from_static(mono)
        }
        .into()
    });

    fonts.font_data.insert("sans_sub".into(), {
        FontData {
            tweak: FontTweak {
                y_offset_factor: sub_y_offset_factor,
                scale: sub_scale * base_scale,
                ..Default::default()
            },
            ..FontData::from_static(sans)
        }
        .into()
    });
    fonts.font_data.insert("bold_sub".into(), {
        FontData {
            tweak: FontTweak {
                y_offset_factor: sub_y_offset_factor,
                scale: sub_scale * base_scale,
                ..Default::default()
            },
            ..FontData::from_static(bold)
        }
        .into()
    });
    fonts.font_data.insert("mono_sub".into(), {
        FontData {
            tweak: FontTweak {
                y_offset_factor: sub_y_offset_factor + mono_y_offset_factor,
                scale: sub_scale * mono_scale,
                baseline_offset_factor: mono_baseline_offset_factor,
                ..Default::default()
            },
            ..FontData::from_static(mono)
        }
        .into()
    });

    fonts.font_data.insert("icons".into(), {
        FontData {
            tweak: FontTweak { y_offset: -0.1, scale: mono_scale, ..Default::default() },
            ..FontData::from_static(lb_fonts::NERD_FONTS_MONO_SYMBOLS)
        }
        .into()
    });

    fonts
        .families
        .insert(FontFamily::Name(Arc::from("Bold")), vec!["bold".into()]);
    fonts
        .families
        .insert(FontFamily::Name(Arc::from("SansSuper")), vec!["sans_super".into()]);
    fonts
        .families
        .insert(FontFamily::Name(Arc::from("BoldSuper")), vec!["bold_super".into()]);
    fonts
        .families
        .insert(FontFamily::Name(Arc::from("MonoSuper")), vec!["mono_super".into()]);
    fonts
        .families
        .insert(FontFamily::Name(Arc::from("SansSub")), vec!["sans_sub".into()]);
    fonts
        .families
        .insert(FontFamily::Name(Arc::from("BoldSub")), vec!["bold_sub".into()]);
    fonts
        .families
        .insert(FontFamily::Name(Arc::from("MonoSub")), vec!["mono_sub".into()]);
    fonts
        .families
        .insert(FontFamily::Name(Arc::from("Icons")), vec!["icons".into()]);

    fonts
        .families
        .get_mut(&FontFamily::Proportional)
        .unwrap()
        .insert(0, "sans".to_string());

    fonts
        .families
        .get_mut(&FontFamily::Monospace)
        .unwrap()
        .insert(0, "mono".to_string());

    fonts
        .families
        .get_mut(&FontFamily::Monospace)
        .unwrap()
        .push("icons".to_owned());
}

/// Headless editor harness matching the Android FFI surface so tests read
/// like the Kotlin call sites (`replace`, `setSelection`, `enterFrame`, …).
#[cfg(test)]
mod test {
    use super::*;
    use crate::tab::ExtendedInput as _;
    use crate::theme::palette_v2::{Mode, Theme};
    use egui::RawInput;
    use input::{Event, Location, Region};
    use lb_rs::model::text::offset_types::DocCharOffset;

    struct TestEditor {
        editor: Editor,
        pending: Vec<Event>,
    }

    impl TestEditor {
        fn new(md: &str) -> Self {
            let mut harness = Self { editor: Editor::test(md), pending: vec![] };
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
        /// Editor::show(), processing all pending events.
        fn enter_frame(&mut self) {
            let ctx = self.editor.edit.renderer.ctx.clone();
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
            &self.editor.edit.renderer.buffer.current.text
        }

        fn get_selection(&self) -> (usize, usize) {
            let sel = self.editor.edit.renderer.buffer.current.selection;
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
