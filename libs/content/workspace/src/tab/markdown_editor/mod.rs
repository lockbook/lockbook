use std::collections::HashMap;
use std::io::{BufReader, Cursor};
use std::mem;
use std::sync::OnceLock;
use std::sync::{Arc, RwLock};
use web_time::Instant;

use crate::file_cache::FileCache;
use crate::link_preview::LinkPreviews;
use crate::resolvers::{EmbedResolver, LinkResolver};
use bounds::Bounds;
use colored::Colorize as _;
use comrak::nodes::AstNode;
use comrak::{Arena, Options};
use core::time::Duration;
use egui::os::OperatingSystem;
use egui::{
    Context, EventFilter, FontData, FontDefinitions, FontFamily, FontTweak, Frame, Id, Margin,
    Pos2, Rect, Stroke, Ui, UiBuilder, Vec2,
};
use input::cursor::CursorState;
use input::mutation::EventState;
use lb_rs::Uuid;
use lb_rs::blocking::Lb;
#[cfg(test)]
use lb_rs::model::core_config::ClientType;
use lb_rs::model::file_metadata::DocumentHmac;
use lb_rs::model::text::buffer::Buffer;
use lb_rs::model::text::offset_types::{Byte, Grapheme};
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
mod clipboard;
pub mod commands;
pub mod fold;
pub mod fragment;
pub mod input;
pub mod md_label;
pub mod row_scroll;
mod scroll_content;
pub mod show;
pub mod table;
pub mod text_units;
mod theme;
pub(crate) mod widget;

#[cfg(test)]
mod tests;

pub use input::Event;
pub use md_label::MdLabel;
pub use widget::toolbar::MarkdownToolbarAction;

use crate::TextBufferArea;
use crate::tab::markdown_editor::scroll_content::DocRowId;
use crate::tab::markdown_editor::widget::toolbar::ToolbarPersistence;
use crate::theme::palette_v2::ThemeExt as _;
use crate::widgets::affine_scroll::{AffineScrollArea, Align, Offset, Reveal};
use crate::workspace::WsPersistentStore;

#[derive(Debug, Default)]
pub struct Response {
    // state changes
    pub text_updated: bool,
    pub selection_updated: bool,
    pub scroll_updated: bool,
    pub open_camera: bool,

    /// Screen rect (egui points) where the native text-input overlay should
    /// live — the editor viewport minus the find widget and toolbar. Positions
    /// the iOS `MdView` and the Android `WorkspaceTextInputWrapper`.
    pub text_interaction_rect: Option<egui::Rect>,

    pub mobile_toolbar_shown: bool,
    /// A popup is painted over the caret: a platform that draws its own
    /// caret above the editor hides it.
    pub caret_covered: bool,
}

/// A region painted this frame that a platform deciding touches itself
/// must not treat as text (see [`commands`]).
#[derive(Clone, Debug)]
pub enum TouchTarget {
    /// A tap applies this event.
    Tap(input::Event),
    /// The scrollbar: a drag on its thumb.
    Scrollbar,
    /// A popup egui handles itself.
    Popup,
}

/// A desktop IME composition in the buffer: where it sits, its text, and
/// the buffer seq that placement is current for.
#[derive(Clone, Debug)]
pub struct ImeMarked {
    pub range: (Grapheme, Grapheme),
    pub text: String,
    pub seq: usize,
}

pub struct MdRender {
    // context
    pub ctx: Context,
    pub layout: MdLayout,
    pub dark_mode: bool,
    pub ext: String,
    pub touch_mode: bool,
    /// Where this account's external file links point (`…/open/<id>`);
    /// `app.lockbook.net` when unknown.
    pub open_origin: Option<String>,
    /// Set only while painting the floating drag-reorder card, so chrome
    /// that shouldn't travel with it (the fold button) can opt out.
    pub painting_drag_float: bool,
    /// The slug of a heading whose link was asked for this frame; the
    /// editor, which knows the note, puts the link on the clipboard.
    pub heading_copy: Option<String>,
    /// The heading whose link was just copied, for its check mark.
    pub heading_copied: Option<((Grapheme, Grapheme), web_time::Instant)>,
    /// The user moved the selection since the last frame was painted: a
    /// table wider than the note scrolls to show its end.
    pub caret_moved: bool,

    // document
    pub buffer: Buffer,
    pub text_seq: u64, // buffer.seq includes selection changes

    pub bounds: Bounds,
    pub bounds_seq: u64,

    // render output
    pub fragments: Vec<widget::utils::wrap_layout::Fragment>,
    pub text_areas: Vec<TextBufferArea>,
    /// Strikethroughs and underlines painted on top of text
    pub deco_lines: Vec<widget::utils::wrap_layout::DecoLine>,
    /// Edge fades of scrolled tables and code blocks `(rect, left color,
    /// right color)`, painted over the text.
    pub edge_fades: Vec<(Rect, egui::Color32, egui::Color32)>,
    /// Painted over the text this frame, as a cut chip's fade.
    pub over_text: Vec<egui::Shape>,
    /// Code blocks' copy buttons, painted over the text.
    pub copy_buttons: Vec<widget::block::leaf::code_block::CopyButton>,
    pub render_events: Vec<input::Event>,
    /// Touch targets painted this frame.
    pub touch_targets: Vec<(Rect, TouchTarget)>,
    /// A block card's Open or `⋯` menu choice this frame, for the editor to act on.
    pub card_action: std::cell::RefCell<Option<(String, widget::inline::link::LinkMenuAction)>>,
    /// Per-frame geometry index of every list item, populated during the
    /// render DFS. Cleared each frame (mirrors `fragments`); read for
    /// pointer hit-testing and drop-gap math by drag-to-reorder.
    pub block_boxes: Vec<widget::block::drag::BlockBox>,
    /// Per-frame geometry index of every table cell, cleared and read like
    /// `block_boxes`: a pointer in a cell resolves to that cell.
    pub table_cell_boxes: Vec<table::TableCellBox>,
    /// A painter slot above each header row's background, with the row's
    /// rect: the selection's highlight there would paint under it.
    pub table_header_slots: Vec<(egui::layers::ShapeIdx, Rect)>,
    /// How far each table or code block too wide for the note is scrolled.
    pub row_scrolls: HashMap<row_scroll::RowScroller, row_scroll::RowScroll>,
    /// Per-frame: where each block wider than the note shows.
    pub row_scrollers: Vec<(Rect, row_scroll::RowScroller)>,
    /// Per-frame: each line of a scrolled code block, as the rect it shows
    /// in and its source.
    pub code_line_clips: Vec<(Rect, (Grapheme, Grapheme))>,
    /// A code block whose code was copied, by its range, and until when its
    /// button says so.
    pub code_copied: Option<((Grapheme, Grapheme), f64)>,
    /// Drag action emitted by a list marker this frame, drained by
    /// `MdEdit::show` to update / commit the in-progress block drag
    /// (mirrors how `render_events` carries buffer edits out of render).
    pub block_drag_action: Option<widget::block::drag::BlockDragAction>,
    /// Per-frame, keyed by `ui.id().with(salt)`; populated by
    /// `interact_fragments`, consumed by handlers in each node type.
    pub interaction_responses: std::collections::HashMap<egui::Id, egui::Response>,
    /// Per-scope interact rects behind `interaction_responses`, whose merged
    /// rect is only a bounding box; read by `touch_consume_interaction`.
    pub interaction_rects: std::collections::HashMap<egui::Id, Vec<Rect>>,
    /// Spoilers the user has tapped to reveal. Persistent across frames;
    /// cleared by `bump_text_seq` whenever the doc text changes so a
    /// fresh edit re-hides every spoiler.
    pub revealed_spoilers: std::collections::HashSet<egui::Id>,

    // render input
    pub in_progress_selection: Option<(Grapheme, Grapheme)>,
    /// Active list-item drag, mirrored from `MdEdit` so the render pass
    /// can dim the source and draw the drop indicator.
    pub in_progress_block_drag: Option<widget::block::drag::BlockDrag>,
    pub find_current_match: Option<(Grapheme, Grapheme)>,
    /// The platform text system's marked text (dictation in progress, an
    /// IME composition), painted so provisional text reads as such.
    pub platform_marked: Option<(Grapheme, Grapheme)>,
    /// Read-only search-preview highlight: the snippet range to reveal,
    /// scroll to, and box-highlight. Independent of the find feature.
    pub preview_match: Option<(Grapheme, Grapheme)>,
    /// Where a heading being landed on starts: its line is laid out wherever
    /// it is, for the landing to measure. Set each frame from the editor's
    /// landing.
    pub landing_heading: Option<Grapheme>,
    pub interactive: bool, // enables fold buttons
    pub readonly: bool,    // disables task checkboxes and saving
    pub plaintext: bool,   // render as source text, not parsed markdown
    /// Password mode: render every source byte as `*` while the buffer keeps
    /// the real text. Only meaningful with `plaintext` — masking a parsed
    /// markdown tree isn't defined. Byte-length preserving, so
    /// cursor/selection mapping is unchanged.
    pub mask: bool,
    /// Single-line field mode: lay out on one unbounded line instead of
    /// wrapping at the shown rect's width. [`MdEdit::show`] pairs this with a
    /// cursor-following horizontal scroll (the `GlyphonTextEdit`
    /// `singleline_offset` pattern); the caller's rect clips the overflow.
    pub single_line: bool,
    pub disable_images: bool, // skip image rendering (e.g. for the mobile toolbar)
    pub contact_linked_sites: bool, // mirror of the persisted pref; gates outbound link fetching

    pub reveal_selection: Option<(Grapheme, Grapheme)>,
    /// A preview atom entered via `Event::EnterAtom` whose whole range is
    /// selected (a bare autolink card/capsule — its URL *is* its full source,
    /// so no selection endpoint can land interior). Force-reveals the source;
    /// re-resolved each frame and cleared when the selection leaves it.
    pub entered_atom: Option<(Grapheme, Grapheme)>,
    pub reveal_seq: u64, // includes selection and find matches

    pub search_range: Option<(Grapheme, Grapheme)>, // drawn in accent color for completions
    pub ime_marked: Option<ImeMarked>,              // desktop IME composition, drawn underlined

    // capabilities
    pub embeds: Box<dyn EmbedResolver>,
    pub link_resolver: Box<dyn LinkResolver>,
    pub client: HttpClient,
    pub files: Arc<RwLock<FileCache>>,
    pub ws_seq: Arc<std::sync::atomic::AtomicU64>,

    // caches
    pub layout_cache: LayoutCache,
    pub previews: LinkPreviews,
    pub syntax: SyntaxHighlightCache,

    // viewport
    pub width: f32,
    pub width_seq: u64,

    pub viewport_height: f32,

    // debug
    pub debug: bool,
    pub frame_times: [Instant; 100],
    pub frame_times_idx: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScrollTarget {
    Cursor,
    FindMatch,
    /// Read-only search-preview snippet (see [`MdRender::preview_match`]).
    Preview,
    /// The caret's line at the top of the view, where a heading link lands.
    Heading,
}

/// Editing primitive — an [`MdRender`] plus the interactive state needed to
/// mutate it. Self-contained so it can be used standalone (by a composer or a
/// label+input pair) without needing an [`Editor`]'s surrounding widgets and
/// workspace integration.
pub struct MdEdit {
    pub renderer: MdRender,

    pub cursor: CursorState,
    pub event: EventState,

    /// Touch typing through the software keyboard: the toolbar docks above it
    /// and shortcut hints hide. Set each frame from the workspace.
    pub phone_mode: bool,

    /// Transient drag selection — `Some` while a drag is in progress; the
    /// rendered cursor/selection falls back to the buffer's own selection
    /// when `None`.
    pub in_progress_selection: Option<(Grapheme, Grapheme)>,

    /// Offset of the selection handle being dragged (Android), so auto-scroll
    /// follows the moving handle rather than always the selection end. `None`
    /// outside a handle drag — scroll then falls back to the selection end.
    pub in_progress_handle: Option<Grapheme>,

    /// Finger → caret offset captured on handle grab and held constant for
    /// the drag. Recomputing it from the current line each frame oscillates
    /// the mapped caret when adjacent lines differ in height.
    pub handle_drag_touch_offset: Option<Vec2>,

    /// Android insertion teardrop. Set on tap-to-place; cleared on text change.
    pub insertion_handle_visible: bool,

    /// Active list-item drag-to-reorder — `Some` from grab until release.
    /// Mirrored onto the renderer each frame for the dim/indicator paint.
    pub in_progress_block_drag: Option<widget::block::drag::BlockDrag>,

    /// Touch long-press → reorder gesture state (touch only; desktop drags
    /// the marker handle directly). See [`MdEdit::detect_touch_reorder`].
    pub touch_reorder: widget::block::drag::TouchReorder,

    /// The platform's own recognizer drives the reorder (see [`commands`]),
    /// so [`MdEdit::detect_touch_reorder`] leaves the state alone.
    pub touch_reorder_driven: bool,

    /// A committed reorder `(drag, insert_offset)`, applied at the
    /// start of the next `handle_input` so the move lands pre-render — the
    /// new layout is then current before the platform refetches selection
    /// geometry. Set on drag release (which runs post-render).
    pub pending_block_move: Option<(widget::block::drag::BlockDrag, Grapheme)>,

    /// Keyboard input after a frame's first IME edit, replayed next frame so
    /// it acts on the edit's result (completions take keys ahead of the IME).
    pub deferred_input: Vec<egui::Event>,

    /// A composition was dropped this frame; withholding the IME output
    /// tells the platform's input method to discard it too.
    pub ime_reset: bool,

    /// The text of a composition the caret left, which the input method may
    /// still send before it lets go; ignored until it sends something else,
    /// the editor gets input of its own, or loses focus.
    pub ime_abandoned: Option<String>,

    /// Frame-scoped single-target scroll intent, consumed at the end of the
    /// scroll area callback.
    pub pending_scroll: Option<ScrollTarget>,

    /// Horizontal scroll (px) of a [`MdRender::single_line`] field — how far
    /// the one-line layout is shifted left so the cursor stays inside the
    /// shown rect. Maintained by [`MdEdit::show`]; 0 outside single-line mode.
    pub single_line_scroll: f32,

    /// Vertical scroll (px) of a multi-line field whose content outgrew its
    /// rect (the chat composer at max height). Maintained by [`MdEdit::show`];
    /// 0 while the content fits.
    pub overflow_scroll: f32,

    /// A composer-like field without a page of its own: gestures scroll `overflow_scroll`.
    pub standalone: bool,

    /// Buffer (seq, selection) and rect at the last cursor-follow of
    /// `overflow_scroll` — the follow runs when these change, leaving a
    /// wheel scroll free to move the cursor out of view.
    overflow_follow: (usize, (Grapheme, Grapheme), Rect),

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

    /// The link-like node under the last desktop right-click, captured when
    /// the click lands so the context menu's link section stays stable while
    /// the menu is open and the pointer moves.
    pub context_menu_link: Option<widget::inline::link::LinkMenuTarget>,
    /// The slug of the heading under the context menu, captured likewise.
    pub context_menu_heading: Option<String>,

    /// When "Paste as table" was last chosen: the paste it requested from
    /// the platform, which arrives as any other does, is to be a table.
    pub paste_as_table: std::cell::Cell<Option<f64>>,

    /// A touch drag that began on a block wider than the note, and once the
    /// drag has set out, whether along the rows. See [`MdEdit::pan_rows`].
    pub row_pan: Option<(row_scroll::RowScroller, Option<bool>)>,
    /// A block wider than the note that the platform's own pan recognizer
    /// is panning (see [`commands`]).
    pub platform_row_pan: Option<row_scroll::RowScroller>,

    /// Owns the per-row scroll state (offset, momentum) and renders
    /// the scrollbar. `id_salt` derived from `file_id` at construction.
    pub scroll_area: AffineScrollArea<DocRowId>,

    cursor_last_interact: Option<f64>,
    /// The user typed or moved the caret this frame.
    pub caret_moved: bool,
}

impl MdEdit {
    /// Minimal editor for standalone composer-like use. Wraps
    /// `MdRender::empty` (no-op resolvers, empty `FileCache`) and sets
    /// `readonly = false` on the renderer so the composer can mutate.
    /// Callers wire `file_id`, `renderer.files`, `renderer.link_resolver`,
    /// and `renderer.embeds` as needed.
    pub fn empty(ctx: Context) -> Self {
        let mut renderer = MdRender::empty(ctx);
        renderer.readonly = false;
        let file_id = Uuid::nil();
        Self {
            renderer,
            cursor: Default::default(),
            event: Default::default(),
            phone_mode: false,
            in_progress_selection: None,
            in_progress_handle: None,
            handle_drag_touch_offset: None,
            insertion_handle_visible: false,
            in_progress_block_drag: None,
            touch_reorder: Default::default(),
            touch_reorder_driven: false,
            pending_block_move: None,
            deferred_input: Vec::new(),
            ime_reset: false,
            ime_abandoned: None,
            pending_scroll: None,
            single_line_scroll: 0.0,
            overflow_scroll: 0.0,
            standalone: true,
            overflow_follow: (0, Default::default(), Rect::ZERO),
            scroll_area_velocity: Default::default(),
            file_id,
            emoji_completions: Default::default(),
            link_completions: Default::default(),
            context_menu_link: None,
            context_menu_heading: None,
            paste_as_table: Default::default(),
            row_pan: None,
            platform_row_pan: None,
            scroll_area: AffineScrollArea::new(file_id),
            cursor_last_interact: None,
            caret_moved: false,
        }
    }
}

pub struct Editor {
    pub edit: MdEdit,

    // workspace dependencies
    pub core: Lb,
    pub persistence: WsPersistentStore,

    // document identity
    pub id_salt: Id,
    /// Shown as a search preview, which has no outline.
    pub in_preview: bool,
    /// Version of the *displayed* buffer. Only written with the matching
    /// text via [`Self::apply_disk_version`] / [`Self::record_disk_save`].
    hmac: Option<DocumentHmac>,
    pub initialized: bool,
    /// The selection was moved between frames (a reveal); the next frame
    /// tells the platform, whose own caret would stay behind.
    selection_moved: bool,
    /// The note can have an outline, headings or not, so native chrome
    /// offers it.
    pub outline_offered: bool,
    /// The note's headings as the outline lists them, for the text at the
    /// `text_seq` beside them.
    outline_rows: Option<(u64, Vec<widget::outline::OutlineRow>)>,
    /// The notes linking here as the card lists them, for the index version
    /// beside them.
    linked: Option<(u64, Vec<widget::outline::LinkedNote>)>,
    /// Where the outline card stands this frame; None once it has faded out.
    card: Option<widget::outline::CardPlace>,
    /// Where a heading link lands, at the top of the view. Its row is laid
    /// out with the frame's rows wherever it is, so the scroll is exact.
    landing: Option<Landing>,
    /// The rows painted this frame and their tops in the canvas, visible or
    /// laid out off-screen.
    painted_rows: Vec<(scroll_content::DocRowId, f32)>,
    /// The canvas those rows were painted in.
    painted_canvas: Rect,
    /// The outline was opened while typing, so it marks the caret's section
    /// even after the keyboard steps aside for it.
    outline_from_typing: bool,
    /// The user last typed or moved the caret rather than scrolled, so the
    /// outline marks the caret's section while it is on screen.
    outline_follows_caret: bool,
    /// The heading just landed on, washed briefly so the eye finds it.
    heading_flash: Option<((Grapheme, Grapheme), web_time::Instant)>,

    // change detection
    pub unprocessed_scroll: Option<Instant>,
    prev_dimensions: Option<Vec2>,
    prev_virtual_keyboard_shown: bool,
    /// The find bar's height while open; opening and closing scroll the
    /// page by it so the content stays put on screen.
    find_bar_height: f32,
    embeds_seq: u64,
    link_seq: u64,

    // interaction widgets (toolbar + find are Editor-owned; completion
    // widgets moved onto MdEdit so a standalone composer inherits them)
    pub toolbar: Toolbar,
    pub find: Find,
    pub outline: widget::outline::Outline,

    // misc
    /// Where the phone toolbar was allocated this frame — the layout-level
    /// signal that embed painting hasn't dragged it off screen (#4892).
    pub mobile_toolbar_rect: Option<egui::Rect>,
    pub virtual_keyboard_shown: bool,
    /// Real platform IME visibility, pushed by the client (Android inset
    /// listener, iOS keyboard callbacks). Unlike `virtual_keyboard_shown`
    /// — the editor's *request* — this is whether the keyboard is actually
    /// on screen. Touch long-press uses it: keyboard down → drag-reorder,
    /// keyboard up → text selection.
    pub keyboard_visible: bool,

    // outputs from drawing a frame need an additional frame to process before reporting
    next_resp: Response,
}

static PRINT: bool = false;

#[derive(Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct MdPersistence {
    toolbar: ToolbarPersistence,
    file: HashMap<Uuid, MdFilePersistence>,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct MdFilePersistence {
    /// `(anchor_block_idx, intra_precise)` — the row index and precise
    /// pixels within that row at which the scroll offset sits.
    /// Width-independent (unlike a raw pixel offset) so reopening the
    /// doc at a different width still lands on the right block.
    #[serde(default)]
    anchor: Option<(usize, f32)>,
    selection: (Grapheme, Grapheme),
}

pub struct MdResources {
    pub ctx: Context,
    /// The workspace's link previews, shared by every tab.
    pub previews: LinkPreviews,
    pub core: Lb,
    pub persistence: WsPersistentStore,
    pub files: Arc<RwLock<FileCache>>,
    pub link_resolver: Box<dyn LinkResolver>,
    pub embeds: Box<dyn EmbedResolver>,
}

pub struct MdConfig {
    pub readonly: bool,
    pub ext: String,
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
            margin: 32.0,
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
    /// Minimal renderer with no resolvers, empty file cache, and `md`
    /// extension. Used by [`MdLabel`] and tests. Touch-mode and layout are
    /// derived from `ctx`.
    pub fn empty(ctx: Context) -> Self {
        let touch_mode = matches!(ctx.os(), OperatingSystem::Android | OperatingSystem::IOS);
        let layout = if touch_mode { MdLayout::mobile() } else { MdLayout::desktop() };
        let dark_mode = ctx.global_style().visuals.dark_mode;
        let ws_seq = crate::seq::ws_seq(&ctx);
        let previews = LinkPreviews::offline(ctx.clone());
        Self {
            ctx,
            layout,
            dark_mode,
            ext: "md".into(),
            touch_mode,
            open_origin: None,
            painting_drag_float: false,
            heading_copy: None,
            heading_copied: None,
            caret_moved: false,
            buffer: "".into(),
            bounds: Default::default(),
            bounds_seq: 0,
            fragments: Vec::new(),
            block_boxes: Vec::new(),
            table_cell_boxes: Vec::new(),
            table_header_slots: Vec::new(),
            row_scrolls: Default::default(),
            row_scrollers: Default::default(),
            code_line_clips: Default::default(),
            code_copied: None,
            block_drag_action: None,
            text_areas: Default::default(),
            deco_lines: Default::default(),
            edge_fades: Default::default(),
            over_text: Default::default(),
            copy_buttons: Default::default(),
            render_events: Vec::new(),
            touch_targets: Default::default(),
            card_action: Default::default(),
            interaction_responses: Default::default(),
            interaction_rects: Default::default(),
            revealed_spoilers: Default::default(),
            in_progress_selection: None,
            in_progress_block_drag: None,
            find_current_match: None,
            platform_marked: None,
            preview_match: None,
            landing_heading: None,
            interactive: false,
            readonly: true,
            plaintext: false,
            mask: false,
            single_line: false,
            disable_images: false,
            contact_linked_sites: false,
            reveal_selection: None,
            entered_atom: None,
            reveal_seq: 0,
            text_seq: 0,
            search_range: None,
            ime_marked: None,
            embeds: Box::new(()),
            link_resolver: Box::new(()),
            client: Default::default(),
            files: Arc::new(RwLock::new(FileCache::empty())),
            ws_seq,
            layout_cache: Default::default(),
            previews,
            syntax: Default::default(),
            width: Default::default(),
            width_seq: 0,
            viewport_height: Default::default(),
            debug: false,
            frame_times: [Instant::now(); 100],
            frame_times_idx: 0,
        }
    }

    /// Setter for `width` that bumps `width_seq` only when the value
    /// actually changes — frame-scoped writers reassign every frame even
    /// when the layout hasn't moved, and an unconditional bump would
    /// invalidate every cached height.
    pub fn set_width(&mut self, width: f32) {
        if self.width.to_bits() != width.to_bits() {
            self.width = width;
            self.width_seq = self
                .ws_seq
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
    }

    /// Test setter for the viewport height — used by property tests
    /// that want to drive layout at a known viewport size without
    /// running a full egui frame.
    pub fn set_viewport_height(&mut self, viewport_height: f32) {
        // Images fit the viewport, so its height is a layout dimension as
        // the width is: a change invalidates the cached heights.
        if self.viewport_height.to_bits() != viewport_height.to_bits() {
            self.viewport_height = viewport_height;
            self.width_seq = self
                .ws_seq
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
    }

    #[cfg(test)]
    pub(crate) fn test(md: &str) -> Self {
        let ctx = test_egui_ctx();
        let ws_seq = crate::seq::ws_seq(&ctx);
        let previews = LinkPreviews::offline(ctx.clone());
        Self {
            ctx,
            layout: MdLayout::desktop(),
            dark_mode: false,
            ext: "md".into(),
            touch_mode: false,
            open_origin: None,
            painting_drag_float: false,
            heading_copy: None,
            heading_copied: None,
            caret_moved: false,
            bounds: Default::default(),
            buffer: md.into(),
            fragments: Vec::new(),
            block_boxes: Vec::new(),
            table_cell_boxes: Vec::new(),
            table_header_slots: Vec::new(),
            row_scrolls: Default::default(),
            row_scrollers: Default::default(),
            code_line_clips: Default::default(),
            code_copied: None,
            block_drag_action: None,
            text_areas: Default::default(),
            deco_lines: Default::default(),
            edge_fades: Default::default(),
            over_text: Default::default(),
            copy_buttons: Default::default(),
            render_events: Vec::new(),
            touch_targets: Default::default(),
            card_action: Default::default(),
            interaction_responses: Default::default(),
            interaction_rects: Default::default(),
            revealed_spoilers: Default::default(),
            reveal_selection: None,
            entered_atom: None,
            search_range: None,
            ime_marked: None,
            disable_images: false,
            contact_linked_sites: false,
            in_progress_selection: None,
            in_progress_block_drag: None,
            find_current_match: None,
            platform_marked: None,
            preview_match: None,
            landing_heading: None,
            interactive: false,
            readonly: true,
            plaintext: false,
            mask: false,
            single_line: false,
            embeds: Box::new(()),
            link_resolver: Box::new(()),
            client: Default::default(),
            files: Arc::new(RwLock::new(FileCache::empty())),
            layout_cache: Default::default(),
            previews,
            syntax: Default::default(),
            width: Default::default(),
            width_seq: 0,
            viewport_height: Default::default(),
            reveal_seq: 0,
            text_seq: 0,
            bounds_seq: 0,
            ws_seq,
            debug: false,
            frame_times: [Instant::now(); 100],
            frame_times_idx: 0,
        }
    }

    /// Bump `text_seq`. Call after any buffer mutation that may change
    /// text — `LayoutCache::ensure_text_consistent`, `bounds.text_seq`,
    /// and find-match invalidation all key off this counter.
    pub fn bump_text_seq(&mut self) {
        self.text_seq = self
            .ws_seq
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.revealed_spoilers.clear();
    }

    /// Re-parse the buffer into a fresh AST and rebuild all text-derived
    /// bounds (source lines, words, inline paragraphs). The caller owns the
    /// [`Arena`] — comrak's AST is arena-allocated. Callers who need heights
    /// also invoke [`MdRender::height`] on the returned root.
    pub fn reparse<'a>(&mut self, arena: &'a Arena<'a>) -> &'a AstNode<'a> {
        // Width / link / embed / reveal invalidation flows through
        // dep-stamp mismatch in the height + spacing caches. Text changes
        // can affect any entry (fold-tag-driven cascades, sourcepos shifts),
        // so the layout cache wipes wholesale here before any reads.
        // `reveal_seq` is bumped at its writers (selection in
        // `MdEdit::handle_input`, find-match in `Editor::show`).
        self.layout_cache.ensure_text_consistent(self.text_seq);

        let options = Self::comrak_options();
        let text_with_newline = format!("{}\n", self.buffer.current.text);
        let root = comrak::parse_document(arena, &text_with_newline, &options);

        // Bounds depend only on text. Skip rebuild + bump bounds_seq
        // when text hasn't changed.
        if self.bounds.text_seq != self.text_seq {
            self.bounds.inline_paragraphs.clear();
            self.calc_source_lines();
            self.calc_fold_bounds(root);
            self.calc_atom_bounds(root);
            self.calc_table_bounds(root);
            self.calc_code_block_bounds(root);
            // Populate before compute_bounds: pre_spacing_lines (called
            // from compute_bounds_block_pre_spacing) queries
            // hidden_by_fold, which now expects every node already
            // cached.
            self.populate_hidden_by_fold(root);
            self.compute_bounds(root);
            self.bounds.inline_paragraphs.sort();
            self.calc_words();
            self.calc_syntax_words(root);
            self.bounds.text_seq = self.text_seq;
            self.bounds_seq = self
                .ws_seq
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        } else {
            // Still call populate — its own deps check skips when
            // text+reveal haven't moved, but this catches reveal-only
            // changes (text seq same, reveal_seq advanced).
            self.populate_hidden_by_fold(root);
        }

        root
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
        options.extension.header_id_prefix = None; // intended for HTML renderers
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
        let MdResources { ctx, previews, core, persistence, files, link_resolver, embeds } = res;
        let MdConfig { readonly, ext } = cfg;
        let plaintext = ext.to_lowercase() != "md";

        let dark_mode = ctx.global_style().visuals.dark_mode;
        let touch_mode = matches!(ctx.os(), OperatingSystem::Android | OperatingSystem::IOS);
        let layout = if touch_mode { MdLayout::mobile() } else { MdLayout::desktop() };

        let client: HttpClient = Default::default();
        let ws_seq = crate::seq::ws_seq(&ctx);
        // the external URL's origin, as lb-rs writes it for this account
        let open_origin = core.get_file_link_url(file_id).ok().and_then(|url| {
            url.strip_suffix(&format!("/open/{file_id}"))
                .map(str::to_string)
        });

        let renderer = MdRender {
            ctx,
            layout,
            dark_mode,
            ext,
            touch_mode,
            open_origin: open_origin.clone(),
            painting_drag_float: false,
            heading_copy: None,
            heading_copied: None,
            caret_moved: false,

            bounds: Default::default(),
            buffer: md.into(),

            fragments: Vec::new(),
            block_boxes: Vec::new(),
            table_cell_boxes: Vec::new(),
            table_header_slots: Vec::new(),
            row_scrolls: Default::default(),
            row_scrollers: Default::default(),
            code_line_clips: Default::default(),
            code_copied: None,
            block_drag_action: None,
            text_areas: Default::default(),
            deco_lines: Default::default(),
            edge_fades: Default::default(),
            over_text: Default::default(),
            copy_buttons: Default::default(),
            render_events: Vec::new(),
            touch_targets: Default::default(),
            card_action: Default::default(),
            interaction_responses: Default::default(),
            interaction_rects: Default::default(),
            revealed_spoilers: Default::default(),

            in_progress_selection: None,
            in_progress_block_drag: None,
            find_current_match: None,
            platform_marked: None,
            preview_match: None,
            landing_heading: None,
            interactive: true,
            readonly,
            plaintext,
            mask: false,
            single_line: false,
            reveal_selection: None,
            entered_atom: None,
            search_range: None,
            ime_marked: None,
            disable_images: false,
            contact_linked_sites: false,

            embeds,
            link_resolver,
            client,
            files,

            layout_cache: Default::default(),
            previews,
            syntax: Default::default(),

            width: Default::default(),
            width_seq: 0,
            viewport_height: Default::default(),
            reveal_seq: 0,
            text_seq: 0,
            bounds_seq: 0,
            ws_seq,

            debug: false,
            frame_times: [Instant::now(); 100],
            frame_times_idx: 0,
        };

        Self {
            edit: MdEdit {
                renderer,
                phone_mode: touch_mode,
                cursor: Default::default(),
                event: Default::default(),
                in_progress_selection: None,
                in_progress_handle: None,
                handle_drag_touch_offset: None,
                insertion_handle_visible: false,
                in_progress_block_drag: None,
                touch_reorder: Default::default(),
                touch_reorder_driven: false,
                pending_block_move: None,
                deferred_input: Vec::new(),
                ime_reset: false,
                ime_abandoned: None,
                pending_scroll: None,
                single_line_scroll: 0.0,
                overflow_scroll: 0.0,
                standalone: false,
                overflow_follow: (0, Default::default(), Rect::ZERO),
                scroll_area_velocity: Default::default(),
                file_id,
                emoji_completions: Default::default(),
                link_completions: LinkCompletions::new(open_origin),
                context_menu_link: None,
                context_menu_heading: None,
                paste_as_table: Default::default(),
                row_pan: None,
                platform_row_pan: None,
                scroll_area: AffineScrollArea::new(file_id),
                cursor_last_interact: None,
                caret_moved: false,
            },

            core,
            persistence,

            id_salt: Id::NULL,
            in_preview: false,
            hmac,
            initialized: Default::default(),
            selection_moved: false,
            outline_offered: false,
            outline_rows: None,
            linked: None,
            card: None,
            landing: None,
            painted_rows: Vec::new(),
            painted_canvas: Rect::NOTHING,
            outline_from_typing: false,
            outline_follows_caret: false,
            heading_flash: None,
            embeds_seq: 0,
            link_seq: 0,

            toolbar: Default::default(),
            outline: Default::default(),
            find: Default::default(),

            // this is used to toggle the mobile toolbar
            mobile_toolbar_rect: None,
            virtual_keyboard_shown: false,
            keyboard_visible: false,
            unprocessed_scroll: Default::default(),

            prev_dimensions: None,
            prev_virtual_keyboard_shown: false,
            find_bar_height: 0.0,

            next_resp: Default::default(),
        }
    }

    #[cfg(test)]
    pub(crate) fn test(md: &str) -> Self {
        let files = Arc::new(RwLock::new(FileCache::empty()));
        let ctx = test_egui_ctx();
        let core = Lb::init(lb_rs::model::core_config::Config {
            writeable_path: format!("/tmp/{}", Uuid::new_v4()),
            logs: false,
            stdout_logs: false,
            colored_logs: false,
            background_work: false,
            client_name: "tests".into(),
            build_info: None,
            client_type: ClientType::Unknown,
        })
        .unwrap();
        Self::new(
            md,
            Uuid::new_v4(),
            None,
            MdResources {
                previews: LinkPreviews::offline(ctx.clone()),
                ctx,
                core,
                persistence: WsPersistentStore::new(
                    false,
                    format!("/tmp/{}", Uuid::new_v4()).into(),
                    true,
                ),
                link_resolver: Box::new(()),
                embeds: Box::new(()),
                files,
            },
            MdConfig { readonly: false, ext: String::new() },
        )
    }

    pub fn id(&self) -> Id {
        Id::unique(self.edit.file_id).with(self.id_salt)
    }

    /// Last disk hmac adopted with the displayed buffer.
    pub fn hmac(&self) -> Option<DocumentHmac> {
        self.hmac
    }

    fn apply_pending_buffer(&mut self) {
        let resp = self.edit.renderer.buffer.update();
        if resp.text_updated {
            self.edit.renderer.bump_text_seq();
        }
    }

    /// Merge `text` into the live buffer (applies queued OT), then store
    /// `hmac` as the version of that displayed text. The two are always set
    /// together so a later save cannot clone a stale body under a new hmac.
    pub fn apply_disk_version(&mut self, text: String, hmac: Option<DocumentHmac>) {
        self.edit.renderer.buffer.reload(text);
        self.apply_pending_buffer();
        self.hmac = hmac;
    }

    /// Apply queued buffer ops, then return hmac + text as one snapshot
    /// for CAS write. Side effect: mutates the buffer if ops were pending.
    pub fn flush_for_save(&mut self) -> (Option<DocumentHmac>, String) {
        self.apply_pending_buffer();
        (self.hmac, self.edit.renderer.buffer.current.text.clone())
    }

    /// Record a successful CAS: this hmac is the on-disk version of `content`.
    pub fn record_disk_save(&mut self, hmac: DocumentHmac, seq: usize, content: String) {
        self.edit.renderer.buffer.saved(seq, content);
        self.hmac = Some(hmac);
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
        self.edit.renderer.plaintext
    }

    pub fn show(&mut self, ui: &mut Ui) -> Response {
        let mut resp: Response = mem::take(&mut self.next_resp);

        let height = ui.available_size().y.round();
        let width = ui
            .max_rect()
            .width()
            .min(self.edit.renderer.layout.max_width)
            .round();
        let dimensions = Vec2::new(width, height);
        let (height_updated, width_updated) = match self.prev_dimensions {
            Some(prev) => (prev.y != dimensions.y, prev.x != dimensions.x),
            None => (true, true),
        };
        let height_shrunk = self
            .prev_dimensions
            .is_some_and(|prev| prev.y > dimensions.y);
        self.prev_dimensions = Some(dimensions);

        let dark_mode = ui.style().visuals.dark_mode;
        if dark_mode != self.edit.renderer.dark_mode {
            self.edit.renderer.syntax.clear();
            self.edit.renderer.dark_mode = dark_mode;
        }

        // Mirror the opt-in fetch preference onto the renderer each frame.
        self.edit.renderer.contact_linked_sites = self.persistence.get_contact_linked_sites();

        let start = web_time::Instant::now();

        if height_updated {
            self.edit.renderer.layout_cache.clear();
        }

        let embeds_seq = self.edit.renderer.embeds.seq();
        let embeds_updated = embeds_seq != self.embeds_seq;
        self.embeds_seq = embeds_seq;

        let link_seq = self.edit.renderer.previews.seq();
        let link_meta_updated = link_seq != self.link_seq;
        self.link_seq = link_seq;

        // --- input phase ------------------------------------------------------
        // Route workspace-origin events (toolbar Markdown, Undo/Redo) through
        // MdEdit's internal event queue, then let MdEdit::handle_input drain
        // everything (workspace + keyboard + completions).
        let workspace_events = self.edit.drain_workspace_events(ui.ctx());
        self.edit.event.internal_events.extend(workspace_events);

        let prior_selection = self.edit.renderer.buffer.current.selection;
        let prior_entered_atom = self.edit.renderer.entered_atom;
        let buf_resp = self.handle_input(ui);
        resp.open_camera = buf_resp.open_camera;

        if !self.initialized || buf_resp.text_updated {
            resp.text_updated = true;
            self.find
                .ensure_matches(&self.edit.renderer.buffer, self.edit.renderer.text_seq);
            ui.ctx().request_repaint();
        }
        resp.selection_updated = std::mem::take(&mut self.selection_moved)
            || prior_selection
                != self
                    .edit
                    .in_progress_selection
                    .unwrap_or(self.edit.renderer.buffer.current.selection);
        // Loads completing (embeds, link meta) and atom enter/exit reflow
        // layout, moving the selection's rects without moving the selection.
        // Report as a scroll: iOS re-fetches geometry on either signal, but a
        // selection report also resets the keyboard's QuickType context.
        resp.scroll_updated |= embeds_updated
            || link_meta_updated
            || prior_entered_atom != self.edit.renderer.entered_atom;

        let all_selected = self.edit.renderer.buffer.current.selection
            == (0.into(), self.edit.renderer.last_cursor_position());
        if self.initialized
            && buf_resp.selection_user_moved
            && !all_selected
            && self.edit.in_progress_block_drag.is_none()
        {
            self.edit.pending_scroll = Some(ScrollTarget::Cursor);
        }

        let ast_elapsed = start.elapsed();
        let print_elapsed = std::time::Duration::ZERO;
        let start = web_time::Instant::now();

        // --- draw phase (back to front) ---------------------------------------
        // Re-parse for render. handle_input parsed its own; that arena has
        // been dropped. The parse is assumed cheap (~1 ms).
        let arena = Arena::new();
        let root = {
            let _parse = tracing::trace_span!("MarkdownEditor::parse").entered();
            let options = MdRender::comrak_options();
            let text_with_newline = self.edit.renderer.buffer.current.text.to_string() + "\n";
            comrak::parse_document(&arena, &text_with_newline, &options)
        };

        if PRINT {
            println!(
                "{}",
                "================================================================================"
                    .bright_black()
            );
            print_ast(root);
        }

        ui.painter().rect_filled(
            ui.max_rect(),
            0.,
            self.edit.renderer.ctx.get_lb_theme().neutral_bg(),
        );
        self.edit.renderer.apply_theme(ui);
        ui.spacing_mut().item_spacing.x = 0.;

        let prev_offset = self.edit.scroll_area.stored_offset();

        let editor_shown = ui
            .vertical(|ui| {
                if self.edit.phone_mode {
                    self.show_find_bar(ui);

                    // ...then show editor content (or toolbar settings)...
                    let available_width = ui.available_width();
                    let toolbar_height = if cfg!(target_os = "android") {
                        ui.ctx()
                            .memory(|m| {
                                m.data.get_temp::<f32>(Id::unique(
                                    "android_native_markdown_toolbar_height",
                                ))
                            })
                            .unwrap_or(0.)
                    } else if !self.edit.renderer.readonly
                        && !self.edit.renderer.plaintext
                        && (self.virtual_keyboard_shown || self.toolbar.menu_open)
                    {
                        MOBILE_TOOL_BAR_SIZE
                    } else {
                        0.
                    };
                    self.next_resp.mobile_toolbar_shown = toolbar_height > 0.;

                    let avail = ui.available_rect_before_wrap();
                    self.next_resp.text_interaction_rect = Some(egui::Rect::from_min_max(
                        avail.min,
                        egui::pos2(avail.max.x, avail.max.y - toolbar_height),
                    ));

                    let editor_shown = ui
                        .allocate_ui(
                            egui::vec2(
                                ui.available_width(),
                                ui.available_height() - toolbar_height,
                            ),
                            |ui| {
                                if !self.toolbar.menu_open {
                                    // galleys / wrap_lines are cleared and
                                    // repopulated inside MdEdit::show — don't
                                    // clear here or input handling (which
                                    // reads last-frame galleys) sees nothing.
                                    self.edit.renderer.touch_targets.clear();
                                    self.show_scrollable_editor(ui, root);
                                    true
                                } else {
                                    // the open menu is a popup: its touches
                                    // stay with egui, not the iOS text page
                                    self.edit.renderer.touch_targets.clear();
                                    self.edit
                                        .renderer
                                        .touch_targets
                                        .push((ui.max_rect(), TouchTarget::Popup));
                                    self.show_toolbar_menu(ui);
                                    false
                                }
                            },
                        )
                        .inner;

                    // ...then show toolbar at the bottom
                    if !cfg!(target_os = "android")
                        && !self.edit.renderer.readonly
                        && !self.edit.renderer.plaintext
                        && (self.virtual_keyboard_shown || self.toolbar.menu_open)
                    {
                        let (_, rect) =
                            ui.allocate_space(egui::vec2(available_width, MOBILE_TOOL_BAR_SIZE));
                        self.mobile_toolbar_rect = Some(rect);
                        ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
                            self.show_toolbar(root, ui);
                        });
                    }

                    editor_shown
                } else {
                    // no menu toggle in this layout; clear in case the menu was
                    // open when a hardware keyboard was attached (it disables the buttons)
                    self.toolbar.menu_open = false;

                    if !cfg!(target_os = "android")
                        && !self.edit.renderer.readonly
                        && !self.edit.renderer.plaintext
                    {
                        self.show_toolbar(root, ui);
                    }
                    self.show_find_bar(ui);

                    self.next_resp.text_interaction_rect = Some(ui.available_rect_before_wrap());

                    // galleys / wrap_lines are cleared and repopulated inside
                    // MdEdit::show — don't clear here or input handling (which
                    // reads last-frame galleys) sees nothing.
                    self.edit.renderer.touch_targets.clear();

                    self.show_scrollable_editor(ui, root);
                    true
                }
            })
            .inner;

        if editor_shown {
            // a caret a scrolled block hides: a platform drawing its own hides it
            let selection = self.edit.renderer.buffer.current.selection;
            self.next_resp.caret_covered |=
                selection.0 == selection.1 && self.edit.renderer.caret_scrolled_out(selection.1);
            let new_offset = self.edit.scroll_area.stored_offset();
            self.next_resp.scroll_updated = new_offset != prev_offset;
            self.edit.scroll_area_velocity = self.edit.scroll_area.velocity();

            // persistence: read on first frame
            if !self.initialized {
                let persisted = self
                    .persistence
                    .get_markdown()
                    .file
                    .get(&self.edit.file_id)
                    .cloned()
                    .unwrap_or_default();
                if let Some((anchor_idx, intra_precise)) = persisted.anchor {
                    let arena = Arena::new();
                    let root = self.edit.renderer.reparse(&arena);
                    let content = scroll_content::DocScrollContent::for_frame(
                        &self.edit.renderer,
                        root,
                        self.edit.scroll_area.state.viewport_height,
                    );
                    let anchor = if self.edit.renderer.plaintext {
                        DocRowId::Line(anchor_idx)
                    } else {
                        DocRowId::Block(anchor_idx)
                    };
                    let off = Offset::new(anchor, intra_precise);
                    self.edit.scroll_area.set_offset(&content, off);
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
                self.edit
                    .renderer
                    .buffer
                    .queue(vec![lb_rs::model::text::operation_types::Operation::Select(selection)]);
                self.edit.renderer.buffer.update();
                self.edit.renderer.reveal_seq = self
                    .edit
                    .renderer
                    .ws_seq
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
        }

        // Selection can change both in `handle_input` and later during
        // `MdEdit::show` pointer handling (e.g. double-click word select on
        // Android). Compute this after render so the response sees both paths.
        resp.selection_updated |= prior_selection
            != self
                .edit
                .in_progress_selection
                .unwrap_or(self.edit.renderer.buffer.current.selection);

        // Completion popups render last, outside the scroll area's clip, so
        // they composite over the toolbar / find widget when the cursor is
        // near the top of the document. `edit.show` already submitted the
        // editor's text callback; popups land on a later glyphon layer.
        self.edit.show_completions(ui);

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
        if embeds_updated || height_updated || width_updated {
            if embeds_updated {
                self.unprocessed_scroll = Some(Instant::now());
            }
            ui.ctx().request_repaint();
        } else if resp.selection_updated {
            ui.ctx().request_repaint();
        }
        // Pull the cursor out from behind the virtual keyboard: scroll on
        // its rising edge, and keep re-asserting while the viewport shrinks
        // across the show animation. A growing viewport hides nothing.
        let keyboard_just_shown = self.virtual_keyboard_shown && !self.prev_virtual_keyboard_shown;
        self.prev_virtual_keyboard_shown = self.virtual_keyboard_shown;
        // A viewport of no height is a transition (rotation); nothing to reveal.
        if self.initialized
            && self.edit.renderer.touch_mode
            && dimensions.y > 0.0
            && (keyboard_just_shown || (height_shrunk && self.virtual_keyboard_shown))
        {
            // Under find, the keyboard is the field's: keep the match in view.
            self.edit.pending_scroll = Some(if self.find.focused(ui.ctx()) {
                ScrollTarget::FindMatch
            } else {
                ScrollTarget::Cursor
            });
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
        // persistence: write — sync the in-memory store each frame,
        // flag the disk write only on actual change. Empty-selection
        // cursor moves must persist too, so the trigger compares
        // against stored state rather than a within-frame change.
        let mut persistence_updated = false;
        let current_selection = self.edit.renderer.buffer.current.selection;
        {
            let mut persistence = self.persistence.data.write().unwrap();
            let entry = persistence
                .markdown
                .file
                .entry(self.edit.file_id)
                .or_default();
            if entry.selection != current_selection {
                entry.selection = current_selection;
                persistence_updated = true;
            }
        }

        let mut scroll_end_processed = false;
        if let Some(unprocessed_scroll) = self.unprocessed_scroll {
            if unprocessed_scroll.elapsed() > Duration::from_millis(100) && editor_shown {
                let arena = Arena::new();
                let root = self.edit.renderer.reparse(&arena);
                // Project the persisted offset onto current rows so a
                // stale anchor (block deleted since save) is recovered
                // rather than persisted as-is.
                let content = scroll_content::DocScrollContent::for_frame(
                    &self.edit.renderer,
                    root,
                    self.edit.scroll_area.state.viewport_height,
                );
                let anchor =
                    self.edit
                        .scroll_area
                        .offset(&content)
                        .and_then(|off| match off.anchor {
                            DocRowId::Block(i) | DocRowId::Line(i) => Some((i, off.intra_precise)),
                            // Cursor in leading/trailing pad isn't a block —
                            // persist None and let the next open default.
                            DocRowId::Leading | DocRowId::Trailing => None,
                        });
                drop(content);

                let mut persistence = self.persistence.data.write().unwrap();
                persistence
                    .markdown
                    .file
                    .entry(self.edit.file_id)
                    .and_modify(|f| {
                        f.anchor = anchor;
                    })
                    .or_insert(MdFilePersistence { anchor, selection: Default::default() });
                persistence_updated = true;

                scroll_end_processed = true;
            }
        };

        if scroll_end_processed {
            self.unprocessed_scroll = None;
        }
        if persistence_updated {
            self.persistence.write_to_file();
        }

        // focus editor when first shown or when nothing else has focus — not
        // while the menu covers it: egui re-drops the unshown editor's focus,
        // and the flip-flop repaint-loops and flickers the menu (#4896)
        if editor_shown && (!self.initialized || ui.memory(|m| m.focused().is_none())) {
            self.focus(ui.ctx());
        }
        if self.focused(ui.ctx()) {
            self.focus_lock(ui.ctx());
        }

        self.initialized = true;

        resp
    }

    pub fn will_consume_touch(&self, pos: Pos2) -> bool {
        self.touches_interactive_element(pos)
            || self.edit.scroll_area_velocity.abs().max_elem() > 0. // velocity zeroed at touch down
            || self.edit.scroll_area.momentum_cancel_press() // platform can check at touch up
            || self.reorder_armed() // a committed reorder isn't a tap
            || self.toolbar.menu_open
    }

    /// Whether a reorder is committed and dragging (`Armed`, not the
    /// still-`Pending` hold). Gates keyboard/cursor suppression — a `Pending`
    /// hold may still resolve to a tap, which must focus normally.
    pub fn reorder_armed(&self) -> bool {
        use widget::block::drag::TouchReorder;
        matches!(self.edit.touch_reorder, TouchReorder::Armed { .. })
    }

    /// Whether `pos` is over an interactive element (checkbox, fold button,
    /// link, spoiler, scrollbar, popup). Excludes [`Self::will_consume_touch`]'s
    /// transient terms so a recognizer's own pre-recognition scroll can't veto it.
    pub fn touches_interactive_element(&self, pos: Pos2) -> bool {
        self.edit
            .renderer
            .touch_targets
            .iter()
            .any(|(rect, _)| rect.contains(pos))
    }

    #[tracing::instrument(name = "MarkdownEditor::input", level = "trace", skip_all)]
    fn handle_input(&mut self, ui: &Ui) -> lb_rs::model::text::buffer::Response {
        self.edit.handle_input(ui.ctx(), self.id())
    }

    #[tracing::instrument(name = "MarkdownEditor::paint", level = "trace", skip_all)]
    fn show_scrollable_editor<'a>(&mut self, ui: &mut Ui, _root: &'a AstNode<'a>) {
        let scroll_id = self.id();

        // Captured by the Frame::canvas closure so post-render code
        // (find-match scroll) can position galleys against the canvas
        // origin without re-deriving it.
        let mut captured_canvas_rect: Option<Rect> = None;

        Frame::canvas(ui.style())
            .inner_margin(Margin::ZERO)
            .stroke(Stroke::NONE)
            // Frame's own fill sizes to the content min_rect, which the
            // off-screen neighbor rows (and their interactive allocations)
            // inflate above the viewport — painting the background over the
            // toolbar and tab strip during scroll. Paint it ourselves,
            // clipped to the viewport, below.
            .fill(egui::Color32::TRANSPARENT)
            .show(ui, |ui| {
                // Claim full available width with 0 vertical space so the
                // Frame stretches horizontally regardless of how narrow the
                // visible content turns out to be.
                let avail_w = ui.available_width();
                ui.allocate_space(Vec2::new(avail_w, 0.0));

                let canvas_rect = ui.max_rect();
                captured_canvas_rect = Some(canvas_rect);
                let canvas_width = canvas_rect.width();
                let layout_margin = self.edit.renderer.layout.margin;

                let arena = Arena::new();
                let root = self.edit.renderer.reparse(&arena);

                // Content column width caps at `max_width`; the affine
                // widget spans the full canvas so its scrollbar lands
                // at the canvas right edge. A docked outline narrows the
                // column, and the two center together as one block.
                let max_width = self.edit.renderer.layout.max_width;
                let r = &self.edit.renderer;
                let sidecar = !r.touch_mode
                    && !self.in_preview
                    && !r.plaintext
                    && r.interactive
                    && self.persistence.get_sidecar_open();
                // The text keeps its width for the card and only slides: left,
                // as far as the margin allows, to dock the card beside it;
                // without room the card floats over the text's right side.
                // Where the text stands follows the canvas without a step, so
                // crossing from docked to floating moves nothing.
                let edge = crate::style::Space::Md.pts();
                let side = SIDECAR_GAP + SIDECAR_W + edge;
                let col_width = (canvas_width - 2.0 * layout_margin).min(max_width).max(0.0);
                let centered = ((canvas_width - col_width) / 2.0).max(0.0);
                let beside = canvas_width - side - col_width;
                let docks = beside >= layout_margin;
                let open_left = beside.max(layout_margin).min(centered);
                self.edit.renderer.set_width(col_width);
                // `shown` fades the card; the text eases the same both
                // ways, so a reversal starts where it is
                let shown = ui.ctx().animate_bool_with_time(
                    Id::unique("md_sidecar_glide").with(self.edit.file_id),
                    sidecar,
                    widget::outline::MOTION_SECS,
                );
                let t = egui::emath::easing::quadratic_in_out(shown);

                // affine render: widget body spans the full canvas
                // (scrollbar at canvas right); content centers within
                // that body via `content_x`.
                let touch_scroll = self.edit.renderer.touch_mode;
                let content_x = canvas_rect.min.x + egui::lerp(centered..=open_left, t);
                self.card = (shown > 0.0).then(|| {
                    let width = SIDECAR_W.min(canvas_width - 2.0 * edge).max(0.0);
                    let x = match docks {
                        true => content_x + col_width + SIDECAR_GAP,
                        false => canvas_rect.right() - edge - width,
                    };
                    widget::outline::CardPlace {
                        area: Rect::from_min_size(
                            Pos2::new(x, canvas_rect.top() + edge),
                            Vec2::new(width, (canvas_rect.height() - 2.0 * edge).max(0.0)),
                        ),
                        floating: !docks,
                        shown,
                        open: sidecar,
                    }
                });
                let pre = ui
                    .scope_builder(UiBuilder::new().max_rect(canvas_rect), |ui| {
                        ui.set_clip_rect(canvas_rect);
                        // Editor background, bounded to the viewport (see the
                        // `fill(TRANSPARENT)` note on the Frame). First shape in
                        // the scope, so it sits behind all content.
                        ui.painter().rect_filled(
                            canvas_rect,
                            0.0,
                            self.edit.renderer.ctx.get_lb_theme().neutral_bg(),
                        );
                        self.edit.scroll_area.touch_scroll = touch_scroll;
                        // An armed touch reorder owns the gesture — don't
                        // scroll the body under the dragged item.
                        self.edit.scroll_area.suppress_body_drag = matches!(
                            self.edit.touch_reorder,
                            widget::block::drag::TouchReorder::Armed { .. }
                        );

                        // Body alloc → pre_render → scrollbar/content
                        // ordering puts pre_render's click rect above
                        // the body's drag (so taps reach it) but below
                        // the scrollbar (so taps on the bar don't).
                        let begun = self.edit.scroll_area.begin(ui);
                        if self.edit.pan_rows(ui, begun.body_response()) {
                            self.edit.scroll_area.suppress_body_drag = true;
                        }

                        let pre = self.edit.pre_render(ui, canvas_rect, scroll_id, root);

                        self.edit.renderer.fragments.clear();
                        self.edit.renderer.block_boxes.clear();
                        self.edit.renderer.table_cell_boxes.clear();
                        self.edit.renderer.table_header_slots.clear();
                        self.edit.renderer.in_progress_block_drag =
                            self.edit.in_progress_block_drag;
                        self.edit.renderer.bounds.wrap_lines.clear();
                        self.edit.renderer.text_areas.clear();
                        self.edit.renderer.deco_lines.clear();
                        self.edit.renderer.clear_row_scroll_frame();

                        // Phase 1: drive scroll math + scrollbar, and
                        // pick up to one viewport's worth of rows above
                        // and below the visible window so navigation
                        // (advance_by_line) finds fragments on off-
                        // screen rows. Without this buffer, holding
                        // up/down arrow at a viewport edge sticks
                        // because `fragments` only contains visible-row
                        // entries. The immutable `DocScrollContent`
                        // borrow is released here before phase 2's
                        // mutable renderer paint.
                        self.land_before_paint(root, canvas_rect);
                        self.edit.renderer.landing_heading = self.landing.and_then(Landing::start);
                        let (visible, neighbors, far, scrollbar_grab) = {
                            use crate::widgets::affine_scroll::{Offset, Rows as _, VisibleRow};

                            let content = scroll_content::DocScrollContent::for_frame(
                                &self.edit.renderer,
                                root,
                                canvas_rect.height(),
                            );
                            let resp = self.edit.scroll_area.finish(ui, begun, &content);
                            if resp.user_scrolled {
                                self.outline_follows_caret = false;
                            }

                            let viewport_height = canvas_rect.height();
                            let mut neighbors: Vec<VisibleRow<scroll_content::DocRowId>> =
                                Vec::new();
                            if let Some(first) = resp.visible.first() {
                                let mut id = first.id;
                                let mut top = first.top;
                                let mut walked = 0.0f32;
                                while walked < viewport_height {
                                    let Some(prev_id) = content.prev(&id) else { break };
                                    let height = content.precise(&prev_id);
                                    top -= height;
                                    neighbors.push(VisibleRow { id: prev_id, top, height });
                                    walked += height;
                                    id = prev_id;
                                }
                            }
                            if let Some(last) = resp.visible.last() {
                                let mut id = last.id;
                                let mut top = last.top + last.height;
                                let mut walked = 0.0f32;
                                while walked < viewport_height {
                                    let Some(next_id) = content.next(&id) else { break };
                                    let height = content.precise(&next_id);
                                    neighbors.push(VisibleRow { id: next_id, top, height });
                                    top += height;
                                    walked += height;
                                    id = next_id;
                                }
                            }

                            // Rows holding the selection's ends beyond
                            // that band, at the scroll model's estimate
                            // of their distance, kept outside the band.
                            // Their layout is exact within the row, so a
                            // platform text system gets real geometry
                            // for a far end without the rows between.
                            let band_top = resp
                                .visible
                                .iter()
                                .chain(neighbors.iter())
                                .map(|r| r.top)
                                .fold(f32::INFINITY, f32::min);
                            let band_bottom = resp
                                .visible
                                .iter()
                                .chain(neighbors.iter())
                                .map(|r| r.top + r.height)
                                .fold(f32::NEG_INFINITY, f32::max);
                            let index = |id: &scroll_content::DocRowId| match id {
                                scroll_content::DocRowId::Block(i)
                                | scroll_content::DocRowId::Line(i) => Some(*i),
                                _ => None,
                            };
                            let band_first = resp
                                .visible
                                .iter()
                                .chain(neighbors.iter())
                                .filter_map(|r| index(&r.id))
                                .min();
                            let selection = self
                                .edit
                                .in_progress_selection
                                .unwrap_or(self.edit.renderer.buffer.current.selection);
                            let mut far: Vec<VisibleRow<scroll_content::DocRowId>> = Vec::new();
                            let landing = self.landing.and_then(Landing::start);
                            for end in [selection.0, selection.1].into_iter().chain(landing) {
                                let Some(row) = content.find_text_row(end) else { continue };
                                let (Some(i), Some(first)) = (index(&row), band_first) else {
                                    continue;
                                };
                                let laid_out = resp
                                    .visible
                                    .iter()
                                    .chain(neighbors.iter())
                                    .chain(far.iter())
                                    .any(|r| r.id == row);
                                if laid_out {
                                    continue;
                                }
                                let height = content.precise(&row);
                                if height <= 0.0 {
                                    continue;
                                }
                                let estimate = self
                                    .edit
                                    .scroll_area
                                    .state
                                    .viewport_y_of(&content, &Offset::at_top_of(row));
                                let top = if i < first {
                                    estimate.min(band_top - height)
                                } else {
                                    estimate.max(band_bottom)
                                };
                                far.push(VisibleRow { id: row, top, height });
                            }
                            (resp.visible, neighbors, far, resp.scrollbar_grab)
                        };
                        // Register the scrollbar's grab area so iOS taps on it
                        // don't fall through to cursor-placement / keyboard-
                        // summon handlers.
                        self.edit
                            .renderer
                            .touch_targets
                            .extend(scrollbar_grab.map(|rect| (rect, TouchTarget::Scrollbar)));

                        // Phase 2: paint each visible row with a mutable
                        // renderer borrow. Block list re-collected
                        // because the immutable borrow that held
                        // `DocScrollContent`'s copy was just dropped.
                        // Neighbor rows paint off-screen — clipped by
                        // the canvas — only to register fragments and
                        // wrap-line bounds for navigation.
                        let blocks: Vec<_> = root.children().collect();
                        self.painted_rows.clear();
                        self.painted_canvas = canvas_rect;
                        for vrow in visible.iter().chain(neighbors.iter()).chain(far.iter()) {
                            self.painted_rows.push((vrow.id, vrow.top));
                            let top_left =
                                Pos2::new(canvas_rect.min.x, canvas_rect.min.y + vrow.top);
                            let from = self.edit.renderer.fragments.len();
                            scroll_content::paint_row(
                                ui,
                                &mut self.edit.renderer,
                                root,
                                &blocks,
                                &vrow.id,
                                top_left,
                                content_x,
                            );
                            if far.contains(vrow) {
                                for f in &mut self.edit.renderer.fragments[from..] {
                                    f.far = true;
                                }
                            }
                        }

                        // paint find match highlights after galleys positioned
                        // but before text callback runs
                        if !self.find.matches.is_empty() {
                            let theme = self.edit.renderer.ctx.get_lb_theme();
                            let highlight_color = theme.neutral_bg_tertiary();
                            let current_color =
                                theme.fg().yellow.lerp_to_gamma(theme.neutral_bg(), 0.5);
                            for (i, &match_range) in self.find.matches.iter().enumerate() {
                                let color = if self.find.current_match == Some(i) {
                                    current_color
                                } else {
                                    highlight_color
                                };
                                self.edit.show_range(ui, match_range, color);
                            }
                        }

                        // read-only search-preview snippet highlight
                        if let Some(range) = self.edit.renderer.preview_match {
                            let theme = self.edit.renderer.ctx.get_lb_theme();
                            let color = theme.fg().yellow.lerp_to_gamma(theme.neutral_bg(), 0.5);
                            self.edit.show_range(ui, range, color);
                        }

                        if let Some((range, at)) = self.heading_flash {
                            let t = at.elapsed().as_secs_f32() / HEADING_FLASH_SECS;
                            if t < 1.0 {
                                let theme = self.edit.renderer.ctx.get_lb_theme();
                                let color = theme.accent().gamma_multiply(0.3 * (1.0 - t));
                                self.edit.show_range(ui, range, color);
                                ui.ctx().request_repaint();
                            } else {
                                self.heading_flash = None;
                            }
                        }

                        if let Some(range) = self.edit.renderer.platform_marked {
                            let theme = self.edit.renderer.ctx.get_lb_theme();
                            let color = theme.accent().lerp_to_gamma(theme.neutral_bg(), 0.75);
                            self.edit.show_range(ui, range, color);
                        }

                        // List-item drag-to-reorder: detect the touch
                        // long-press, then consume the handle action and
                        // commit on release (visuals are drawn after
                        // post_render so they composite on top).
                        self.edit.detect_touch_reorder(ui, self.keyboard_visible);
                        self.edit.handle_block_drag(ui);
                        pre
                    })
                    .inner;
                self.edit.renderer.fragments.sort_by_key(|f| f.source_range);
                // Neighbor rows above the visible window paint after
                // visible rows, so wrap-lines lose their natural source
                // order. `Bound::Line` lookup is a linear `range_before`
                // / `range_after` walk that assumes sorted ranges.
                self.edit.renderer.bounds.wrap_lines.sort_by_key(|r| r.0);

                // Favicon + selection tint, over the opaque capsule pills.
                self.edit
                    .renderer
                    .show_capsule_overlays(ui, root, canvas_rect, 0);

                self.edit.post_render(ui, canvas_rect, scroll_id, pre);
                self.edit.draw_dragged_overlay(ui, root);
                self.show_outline(ui, root, canvas_rect);
                ui.advance_cursor_after_rect(canvas_rect);
            });

        self.find
            .ensure_matches(&self.edit.renderer.buffer, self.edit.renderer.text_seq);

        if let Some(slug) = self.edit.renderer.heading_copy.take() {
            ui.ctx().copy_text(self.edit.heading_link(&slug));
        }

        if matches!(self.edit.pending_scroll, Some(ScrollTarget::FindMatch)) {
            self.edit.pending_scroll = None;
            if let Some(canvas_rect) = captured_canvas_rect {
                self.scroll_to_find_match(canvas_rect);
            }
        }

        if matches!(self.edit.pending_scroll, Some(ScrollTarget::Preview)) {
            self.edit.pending_scroll = None;
            if let Some(canvas_rect) = captured_canvas_rect {
                self.scroll_to_preview(canvas_rect);
            }
        }

        if matches!(self.edit.pending_scroll, Some(ScrollTarget::Heading)) {
            self.edit.pending_scroll = None;
            if let Some(canvas_rect) = captured_canvas_rect {
                self.scroll_to_heading(canvas_rect);
            }
        } else {
            // another scroll took over before the landing's row was laid out
            self.landing = None;
        }
    }

    /// The find bar above the document, with the page scrolled by its height
    /// as it opens and closes so the content stays put on screen.
    fn show_find_bar(&mut self, ui: &mut Ui) {
        let height = self.show_find_centered(ui);
        // The bar has no height in the frame it opens, and its height can
        // change with the keyboard: the page scrolls by each change.
        let target = if self.find.term.is_some() { height } else { 0.0 };
        if target != self.find_bar_height {
            self.edit
                .scroll_area
                .gesture_scroll(target - self.find_bar_height);
            self.find_bar_height = target;
        }
    }

    fn show_find_centered(&mut self, ui: &mut Ui) -> f32 {
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
            self.find.show(
                &self.edit.renderer.buffer,
                self.edit.renderer.text_seq,
                self.virtual_keyboard_shown,
                ui,
            )
        });
        let find_output = scope_resp.inner;
        let rendered_rect = scope_resp.response.rect;
        ui.advance_cursor_after_rect(rendered_rect);

        self.edit.event.internal_events.extend(find_output.events);
        if find_output.scroll_to_match {
            self.edit.pending_scroll = Some(ScrollTarget::FindMatch);
        }
        if find_output.closed {
            self.focus(ui.ctx());
        }

        let new_match = self.find.current_match_range();
        self.edit.renderer.find_current_match = new_match;
        if prev_match != new_match {
            self.edit.renderer.reveal_seq = self
                .edit
                .renderer
                .ws_seq
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        rendered_rect.height()
    }

    fn scroll_to_find_match(&mut self, canvas_rect: Rect) {
        let Some(idx) = self.find.current_match else { return };
        let Some(&match_range) = self.find.matches.get(idx) else { return };

        let arena = Arena::new();
        let root = self.edit.renderer.reparse(&arena);
        let content = scroll_content::DocScrollContent::for_frame(
            &self.edit.renderer,
            root,
            canvas_rect.height(),
        );

        let Some(target_rect) = build_target_reveal(
            &self.edit.renderer,
            &content,
            &self.edit.scroll_area.state,
            match_range,
            canvas_rect,
            self.edit.renderer.layout.row_spacing / 2.0,
        ) else {
            return;
        };
        self.edit
            .scroll_area
            .reveal(&content, target_rect, Align::Center);
    }

    /// Opens or closes the outline from native chrome: on touch, a sheet;
    /// with a mouse, the card, remembered for every note.
    pub fn toggle_outline(&mut self) {
        if self.edit.renderer.touch_mode {
            self.outline.toggle();
        } else {
            let open = self.persistence.get_sidecar_open();
            self.persistence.set_sidecar_open(!open);
        }
    }

    /// Whether the outline shows: the card or the sheet.
    pub fn outline_open(&self) -> bool {
        self.card.is_some_and(|c| c.open) || self.outline.is_open()
    }

    /// The outline, docked beside the text or opened over the note.
    fn show_outline<'a>(&mut self, ui: &mut Ui, root: &'a AstNode<'a>, canvas: Rect) {
        if std::mem::take(&mut self.edit.caret_moved) {
            self.outline_follows_caret = true;
        }
        let r = &self.edit.renderer;
        self.outline_offered = !r.plaintext && r.interactive && !self.in_preview;
        if !self.outline_offered || self.card.is_none() && !self.outline.is_open() {
            return;
        }

        // the caret's section while typing, else the one being read
        let caret = r.buffer.current.selection.1;
        let caret_shown = r.fragment_at_offset(caret).is_some_and(|f| {
            !f.far && f.rect.bottom() > canvas.top() && f.rect.top() < canvas.bottom()
        });
        let mut typing = if r.touch_mode {
            self.keyboard_visible
        } else {
            self.outline_follows_caret && caret_shown
        };
        if self.outline.is_opening() {
            self.outline_from_typing = typing;
            // the sheet stands where the keyboard is
            if r.touch_mode && self.keyboard_visible {
                crate::tab::ExtendedOutput::set_virtual_keyboard_shown(ui.ctx(), false);
            }
        }
        if self.outline.is_open() {
            typing = self.outline_from_typing;
        }
        let current = match typing {
            true => self
                .outline_rows(root)
                .iter()
                .rposition(|row| row.start <= caret),
            false => self.scrolled_to_heading(root),
        };
        let (seq, rows) = self.outline_rows.take().unwrap_or_default();

        let id = Id::unique("md_outline").with(self.edit.file_id);
        let (version, linked) = self.linked_notes();
        let (chosen, popup) = if self.edit.renderer.touch_mode {
            self.outline
                .show(ui.ctx(), id.with("sheet"), &rows, current, &linked)
        } else {
            let (chosen, rect, dismissed) =
                self.outline
                    .card(ui.ctx(), id.with("card"), self.card, &rows, current, &linked);
            if dismissed {
                self.persistence.set_sidecar_open(false);
            }
            (chosen, rect.filter(|_| self.card.is_some_and(|c| c.floating && c.open)))
        };
        self.linked = Some((version, linked));
        self.outline_rows = Some((seq, rows));
        if let Some(rect) = popup {
            self.edit
                .renderer
                .touch_targets
                .push((rect, TouchTarget::Popup));
        }
        match chosen {
            Some(widget::outline::Choice::Heading(slug)) => {
                self.open_at_heading(&slug);
                // a reader who jumped didn't ask for the keyboard
                if !self.edit.renderer.touch_mode || self.keyboard_visible {
                    self.focus(ui.ctx());
                }
            }
            Some(widget::outline::Choice::Note { id, span }) => {
                use crate::tab::ExtendedOutput as _;
                let new_tab = ui.input(|i| i.modifiers.command);
                match span {
                    Some(span) => ui.ctx().open_link_at(id, span, new_tab),
                    None => ui.ctx().open_link(id, None, new_tab),
                }
            }
            None => {}
        }
    }

    /// The notes with links to this one, by path, each with its links here
    /// in order, kept until the index changes.
    fn linked_notes(&mut self) -> (u64, Vec<widget::outline::LinkedNote>) {
        use crate::file_cache::FilesExt as _;
        let files = self.edit.renderer.files.read().unwrap();
        files.notes.want();
        let version = files.notes.version();
        if let Some((at, linked)) = self.linked.take() {
            if at == version {
                return (at, linked);
            }
        }
        let mut linked: Vec<widget::outline::LinkedNote> = vec![];
        for b in files.notes.backlinks(&*files, self.edit.file_id) {
            let mention = widget::outline::Mention {
                context: b.link.context,
                label: b.link.label,
                span: b.link.span,
            };
            match linked.last_mut() {
                Some(note) if note.id == b.note => note.links.push(mention),
                _ => {
                    let name = files.get_by_id(b.note).map_or("", |f| f.name.as_str());
                    linked.push(widget::outline::LinkedNote {
                        id: b.note,
                        title: crate::file_cache::wiki_title(name).to_string(),
                        links: vec![mention],
                    });
                }
            }
        }
        (version, linked)
    }

    pub fn open_navigate(&mut self, byte_range: std::ops::Range<usize>) {
        let segs = &self.edit.renderer.buffer.current.segs;
        let last = segs.last_cursor_position();
        let start = segs
            .byte_to_char_floor(Byte(byte_range.start))
            .clamp(0.into(), last);
        let end = segs
            .byte_to_char_ceil(Byte(byte_range.end))
            .clamp(0.into(), last);
        self.select_from_outside((start, end));
        self.edit.pending_scroll = Some(ScrollTarget::Cursor);
    }

    /// Lands on the text at `byte_range`, as on a heading: near the top of
    /// the view, washed for a moment, any fold hiding it opened, and the
    /// caret at its start except while reading on touch.
    pub fn open_at_text(&mut self, byte_range: std::ops::Range<usize>) {
        let segs = &self.edit.renderer.buffer.current.segs;
        let last = segs.last_cursor_position();
        let start = segs
            .byte_to_char_floor(Byte(byte_range.start))
            .clamp(0.into(), last);
        let end = segs
            .byte_to_char_ceil(Byte(byte_range.end))
            .clamp(0.into(), last);
        let arena = Arena::new();
        self.edit.pending_scroll = Some(ScrollTarget::Heading);
        let reading = self.edit.renderer.touch_mode && !self.keyboard_visible;
        let prior = self.edit.renderer.buffer.current.selection;
        self.select_from_outside((start, end));
        if !self.edit.renderer.plaintext {
            self.edit.unfold_at_selection(&arena);
        }
        // an unfold above the text moves it; the selection moved with it
        let range = self.edit.renderer.buffer.current.selection;
        let caret = if reading { prior } else { (range.0, range.0) };
        self.select_from_outside(caret);
        self.landing = Some(Landing::Heading(range));
        self.heading_flash = Some((range, web_time::Instant::now()));
    }

    /// Lands on the heading `fragment` names: its line near the top of the
    /// view, washed for a moment, any fold hiding it opened. The caret goes
    /// after its text, except while reading on touch, where that would
    /// reveal its markup. A fragment that names nothing lands at the top, as
    /// a browser does.
    pub fn open_at_heading(&mut self, fragment: &str) {
        let arena = Arena::new();
        self.edit.pending_scroll = Some(ScrollTarget::Heading);
        let Some((mut start, mut caret)) = self.find_heading(&arena, fragment) else {
            self.landing = Some(Landing::Top);
            return;
        };
        let reading = self.edit.renderer.touch_mode && !self.keyboard_visible;
        let prior = self.edit.renderer.buffer.current.selection;
        self.select_from_outside((caret, caret));
        if !self.edit.renderer.plaintext && self.edit.unfold_at_selection(&arena).text_updated {
            // the fold's tag came out before the heading
            if let Some(found) = self.find_heading(&arena, fragment) {
                (start, caret) = found;
                self.select_from_outside((caret, caret));
            }
        } else if reading {
            self.select_from_outside(prior);
        }
        self.landing = Some(Landing::Heading((start, caret)));
        self.heading_flash = Some(((start, caret), web_time::Instant::now()));
    }

    /// The heading `fragment` names: where its text starts (its markup may
    /// be hidden, with nothing laid out to scroll to) and where it ends.
    fn find_heading<'a>(
        &mut self, arena: &'a Arena<'a>, fragment: &str,
    ) -> Option<(Grapheme, Grapheme)> {
        let root = self.edit.renderer.reparse(arena);
        let (nodes, headings): (Vec<_>, Vec<_>) = fragment::headings_in(root).into_iter().unzip();
        let node = nodes[fragment::find_heading(&headings, fragment)?];
        Some(self.edit.renderer.heading_span(node))
    }

    fn select_from_outside(&mut self, selection: (Grapheme, Grapheme)) {
        self.selection_moved = true;
        self.edit
            .renderer
            .buffer
            .queue(vec![lb_rs::model::text::operation_types::Operation::Select(selection)]);
        self.edit.renderer.buffer.update();
        self.edit.renderer.reveal_seq = self
            .edit
            .renderer
            .ws_seq
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    /// The heading scrolled to, by its index among the note's headings: the
    /// last at or above the [`reading_line`]. Opening at any heading of a
    /// note that scrolls makes it the one scrolled to.
    pub(crate) fn scrolled_to_heading<'a>(&mut self, root: &'a AstNode<'a>) -> Option<usize> {
        let spans: Vec<_> = self.outline_rows(root).iter().map(|row| row.span).collect();
        let r = &self.edit.renderer;
        let rows =
            scroll_content::DocScrollContent::for_frame(r, root, self.painted_canvas.height());
        let line = reading_line(&self.edit.scroll_area.state, &rows)?;
        scrolled_heading(&line, spans.len(), |i| self.heading_point(&rows, spans[i]))
    }

    /// The note's headings as the outline lists them, kept until the text
    /// changes.
    fn outline_rows<'a>(&mut self, root: &'a AstNode<'a>) -> &[widget::outline::OutlineRow] {
        let seq = self.edit.renderer.text_seq;
        if self.outline_rows.as_ref().is_none_or(|(at, _)| *at != seq) {
            let r = &self.edit.renderer;
            let rows = fragment::headings_in(root)
                .into_iter()
                .map(|(node, h)| widget::outline::OutlineRow {
                    level: h.level,
                    text: h.text,
                    slug: h.slug,
                    start: r.node_range(node).0,
                    span: r.heading_span(node),
                })
                .collect();
            self.outline_rows = Some((seq, rows));
        }
        self.outline_rows.as_ref().map_or(&[], |(_, rows)| rows)
    }

    /// Where a heading's first line is in the note: its row, and the top of
    /// what's drawn of `heading` (its text's start to its end) in it. Hidden
    /// markup draws nothing, so the highest thing drawn over the range stands
    /// for the line. A row laid out this frame draws only what's in view, so a
    /// heading in it not drawn is above the view if text in view follows it,
    /// else below. A row not laid out stands at its top: wholly above or
    /// below the view, it's never the one the reading line is in.
    fn heading_point(
        &self, rows: &scroll_content::DocScrollContent<'_, '_>, heading: (Grapheme, Grapheme),
    ) -> Option<DocOffset> {
        use crate::widgets::affine_scroll::Rows as _;
        let (start, end) = heading;
        let row = rows.find_text_row(start)?;
        let Some(&(_, row_top)) = self.painted_rows.iter().find(|(id, _)| *id == row) else {
            return Some(Offset::new(row, 0.0));
        };
        let view = self.painted_canvas;
        let drawn = || self.edit.renderer.fragments.iter();
        let top = drawn()
            .filter(|f| f.source_range.0 <= end && f.source_range.1 >= start)
            .map(|f| f.rect.top())
            .reduce(f32::min);
        let in_view = |f: &&widget::utils::wrap_layout::Fragment| {
            !f.far && f.rect.bottom() > view.top() && f.rect.top() < view.bottom()
        };
        let intra = match top {
            Some(top) => top - view.min.y - row_top,
            None if drawn().filter(in_view).any(|f| f.source_range.0 > end) => 0.0,
            None => rows.precise(&row),
        };
        Some(Offset::new(row, intra))
    }

    /// Lands where it can without painting: the note's top, or a heading that
    /// is a block of its own, whose line starts below its row's spacing. The
    /// frame then paints there once; any other landing waits for its row.
    fn land_before_paint<'ast>(&mut self, root: &'ast AstNode<'ast>, canvas_rect: Rect) {
        use scroll_content::DocRowId;
        if !matches!(self.edit.pending_scroll, Some(ScrollTarget::Heading)) {
            return;
        }
        let Some(landing) = self.landing else { return };
        let content = scroll_content::DocScrollContent::for_frame(
            &self.edit.renderer,
            root,
            canvas_rect.height(),
        );
        let target = match landing {
            Landing::Top => Offset::at_top_of(DocRowId::Leading),
            Landing::Heading((start, _)) => {
                let Some(row @ DocRowId::Block(i)) = content.find_text_row(start) else { return };
                let Some(node) = root.children().nth(i) else { return };
                if !matches!(node.data.borrow().value, comrak::nodes::NodeValue::Heading(_)) {
                    return;
                }
                let intra = self.edit.renderer.block_pre_spacing_height(node);
                heading_scroll(&self.edit.scroll_area.state, &content, Offset::new(row, intra))
            }
        };
        self.landing = None;
        self.edit.pending_scroll = None;
        self.edit
            .scroll_area
            .reveal(&content, Reveal::point_at(target), Align::Top);
    }

    /// The landing's line on the reading line, with room above it so it
    /// clears the chrome. Its row is laid out this frame wherever it is; a
    /// landing set after that waits a frame rather than scroll by estimate.
    fn scroll_to_heading(&mut self, canvas_rect: Rect) {
        let Some(landing) = self.landing else { return };
        let arena = Arena::new();
        let root = self.edit.renderer.reparse(&arena);
        let content = scroll_content::DocScrollContent::for_frame(
            &self.edit.renderer,
            root,
            canvas_rect.height(),
        );
        let target = match landing {
            Landing::Top => Offset::at_top_of(scroll_content::DocRowId::Leading),
            Landing::Heading(range) => {
                let at = range.0;
                let Some(row) = content.find_text_row(at) else {
                    self.landing = None;
                    return;
                };
                if !self.painted_rows.iter().any(|(id, _)| *id == row) {
                    self.edit.pending_scroll = Some(ScrollTarget::Heading);
                    self.edit.renderer.ctx.request_repaint();
                    return;
                }
                let Some(point) = self.heading_point(&content, range) else { return };
                heading_scroll(&self.edit.scroll_area.state, &content, point)
            }
        };
        self.landing = None;
        self.edit
            .scroll_area
            .reveal(&content, Reveal::point_at(target), Align::Top);
    }

    pub fn preview_navigate(&mut self, byte_range: Option<std::ops::Range<usize>>) {
        let segs = &self.edit.renderer.buffer.current.segs;
        let new_match = byte_range
            .map(|r| (segs.byte_to_char_floor(Byte(r.start)), segs.byte_to_char_ceil(Byte(r.end))));

        if self.edit.renderer.preview_match != new_match {
            self.edit.renderer.preview_match = new_match;
            // reveal state feeds cached block heights; invalidate like find does
            self.edit.renderer.reveal_seq = self
                .edit
                .renderer
                .ws_seq
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if new_match.is_some() {
                self.edit.pending_scroll = Some(ScrollTarget::Preview);
            }
        }
    }

    fn scroll_to_preview(&mut self, canvas_rect: Rect) {
        let Some(match_range) = self.edit.renderer.preview_match else { return };

        let arena = Arena::new();
        let root = self.edit.renderer.reparse(&arena);
        let content = scroll_content::DocScrollContent::for_frame(
            &self.edit.renderer,
            root,
            canvas_rect.height(),
        );

        let Some(target_rect) = build_target_reveal(
            &self.edit.renderer,
            &content,
            &self.edit.scroll_area.state,
            match_range,
            canvas_rect,
            self.edit.renderer.layout.row_spacing / 2.0,
        ) else {
            return;
        };
        self.edit
            .scroll_area
            .reveal(&content, target_rect, Align::Center);
    }
}

/// How long a heading landed on stays washed.
const HEADING_FLASH_SECS: f32 = 0.9;

/// The outline card's width, its gap from the text, and the narrowest text
/// it docks beside; in narrower windows it floats over the text.
const SIDECAR_W: f32 = 260.0;
const SIDECAR_GAP: f32 = 24.0;

/// Where a heading link lands.
#[derive(Clone, Copy)]
enum Landing {
    /// The note's top, for a heading that isn't there.
    Top,
    /// Text from its start to its end: a heading's, or a link's.
    Heading((Grapheme, Grapheme)),
}

impl Landing {
    /// Where a heading landed on starts.
    fn start(self) -> Option<Grapheme> {
        match self {
            Landing::Top => None,
            Landing::Heading((at, _)) => Some(at),
        }
    }
}

/// Room above a landed heading. Android's status and app bars cover more
/// of the page's top than its leading pad reserves, so it takes more.
fn landing_room() -> f32 {
    use crate::style::Space;
    if cfg!(target_os = "android") { Space::Xl.pts() + Space::Md.pts() } else { Space::Lg.pts() }
}

type DocOffset = Offset<scroll_content::DocRowId>;

/// Room above the reading line: the landing room, plus the bars an Android
/// page runs under.
fn reading_room(rows: &scroll_content::DocScrollContent<'_, '_>) -> f32 {
    landing_room() + android_top_overlay(rows)
}

/// The reading line's slide over the last screen of scrolling, in a note
/// that scrolls: `span` of scrolling before the bottom offset `max`, it
/// walks down the view from `room` below its top to `tail`, where the note's
/// last line rests at the bottom. It only walks down, so headings land in
/// order.
struct Slide {
    max: DocOffset,
    span: f32,
    tail: f32,
    room: f32,
}

impl Slide {
    fn new(
        state: &crate::widgets::affine_scroll::ScrollArea<scroll_content::DocRowId>,
        rows: &scroll_content::DocScrollContent<'_, '_>, room: f32,
    ) -> Option<Self> {
        use crate::widgets::affine_scroll::Rows as _;
        let screen = state.viewport_height;
        let max = state.max_offset(rows)?;
        let top = Offset::at_top_of(rows.first()?);
        // the bound budgets the walk; a farther distance may still return
        let range = state
            .precise_distance(rows, &top, &max, screen)
            .map_or(screen, |d| d.min(screen));
        (range > 0.0).then_some(Self {
            max,
            span: range,
            tail: (screen - rows.trailing_precise).max(room),
            room,
        })
    }

    /// How far below the view's top the line is, `r` short of the bottom.
    fn line_y(&self, r: f32) -> f32 {
        egui::lerp(self.room..=self.tail, 1.0 - (r / self.span).min(1.0))
    }

    /// The line's place relative to `max`, `r` short of the bottom. It
    /// falls as `r` grows, so each place has one scroll.
    fn line_from_max(&self, r: f32) -> f32 {
        self.line_y(r) - r
    }

    /// How far short of the bottom the line's place is `t` relative to
    /// `max`: [`Self::line_from_max`] inverted, within the slide.
    fn short_of_bottom(&self, t: f32) -> f32 {
        let pace = 1.0 + (self.tail - self.room) / self.span;
        ((self.tail - t) / pace).clamp(0.0, self.span)
    }
}

/// The reading line at the view's offset: `room` below its top, sliding over
/// the last screen of scrolling down to the note's last line. A function of
/// the rows, the offset, and the view's height alone; [`heading_scroll`] is
/// its inverse, so a heading opened is the heading scrolled to.
fn reading_line(
    state: &crate::widgets::affine_scroll::ScrollArea<scroll_content::DocRowId>,
    rows: &scroll_content::DocScrollContent<'_, '_>,
) -> Option<DocOffset> {
    use crate::widgets::affine_scroll::offset_by;
    let room = reading_room(rows);
    let off = state.offset(rows)?;
    if let Some(slide) = Slide::new(state, rows, room) {
        if let Some(r) = state
            .precise_distance(rows, &off, &slide.max, slide.span)
            .filter(|r| *r < slide.span)
        {
            let r = r.max(0.0);
            return Some(offset_by(rows, slide.max.clone(), slide.line_from_max(r)));
        }
    }
    Some(offset_by(rows, off, room))
}

/// The offset whose reading line falls on `target`: `room` above it, or
/// near the end the scroll where the sliding line meets it.
fn heading_scroll(
    state: &crate::widgets::affine_scroll::ScrollArea<scroll_content::DocRowId>,
    rows: &scroll_content::DocScrollContent<'_, '_>, target: DocOffset,
) -> DocOffset {
    use crate::widgets::affine_scroll::offset_by;
    let room = reading_room(rows);
    let above = offset_by(rows, target.clone(), -room);
    let Some(slide) = Slide::new(state, rows, room) else { return above };
    let Some(t) = state.precise_distance(rows, &slide.max, &target, state.viewport_height) else {
        return above;
    };
    if t <= slide.line_from_max(slide.span) {
        return above;
    }
    let r = slide.short_of_bottom(t);
    offset_by(rows, slide.max, -r)
}

/// The heading scrolled to: the last of `count` headings whose line,
/// `point(i)` in order down the note, is at or above the reading line `line`.
fn scrolled_heading(
    line: &DocOffset, count: usize, point: impl Fn(usize) -> Option<DocOffset>,
) -> Option<usize> {
    use scroll_content::DocRowId;
    let order = |id: &DocRowId| match id {
        DocRowId::Leading => 0,
        DocRowId::Block(i) | DocRowId::Line(i) => i + 1,
        DocRowId::Trailing => usize::MAX,
    };
    let at_or_above = |h: &DocOffset| match order(&h.anchor).cmp(&order(&line.anchor)) {
        std::cmp::Ordering::Less => true,
        std::cmp::Ordering::Equal => h.intra_precise <= line.intra_precise + 0.5,
        std::cmp::Ordering::Greater => false,
    };
    let indices: Vec<usize> = (0..count).collect();
    let above = indices.partition_point(|&i| point(i).is_some_and(|h| at_or_above(&h)));
    above.checked_sub(1)
}

/// Build a [`Reveal`] spanning a Grapheme range — works for a single
/// cursor (`(g, g)`), a multi-line selection, or a find match that
/// crosses a wrap boundary. Each endpoint independently prefers a
/// fragment-derived intra-row position so `Align::Center` lands the
/// span at viewport center and `Align::Nearest` knows precisely which
/// screen lines need to be in view; falls back to a row-anchor when
/// no fragment exists (off-viewport block not yet shaped). The range
/// is reordered to `(min, max)` so callers can pass raw `(anchor,
/// cursor)` selections.
pub(crate) fn build_target_reveal(
    renderer: &MdRender, content: &scroll_content::DocScrollContent<'_, '_>,
    state: &crate::widgets::affine_scroll::ScrollArea<DocRowId>, range: (Grapheme, Grapheme),
    canvas_rect: Rect, pad: f32,
) -> Option<Reveal<DocRowId>> {
    let (start, end) = if range.0 <= range.1 { range } else { (range.1, range.0) };
    let top =
        endpoint_offset(renderer, content, state, start, canvas_rect, EndpointSide::Top, pad)?;
    let bottom =
        endpoint_offset(renderer, content, state, end, canvas_rect, EndpointSide::Bottom, pad)?;
    Some(Reveal { top, bottom })
}

#[derive(Clone, Copy)]
enum EndpointSide {
    Top,
    Bottom,
}

/// Height of the fixed overlay covering the top of the editor (Android
/// status-bar safe area); zero elsewhere. Equals the leading pad, which
/// exists to hold content clear of that overlay.
fn android_top_overlay(content: &scroll_content::DocScrollContent<'_, '_>) -> f32 {
    if cfg!(target_os = "android") { content.leading_precise } else { 0.0 }
}

fn endpoint_offset(
    renderer: &MdRender, content: &scroll_content::DocScrollContent<'_, '_>,
    state: &crate::widgets::affine_scroll::ScrollArea<DocRowId>, target: Grapheme,
    canvas_rect: Rect, side: EndpointSide, pad: f32,
) -> Option<Offset<DocRowId>> {
    use crate::widgets::affine_scroll::Rows as _;
    let frag = renderer.fragment_at_offset(target).or_else(|| {
        // a table row's pipes aren't drawn; its last cell stands in for them
        let table = renderer.bounds.table_at(target)?;
        let row = &table.rows[table.row_at(target)?];
        renderer.fragment_at_offset(row.cells.last()?.content.1)
    });
    if let Some(frag) = frag.filter(|f| !f.far) {
        let y_range = frag.rect.y_range().expand(pad);
        let y = match side {
            EndpointSide::Top => y_range.min - android_top_overlay(content),
            EndpointSide::Bottom => y_range.max,
        };
        return state.offset_at_viewport_y(content, y - canvas_rect.min.y);
    }
    let row = content.find_text_row(target)?;
    Some(match side {
        EndpointSide::Top => Offset::at_top_of(row),
        EndpointSide::Bottom => Offset { anchor: row, intra_precise: content.precise(&row) },
    })
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
    let text = crate::font::text_fonts();
    let base_scale = text.scale;
    let mono_scale = 0.9 * base_scale;
    let mono_y_offset_factor = 0.1;

    let super_y_offset_factor = -1. / 4.;
    let sub_y_offset_factor = 1. / 4.;
    let super_scale = 3. / 4.;
    let sub_scale = 3. / 4.;

    let tweaked = |font: &FontData, scale: f32, y_offset_factor: f32| {
        let mut font = font.clone();
        font.tweak.scale = scale;
        font.tweak.y_offset_factor = y_offset_factor;
        Arc::new(font)
    };
    for (name, font, scale, y_offset_factor) in [
        ("sans", &text.sans, base_scale, 0.),
        ("bold", &text.bold, base_scale, 0.),
        ("italic", &text.italic, base_scale, 0.),
        ("mono", &text.mono, mono_scale, mono_y_offset_factor),
        ("sans_super", &text.sans, super_scale * base_scale, super_y_offset_factor),
        ("bold_super", &text.bold, super_scale * base_scale, super_y_offset_factor),
        (
            "mono_super",
            &text.mono,
            super_scale * mono_scale,
            super_y_offset_factor + mono_y_offset_factor,
        ),
        ("sans_sub", &text.sans, sub_scale * base_scale, sub_y_offset_factor),
        ("bold_sub", &text.bold, sub_scale * base_scale, sub_y_offset_factor),
        (
            "mono_sub",
            &text.mono,
            sub_scale * mono_scale,
            sub_y_offset_factor + mono_y_offset_factor,
        ),
    ] {
        fonts
            .font_data
            .insert(name.into(), tweaked(font, scale, y_offset_factor));
    }

    fonts.font_data.insert("icons".into(), {
        FontData {
            tweak: FontTweak { y_offset: -0.1, scale: mono_scale, ..Default::default() },
            ..FontData::from_static(lb_fonts::NERD_FONTS_MONO_SYMBOLS)
        }
        .into()
    });

    // Phosphor UI icons — family name `"phosphor"`. No global scale tweak:
    // design chrome sizes glyphs via FontId. File **names** (emoji-capable)
    // paint via GlyphonLabel/GlyphonTextEdit, not egui proportional + Twemoji.
    fonts
        .font_data
        .insert("phosphor".into(), FontData::from_static(lb_fonts::PHOSPHOR).into());

    fonts
        .families
        .insert(FontFamily::Name(Arc::from("Bold")), vec!["bold".into()]);
    fonts
        .families
        .insert(FontFamily::Name(Arc::from("Italic")), vec!["italic".into()]);
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
        .insert(FontFamily::Name(Arc::from("phosphor")), vec!["phosphor".into()]);

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

    // Icon codepoints in labels resolve without an explicit phosphor FontId
    // (toolbar mixed Latin + PUA in one run).
    for family in [FontFamily::Proportional, FontFamily::Monospace] {
        fonts
            .families
            .get_mut(&family)
            .unwrap()
            .push("phosphor".to_owned());
    }

    crate::font::cjk::register_egui(fonts, base_scale);
}

/// Headless `Context` with the same fonts as production, including Phosphor.
/// `Context::default()` never binds `FontFamily::Name("phosphor")`, so any
/// test that shapes a toolbar/icon galley panics in epaint.
#[cfg(test)]
pub(crate) fn test_egui_ctx() -> Context {
    let ctx = Context::default();
    let mut fonts = FontDefinitions::default();
    register_fonts(&mut fonts);
    ctx.set_fonts(fonts);
    ctx
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
    use lb_rs::model::text::offset_types::Grapheme;

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
                    start: Location::Grapheme(Grapheme(start)),
                    end: Location::Grapheme(Grapheme(end)),
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
            ctx.run_pass(RawInput::default(), |ctx| {
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
            })
            .textures_delta
            .clear();
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
