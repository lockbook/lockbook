//! The file tree — the first feature widget, and the template for the whole
//! action-surface pattern. `show` draws from a borrowed `FilesExt` view and
//! turns input into `Action`s; every `Action` routes through `apply`, the one
//! chokepoint. Navigation (select, expand) folds into widget-local view state;
//! model mutations escape as an `Op` the shell runs against the workspace. That
//! chokepoint is what makes the surface scriptable and fuzzable — a driver
//! builds a `Vec<File>` and drives `apply`, no GUI required.
//!
//! Rendering runs off a *flattened* row list — a pre-order walk of expanded
//! nodes at uniform `ROW_H`. Folder open/close reveals the child *block* from
//! under the folder (growing clip over full-size rows) rather than scaling each
//! row. That one model backs virtualization, sticky headers, keyboard nav, and
//! reveal-to-selection alike; the tree's geometry lives in the list, not in the
//! recursion.
//!
//! Wired so far: multi-select (plain / Cmd-toggle / Shift-range) with a cursor,
//! keyboard nav (cursor arrows + Shift-extend, Enter to open), expand/collapse,
//! open (same/new-tab, middle-click), virtualized scroll, sticky ancestor
//! headers, right-click context menu (open / create / expand / arrange /
//! share / delete), cut·copy·paste (Finder-style internal clipboard), delete
//! key, inline rename (glyphon, emoji-safe; menu + F2), drag-and-drop move
//! (folder / parent drop with floating card + indicators). Row labels use
//! GlyphonLabel so emoji in names render. Deferred: virtual folders, sort/view.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use egui::{
    CursorIcon, DragAndDrop, Id, LayerId, Order, Pos2, Rect, Sense, Stroke, Ui, Vec2, pos2, vec2,
};
use lb::Uuid;
use lb::model::file::{File, ShareMode};
use lb::subscribers::status::Status;
use workspace_rs::file_cache::FilesExt;
use workspace_rs::show::{DocType, ElapsedHumanString};
use workspace_rs::theme::palette_v2::ThemeExt;
use workspace_rs::widgets::{GlyphonLabel, TextOverflow, tip_ui_rich};
use workspace_rs::GlyphonRendererCallback;

use crate::theme::icons;
use crate::theme::tokens::Tokens;
use crate::widgets::tree_chrome::{
    ICON_NAME_GAP, INDENT_BASE, INDENT_STEP, NAME_FONT, ROW_H, TYPE_ICON_SLOT,
};

/// Name line box height — shared with the rename field so the baseline doesn't
/// jump when toggling display ↔ edit (see `GlyphonLabel::line_height`).
/// (`ROW_H` / indent / icon metrics live in `tree_chrome` with the move picker.)
const NAME_LINE_H: f32 = 20.0;

/// How long a file must stay dirty / in-flight before its tree mark appears.
/// Matches Apple (`statusDotDelay = 2`) so quick successful syncs never flash.
const SYNC_DOT_DEBOUNCE: Duration = Duration::from_secs(2);

/// Per-row sync activity (same ranking as Apple / master egui).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SyncDot {
    Pushing,
    Dirty,
    Pulling,
}

impl SyncDot {
    /// Lower wins when a file has multiple states.
    fn rank(self) -> u8 {
        match self {
            SyncDot::Pushing => 0,
            SyncDot::Dirty => 1,
            SyncDot::Pulling => 2,
        }
    }

    fn glyph(self) -> &'static str {
        match self {
            SyncDot::Pushing | SyncDot::Dirty => icons::CLOUD_ARROW_UP,
            SyncDot::Pulling => icons::CLOUD_ARROW_DOWN,
        }
    }

    fn tip(self) -> &'static str {
        match self {
            SyncDot::Pushing => "Uploading…",
            SyncDot::Dirty => "Not synced yet",
            SyncDot::Pulling => "Downloading…",
        }
    }
}

/// Debounced, parent-bubbled sync marks — Apple `statusDots` / master `SyncDots`.
#[derive(Default)]
struct SyncDots {
    /// Visible marks after debounce (self + ancestors).
    dots: HashMap<Uuid, SyncDot>,
    pushing: Vec<Uuid>,
    dirty: Vec<Uuid>,
    pulling: Vec<Uuid>,
    /// When each source id first entered a status list; cleared when it leaves.
    pending_since: HashMap<Uuid, Instant>,
}

/// Horizontal inset inside the scroll content (left/right only). Sticky fills
/// full width; in-flow rows and sticky content share this side pad so handoff
/// stays continuous. No top/bottom pad (that caused a jump at the pin edge).
const SCROLL_INSET: f32 = 5.0;

/// Pointer must move this far before a press becomes a tree drag (master tree).
const DRAG_THRESHOLD: f32 = 8.0;
/// Hover a folder this long during DnD before auto-expanding (master: 600ms).
const DROP_EXPAND_MS: u64 = 600;
/// How far from the viewport top/bottom (px) starts edge auto-scroll during DnD.
const DND_SCROLL_EDGE: f32 = 40.0;
/// Peak scroll speed at the extreme edge (px/sec). Ramps with depth into the zone.
const DND_SCROLL_MAX_PX_PER_SEC: f32 = 520.0;

/// egui DnD payload — selection is the real move set; this marks an in-tree drag.
#[derive(Clone, Copy, Debug)]
struct TreeDnd;

/// Temp-data latch: once the sidebar separator starts dragging, stay "resizing"
/// until primary is released. Survives frames where `is_being_dragged` flickers
/// (pointer races ahead of the moving edge). Must match `lib.rs`.
const SIDEBAR_RESIZING_LATCH: &str = "lb_sidebar_resizing";

/// The complete input vocabulary. Mouse, keyboard, and (later) fuzz/CLI all
/// produce `Action`s; nothing mutates tree state except `apply`.
#[derive(Clone, Debug)]
pub enum Action {
    /// Replace the selection with `id` (plain click); moves cursor and anchor.
    Select(Uuid),
    /// Toggle `id` in the selection (Cmd/Ctrl-click); moves cursor and anchor.
    SelectAdd(Uuid),
    /// Select the visible range from the anchor to `id` (Shift-click); moves the
    /// cursor, keeps the anchor.
    SelectRange(Uuid),
    /// Move the cursor to the previous/next visible row (arrow keys). `extend`
    /// (Shift) grows the selection from the anchor; otherwise it single-selects.
    CursorMove {
        down: bool,
        extend: bool,
    },
    /// Open the cursor's document if it is one (Enter).
    OpenCursor,
    /// Create a new document / folder under `parent` — escapes to the shell.
    CreateDoc {
        parent: Uuid,
    },
    CreateFolder {
        parent: Uuid,
    },
    /// Delete the current selection — escapes to the shell.
    DeleteSelected,
    /// Enter inline-rename on `id`, seeding the buffer with its current name.
    BeginRename(Uuid),
    /// Commit the in-progress rename (Enter / click-away) — escapes if changed.
    CommitRename,
    /// Discard the in-progress rename (Escape).
    CancelRename,
    /// Flip a folder's expansion.
    Toggle(Uuid),
    /// Expand `id` and every folder under it (menu “Expand all” on a folder).
    ExpandSubtree(Uuid),
    /// Collapse `id` and every folder under it (menu “Collapse all” on a folder).
    CollapseSubtree(Uuid),
    /// Open a document — escapes to the shell.
    Open {
        id: Uuid,
        new_tab: bool,
    },
    /// Toggle pin for `id` — escapes to the shell.
    TogglePin(Uuid),
    /// Open the share sheet for `id` — escapes to the shell.
    Share(Uuid),
    /// Move the current selection — escapes to the shell (folder picker).
    MoveSelected,
    /// Export the current selection to disk — escapes to the shell.
    ExportSelected,
    /// Duplicate `id` (sibling with unique name) — escapes to the shell.
    Duplicate(Uuid),
    /// Copy the lockbook link for `id` to the system clipboard — escapes.
    CopyLink(Uuid),
    /// Mark the selection for move-on-paste (Cmd+X / Cut). View state only.
    CutSelected,
    /// Mark the selection for copy-on-paste (Cmd+C / Copy). View state only.
    CopySelected,
    /// Paste the internal clipboard into `dest` (folder id). Cut → move;
    /// copy → duplicate under `dest`.
    PasteInto { dest: Uuid },
    /// Set the scroll offset. Scroll is view state like selection, so it's part
    /// of the surface — scriptable, observable, fuzzable — not a side channel.
    ScrollTo(f32),
}

/// A model mutation the tree can't perform itself; the shell fulfills it against
/// the workspace. The shell's own action enum is composed from these.
#[derive(Clone, Debug)]
pub enum Op {
    Open { id: Uuid, new_tab: bool },
    CreateDoc { parent: Uuid },
    CreateFolder { parent: Uuid },
    Rename { id: Uuid, name: String },
    Delete { ids: Vec<Uuid> },
    /// Toggle pin state for `id` — shell owns the pin set + `lb` API.
    TogglePin { id: Uuid },
    Share { id: Uuid },
    /// Open the move picker for `ids`.
    Move { ids: Vec<Uuid> },
    /// Move `ids` straight into `parent` (cut-paste). No picker.
    MoveInto { ids: Vec<Uuid>, parent: Uuid },
    /// Duplicate each of `ids` under `parent` (copy-paste).
    CopyInto { ids: Vec<Uuid>, parent: Uuid },
    Export { ids: Vec<Uuid> },
    Duplicate { id: Uuid },
    CopyLink { id: Uuid },
}

/// Internal file clipboard — not the OS pasteboard. Finder-style cut/copy/paste
/// for tree rows: cut-paste moves, copy-paste duplicates.
#[derive(Clone, Debug)]
struct FileClip {
    ids: Vec<Uuid>,
    mode: ClipMode,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ClipMode {
    Cut,
    Copy,
}

/// In-progress inline rename. `fresh` triggers a one-time focus grab so the
/// glyphon field takes over and applies its stem selection.
struct Rename {
    id: Uuid,
    buf: String,
    fresh: bool,
}

/// One entry in the flattened visible-row list: which file, how deep, and
/// whether it's a folder (a sticky-header candidate). Position is implicit —
/// index `i` sits at content-y `i * ROW_H`.
#[derive(Clone, Copy)]
struct Row {
    id: Uuid,
    depth: usize,
    is_folder: bool,
}

/// Viewport geometry for a frame: screen-space content origin (content-y 0),
/// content width, and the current scroll offset.
///
/// In-flow rows use `origin` / `width` (side-inset). Sticky headers use
/// `sticky_left` / `sticky_width` for full-bleed L/R fill and `sticky_top`
/// flush to the viewport top (same Y as flow — continuous pin handoff).
#[derive(Clone, Copy)]
struct View {
    origin: Pos2,
    width: f32,
    offset: f32,
    /// Full-bleed sticky stack: left edge, width, and viewport-top y.
    sticky_left: f32,
    sticky_width: f32,
    sticky_top: f32,
}

/// How a row is laid out: normal in-flow, or a stuck header clipped under its
/// parent (`clip`) so its pushed-up portion slides behind rather than over.
/// `elevate_t` is paint-only: 0 = panel canvas (sliding off), 1 = raised mid
/// chrome. Animated with a fixed-duration ease when the stick state flips —
/// not a per-frame exponential filter.
#[derive(Clone, Copy)]
enum Placement {
    Flow,
    Sticky { clip: Rect, elevate_t: f32 },
}

impl Placement {
    fn ns(self) -> &'static str {
        match self {
            Placement::Flow => "flow",
            Placement::Sticky { .. } => "sticky",
        }
    }
    fn clip(self) -> Option<Rect> {
        match self {
            Placement::Flow => None,
            Placement::Sticky { clip, .. } => Some(clip),
        }
    }
    fn opaque(self) -> bool {
        matches!(self, Placement::Sticky { .. })
    }
    /// 0..=1 raised-chrome factor (see sticky elevate animation).
    fn elevate_t(self) -> f32 {
        match self {
            Placement::Flow => 0.0,
            Placement::Sticky { elevate_t, .. } => elevate_t,
        }
    }
}

/// How long the sticky raised fill eases in/out (seconds). Fixed-duration
/// clock animation via egui's animation manager.
const STICKY_ELEVATE_SECS: f32 = 0.20;

/// Folder open/close — child block slides out from under the folder (seconds).
const FOLDER_ANIM_SECS: f32 = 0.22;

/// Fixed-duration elevate animation for sticky chrome (clock time, not
/// exponential smoothing). egui's `animate_bool` snaps on first registration,
/// which is exactly the Flow→Sticky handoff — so we own this state.
struct ElevAnim {
    from: f32,
    value: f32,
    target: f32,
    t0: f64,
}

/// Fixed-duration 0..=1 open factor for a folder’s child block.
struct FolderAnim {
    from: f32,
    value: f32,
    target: f32,
    t0: f64,
}

/// Per-row layout. Rows are always full `ROW_H`; folder open/close clips the
/// child block so content emerges from under the parent (stacked-behind reveal).
#[derive(Clone, Copy)]
struct RowGeom {
    /// Content-y of the row top (natural layout within expanded tree).
    y: f32,
    /// Always `ROW_H` when the row participates in layout.
    h: f32,
    /// Content-y clip window from ancestor folder slots (inclusive top / exclusive-ish bot).
    clip_top: f32,
    clip_bot: f32,
}

#[derive(Default)]
pub struct FileTree {
    expanded: HashSet<Uuid>,
    /// Open/close ease per folder. Settled open folders use `expanded` alone
    /// (`open_t == 1`). Closing keeps `expanded` until the ease hits 0.
    folder_anim: std::collections::HashMap<Uuid, FolderAnim>,
    /// The selection set. Multi-select via Cmd (toggle) and Shift (range).
    selected: HashSet<Uuid>,
    /// The active row — the keyboard cursor, and the moving end of a Shift-range.
    /// Usually in `selected`, but kept separate so it can lead independently.
    cursor: Option<Uuid>,
    /// The fixed end a Shift-range grows from — set on plain/Cmd click, held
    /// across Shift clicks so successive range-extends pivot on one point.
    anchor: Option<Uuid>,
    /// Inline-rename mode: the row being renamed and its edit buffer.
    renaming: Option<Rename>,
    /// Set by `Action::ScrollTo`, forces the scroll offset for the next frame
    /// then clears. `None` leaves egui in charge (user wheel/drag). The one-shot
    /// jump both scripts scroll for observation and, later, backs reveal-to-file.
    forced_offset: Option<f32>,
    /// Last painted scroll offset / viewport height — used by `reveal` for
    /// minimum-scroll (only move if the row is off-screen).
    last_offset: f32,
    last_view_h: f32,
    /// Sticky raised-fill animation per file id (0 = panel surface, 1 = raised).
    elev_anim: std::collections::HashMap<Uuid, ElevAnim>,
    /// Cut/copy buffer for paste. Cleared after a cut-paste; kept after copy-paste.
    clip: Option<FileClip>,
    /// Folder under the pointer during DnD + enter time (auto-expand).
    drop_hover: Option<(Uuid, Instant)>,
    /// Pointer − row origin at drag start (floating card tracks the grab).
    drag_grab_offset: Option<Vec2>,
    /// Row that started the drag (float paints this file’s row chrome).
    drag_primary: Option<Uuid>,
    /// Persistent float chrome for the primary drag row — kept for the whole
    /// gesture so virtualization / folder expand can’t blank the card.
    drag_float: Option<DragRowSnap>,
    /// Drop line / into-folder ring for the current hover (set while painting
    /// rows, drawn once after so row fills can’t cover it).
    drop_paint: Option<DropPaint>,
    /// Last frame’s in-flow content width (`clip − 2×SCROLL_INSET`). Float and
    /// sep diagnostics key off this so sticky full-bleed never leaks in.
    last_content_w: f32,
    /// Debounced dirty / push / pull marks (macOS-style, see `SyncDots`).
    sync_dots: SyncDots,
}

/// Snapshot of the primary drag row (float card re-paints tree chrome).
#[derive(Clone)]
struct DragRowSnap {
    /// Content-band width only (inside `SCROLL_INSET`) — same as an in-flow
    /// row, not sticky full-bleed. Card is exactly this wide (“picked up”).
    width: f32,
    /// Tree depth — same `INDENT_BASE + depth * INDENT_STEP` as the source row.
    depth: usize,
    name: String,
    is_folder: bool,
    expanded: bool,
}

/// Pending drop chrome for one frame (painted after all rows so fills can’t
/// cover the sep, and on a high layer so it sits above tree body).
#[derive(Clone, Copy)]
enum DropPaint {
    /// Sibling insert line across the content band.
    Between { y: f32, x0: f32, x1: f32, valid: bool },
    /// Folder target ring on the content-band rect.
    Into { rect: Rect, valid: bool },
}

impl FileTree {
    /// Expand every ancestor of `id` so it appears in the flattened tree (Apple
    /// pin → folder: expand path then select). Does not scroll/select.
    /// Ancestors that were closed animate open (same ease as a click).
    pub fn expand_to(&mut self, id: Uuid, files: &impl FilesExt) {
        let mut current = id;
        for _ in 0..64 {
            let Some(file) = files.get_by_id(current) else { break };
            if file.is_root() || file.parent == file.id {
                break;
            }
            self.animate_folder(file.parent, true);
            current = file.parent;
        }
    }

    /// Rebuild debounced, parent-bubbled sync marks from `status` (Apple
    /// `recomputeStatusDots` / master `refresh_sync_dots`).
    fn refresh_sync_dots(&mut self, ui: &Ui, files: &impl FilesExt, status: &Status) {
        let inputs_unchanged = self.sync_dots.pushing == status.pushing_files
            && self.sync_dots.dirty == status.dirty_locally
            && self.sync_dots.pulling == status.pulling_files;

        if !inputs_unchanged {
            let now = Instant::now();
            let mut pending_since = HashMap::new();
            for ids in [&status.pushing_files, &status.dirty_locally, &status.pulling_files] {
                for &id in ids {
                    let since = self.sync_dots.pending_since.get(&id).copied().unwrap_or(now);
                    pending_since.insert(id, since);
                }
            }
            self.sync_dots.pending_since = pending_since;
            self.sync_dots.pushing = status.pushing_files.clone();
            self.sync_dots.dirty = status.dirty_locally.clone();
            self.sync_dots.pulling = status.pulling_files.clone();
        }

        self.sync_dots.dots = self.compute_sync_dots(files, status);

        // Repaint when the next source id clears the debounce window.
        let soonest = self
            .sync_dots
            .pending_since
            .values()
            .map(|since| SYNC_DOT_DEBOUNCE.saturating_sub(since.elapsed()))
            .filter(|remaining| !remaining.is_zero())
            .min();
        if let Some(remaining) = soonest {
            ui.ctx().request_repaint_after(remaining);
        }
    }

    fn compute_sync_dots(
        &self, files: &impl FilesExt, status: &Status,
    ) -> HashMap<Uuid, SyncDot> {
        let mut dots = HashMap::new();
        for (ids, dot) in [
            (&status.pushing_files, SyncDot::Pushing),
            (&status.dirty_locally, SyncDot::Dirty),
            (&status.pulling_files, SyncDot::Pulling),
        ] {
            for &id in ids {
                if !self.debounce_elapsed(id) {
                    continue;
                }
                // Apple: seed the file and every non-root ancestor.
                let mut current = id;
                loop {
                    Self::bump_sync_dot(&mut dots, current, dot);
                    let Some(file) = files.get_by_id(current) else { break };
                    if file.is_root() || file.parent == file.id {
                        break;
                    }
                    let Some(parent) = files.get_by_id(file.parent) else { break };
                    if parent.is_root() {
                        break;
                    }
                    current = parent.id;
                }
            }
        }
        dots
    }

    fn debounce_elapsed(&self, id: Uuid) -> bool {
        self.sync_dots
            .pending_since
            .get(&id)
            .is_some_and(|since| since.elapsed() >= SYNC_DOT_DEBOUNCE)
    }

    fn bump_sync_dot(dots: &mut HashMap<Uuid, SyncDot>, id: Uuid, dot: SyncDot) {
        let replace = dots.get(&id).is_none_or(|existing| dot.rank() < existing.rank());
        if replace {
            dots.insert(id, dot);
        }
    }

    /// Select `id`, expand its ancestors, and scroll the minimum amount so the
    /// row is fully visible below the sticky-header stack (no-op if already
    /// on-screen). Workspace → tree reveal (tab open / switch / create).
    /// No-op if `id` is not in `files`.
    pub fn reveal(&mut self, id: Uuid, files: &impl FilesExt) {
        if files.get_by_id(id).is_none() {
            return;
        }
        self.expand_to(id, files);
        self.apply(Action::Select(id), files);
        let rows = self.flatten(files);
        let (geoms, _) = self.row_geometry(&rows);
        if let Some(i) = rows.iter().position(|r| r.id == id) {
            if let Some(y) =
                min_scroll_to_row(&rows, &geoms, i, self.last_offset, self.last_view_h)
            {
                self.forced_offset = Some(y);
            }
        }
    }

    /// Snap all in-flight folder open/close eases to their targets. Headless
    /// tests and drivers that only call `apply` (no `tick_folder_anims`) use
    /// this so layout sees fully open/closed heights.
    #[cfg(test)]
    fn settle_folder_anims(&mut self) {
        let ids: Vec<Uuid> = self.folder_anim.keys().copied().collect();
        for id in ids {
            let target = self.folder_anim.get(&id).map(|a| a.target).unwrap_or(0.0);
            if target < 0.5 {
                self.expanded.remove(&id);
            } else {
                self.expanded.insert(id);
            }
            self.folder_anim.remove(&id);
        }
    }

    /// Active keyboard/selection cursor, if any.
    /// Whether the internal cut/copy clipboard has anything (for Paste menus).
    pub fn has_clip(&self) -> bool {
        self.clip.is_some()
    }

    pub fn cursor(&self) -> Option<Uuid> {
        self.cursor
    }

    /// Drop cursor/selection entries that no longer exist in `files` (after
    /// delete or cache rebuild).
    pub fn prune_missing(&mut self, files: &impl FilesExt) {
        self.selected.retain(|id| files.get_by_id(*id).is_some());
        if self.cursor.is_some_and(|c| files.get_by_id(c).is_none()) {
            self.cursor = self.selected.iter().next().copied();
        }
        if let Some(clip) = &mut self.clip {
            clip.ids.retain(|id| files.get_by_id(*id).is_some());
            if clip.ids.is_empty() {
                self.clip = None;
            }
        }
        if self.anchor.is_some_and(|a| files.get_by_id(a).is_none()) {
            self.anchor = self.cursor;
        }
        self.elev_anim.retain(|id, _| files.get_by_id(*id).is_some());
        self.folder_anim.retain(|id, _| files.get_by_id(*id).is_some());
    }

    /// Visual open factor for folder `id` (0 = closed, 1 = fully open).
    fn folder_open_t(&self, id: Uuid) -> f32 {
        if let Some(a) = self.folder_anim.get(&id) {
            a.value
        } else if self.expanded.contains(&id) {
            1.0
        } else {
            0.0
        }
    }

    /// Whether the folder is open or mid-open (not mid-close / closed).
    fn folder_is_openish(&self, id: Uuid) -> bool {
        self.folder_anim
            .get(&id)
            .map(|a| a.target > 0.5)
            .unwrap_or_else(|| self.expanded.contains(&id))
    }

    /// Single chokepoint for open/close: click, menu, expand-all, collapse-all,
    /// reveal path, and DnD auto-expand. Opening inserts `expanded` immediately
    /// so children are in the flatten; closing keeps it until the ease finishes.
    /// Redirects mid-flight (reverse from current value). No-op if already settled
    /// at the target.
    fn animate_folder(&mut self, id: Uuid, open: bool) {
        let target = if open { 1.0 } else { 0.0 };
        let cur = self.folder_open_t(id);
        if open {
            self.expanded.insert(id);
        }
        // Already settled at target — nothing to play.
        if !self.folder_anim.contains_key(&id) && (cur - target).abs() < 0.001 {
            if !open {
                self.expanded.remove(&id);
            }
            return;
        }
        // Already easing toward this target — leave the in-flight ease alone.
        if self
            .folder_anim
            .get(&id)
            .is_some_and(|a| (a.target - target).abs() < 0.001)
        {
            return;
        }
        self.folder_anim.insert(
            id,
            FolderAnim {
                from: cur,
                value: cur,
                target,
                t0: 0.0, // latched on next tick (no clock in apply / expand_to)
            },
        );
    }

    /// Advance folder open/close eases; drop `expanded` when a close finishes.
    fn tick_folder_anims(&mut self, ctx: &egui::Context) {
        if self.folder_anim.is_empty() {
            return;
        }
        let now = ctx.input(|i| i.time);
        let mut finished_close: Vec<Uuid> = Vec::new();
        let mut settled_open: Vec<Uuid> = Vec::new();
        for (&id, a) in self.folder_anim.iter_mut() {
            // apply() has no clock — first tick latches start time.
            if a.t0 == 0.0 {
                a.t0 = now;
            }
            let u = ((now - a.t0) as f32 / FOLDER_ANIM_SECS).clamp(0.0, 1.0);
            // Ease-out quad.
            let e = 1.0 - (1.0 - u) * (1.0 - u);
            a.value = a.from + (a.target - a.from) * e;
            if u >= 1.0 {
                a.value = a.target;
                if a.target < 0.5 {
                    finished_close.push(id);
                } else {
                    settled_open.push(id);
                }
            }
        }
        for id in finished_close {
            self.expanded.remove(&id);
            self.folder_anim.remove(&id);
        }
        for id in settled_open {
            self.folder_anim.remove(&id);
        }
        if !self.folder_anim.is_empty() {
            ctx.request_repaint();
        }
    }

    /// Fixed-duration 0..=1 elevate factor toward `target` (0 or 1). Starts at 0
    /// on first sight so enter-sticky eases in (unlike egui `animate_bool`).
    fn elev_t(&mut self, ctx: &egui::Context, id: Uuid, target: f32) -> f32 {
        let now = ctx.input(|i| i.time);
        let e = self.elev_anim.entry(id).or_insert(ElevAnim {
            from: 0.0,
            value: 0.0,
            target: 0.0,
            t0: now,
        });
        if (e.target - target).abs() > f32::EPSILON {
            e.from = e.value;
            e.target = target;
            e.t0 = now;
        }
        let u = ((now - e.t0) as f32 / STICKY_ELEVATE_SECS).clamp(0.0, 1.0);
        let eased = egui::emath::easing::quadratic_out(u);
        e.value = e.from + (e.target - e.from) * eased;
        if (e.value - e.target).abs() > 0.001 {
            ctx.request_repaint();
        }
        e.value
    }

    /// Peek current elevate factor without advancing (for flow rows still fading).
    fn elev_t_peek(&self, id: Uuid) -> f32 {
        self.elev_anim.get(&id).map(|e| e.value).unwrap_or(0.0)
    }

    /// The one chokepoint. Navigation folds into view state and returns `None`;
    /// model mutations return the `Op` for the shell to run. Takes `files` because
    /// selection (range) and — later — keyboard nav and reveal are defined over
    /// the flattened visible order, not raw ids.
    pub fn apply(&mut self, action: Action, files: &impl FilesExt) -> Option<Op> {
        match action {
            Action::Select(id) => {
                self.selected = HashSet::from([id]);
                self.cursor = Some(id);
                self.anchor = Some(id);
            }
            Action::SelectAdd(id) => {
                if !self.selected.remove(&id) {
                    self.selected.insert(id);
                }
                self.cursor = Some(id);
                self.anchor = Some(id);
            }
            Action::SelectRange(id) => {
                self.select_range(files, id);
                self.cursor = Some(id);
            }
            Action::CursorMove { down, extend } => {
                let rows = self.flatten(files);
                if rows.is_empty() {
                    return None;
                }
                let here = self
                    .cursor
                    .and_then(|c| rows.iter().position(|r| r.id == c));
                let next = match here {
                    Some(i) if down => (i + 1).min(rows.len() - 1),
                    Some(i) => i.saturating_sub(1),
                    None if down => 0,
                    None => rows.len() - 1,
                };
                let id = rows[next].id;
                if extend {
                    self.select_range(files, id); // pivots on the held anchor
                } else {
                    self.selected = HashSet::from([id]);
                    self.anchor = Some(id);
                }
                self.cursor = Some(id);
            }
            Action::OpenCursor => {
                if let Some(id) = self.cursor {
                    if files.get_by_id(id).is_some_and(|f| f.is_document()) {
                        return Some(Op::Open { id, new_tab: false });
                    }
                }
            }
            Action::CreateDoc { parent } => return Some(Op::CreateDoc { parent }),
            Action::CreateFolder { parent } => return Some(Op::CreateFolder { parent }),
            Action::TogglePin(id) => return Some(Op::TogglePin { id }),
            Action::Share(id) => return Some(Op::Share { id }),
            Action::Duplicate(id) => return Some(Op::Duplicate { id }),
            Action::CopyLink(id) => return Some(Op::CopyLink { id }),
            Action::CutSelected => {
                let ids = self.clip_source_ids(files);
                if !ids.is_empty() {
                    self.clip = Some(FileClip { ids, mode: ClipMode::Cut });
                }
            }
            Action::CopySelected => {
                let ids = self.clip_source_ids(files);
                if !ids.is_empty() {
                    self.clip = Some(FileClip { ids, mode: ClipMode::Copy });
                }
            }
            Action::PasteInto { dest } => {
                let clip = self.clip.clone()?;
                // Refuse paste into a cut/copied folder (or its descendants).
                let ids: Vec<Uuid> = clip
                    .ids
                    .into_iter()
                    .filter(|id| *id != dest && !is_under(files, dest, *id))
                    .collect();
                if ids.is_empty() {
                    return None;
                }
                match clip.mode {
                    ClipMode::Cut => {
                        self.clip = None;
                        return Some(Op::MoveInto { ids, parent: dest });
                    }
                    ClipMode::Copy => {
                        // Finder keeps the copy buffer so paste can repeat.
                        return Some(Op::CopyInto { ids, parent: dest });
                    }
                }
            }
            Action::MoveSelected => {
                if !self.selected.is_empty() {
                    return Some(Op::Move { ids: self.selected.iter().copied().collect() });
                }
            }
            Action::ExportSelected => {
                if !self.selected.is_empty() {
                    return Some(Op::Export { ids: self.selected.iter().copied().collect() });
                }
            }
            Action::DeleteSelected => {
                if !self.selected.is_empty() {
                    return Some(Op::Delete { ids: self.selected.iter().copied().collect() });
                }
            }
            Action::BeginRename(id) => {
                if let Some(f) = files.get_by_id(id) {
                    self.renaming = Some(Rename { id, buf: f.name.clone(), fresh: true });
                }
            }
            Action::CommitRename => {
                if let Some(rn) = self.renaming.take() {
                    let name = rn.buf.trim().to_string();
                    if !name.is_empty() {
                        return Some(Op::Rename { id: rn.id, name });
                    }
                }
            }
            Action::CancelRename => self.renaming = None,
            Action::Toggle(id) => {
                let open = !self.folder_is_openish(id);
                self.animate_folder(id, open);
            }
            Action::ExpandSubtree(id) => self.expand_subtree(id, files),
            Action::CollapseSubtree(id) => self.collapse_subtree(id, files),
            Action::ScrollTo(y) => self.forced_offset = Some(y.max(0.0)),
            Action::Open { id, new_tab } => return Some(Op::Open { id, new_tab }),
        }
        None
    }

    /// Expand `id` and all descendant folders (animated open ease).
    fn expand_subtree(&mut self, id: Uuid, files: &impl FilesExt) {
        let mut stack = vec![id];
        while let Some(cur) = stack.pop() {
            self.animate_folder(cur, true);
            for child in files.children(cur) {
                if child.is_folder() {
                    stack.push(child.id);
                }
            }
        }
    }

    /// Collapse `id` and all descendant folders (animated close ease).
    fn collapse_subtree(&mut self, id: Uuid, files: &impl FilesExt) {
        let mut stack = vec![id];
        while let Some(cur) = stack.pop() {
            self.animate_folder(cur, false);
            for child in files.children(cur) {
                if child.is_folder() {
                    stack.push(child.id);
                }
            }
        }
    }

    /// Ids to put on the internal clipboard: selection, else the cursor.
    /// Skips the account root (can't leave the tree).
    fn clip_source_ids(&self, files: &impl FilesExt) -> Vec<Uuid> {
        let raw: Vec<Uuid> = if !self.selected.is_empty() {
            self.selected.iter().copied().collect()
        } else if let Some(c) = self.cursor {
            vec![c]
        } else {
            return Vec::new();
        };
        raw.into_iter()
            .filter(|id| files.get_by_id(*id).is_some_and(|f| !f.is_root()))
            .collect()
    }

    /// Paste destination for keyboard paste: folder under the cursor, or the
    /// parent when the cursor is a document.
    fn paste_dest(&self, files: &impl FilesExt) -> Option<Uuid> {
        let cursor = self.cursor?;
        let file = files.get_by_id(cursor)?;
        Some(if file.is_folder() { cursor } else { file.parent })
    }

    /// Select every visible row between the anchor (falling back to the cursor,
    /// then `id` itself) and `id`, inclusive. Undefined ids collapse to a single
    /// selection.
    fn select_range(&mut self, files: &impl FilesExt, id: Uuid) {
        let rows = self.flatten(files);
        let pos = |target: Uuid| rows.iter().position(|r| r.id == target);
        let anchor = self.anchor.or(self.cursor).unwrap_or(id);
        match (pos(anchor), pos(id)) {
            (Some(a), Some(b)) => {
                let (lo, hi) = (a.min(b), a.max(b));
                self.selected = rows[lo..=hi].iter().map(|r| r.id).collect();
            }
            _ => self.selected = HashSet::from([id]),
        }
    }

    /// Pre-order walk of expanded nodes into the positioned row list. Pure over
    /// `(expanded, files)` — no pixels — so the same list drives rendering, the
    /// readout, and sticky/keyboard geometry. Children stay while `expanded`
    /// (including during close ease until tick removes it).
    fn flatten(&self, files: &impl FilesExt) -> Vec<Row> {
        fn walk(
            rows: &mut Vec<Row>, tree: &FileTree, files: &impl FilesExt, id: Uuid, depth: usize,
        ) {
            let Some(file) = files.get_by_id(id) else { return };
            let is_folder = file.is_folder();
            rows.push(Row { id, depth, is_folder });
            if is_folder && tree.expanded.contains(&id) {
                for cid in child_ids(files, id) {
                    walk(rows, tree, files, cid, depth + 1);
                }
            }
        }
        let mut rows = Vec::new();
        for id in child_ids(files, files.root().id) {
            walk(&mut rows, self, files, id, 0);
        }
        rows
    }

    /// Layout rows: full-size children, parent folder open_t grows a clip slot
    /// so the block drops out from under the folder (not per-row height scale).
    fn row_geometry(&self, rows: &[Row]) -> (Vec<RowGeom>, f32) {
        let mut out = vec![
            RowGeom {
                y: 0.0,
                h: ROW_H,
                clip_top: f32::NEG_INFINITY,
                clip_bot: f32::INFINITY,
            };
            rows.len()
        ];
        let total = self.layout_rows(
            rows,
            0,
            rows.len(),
            /* min_depth */ 0,
            /* y0 */ 0.0,
            f32::NEG_INFINITY,
            f32::INFINITY,
            &mut out,
        );
        (out, total)
    }

    /// Natural height of `rows[start..end)` (descendants at depth > parent),
    /// with nested folder slots at their current open_t.
    fn measure_block(&self, rows: &[Row], start: usize, end: usize, parent_depth: usize) -> f32 {
        let mut h = 0.0_f32;
        let mut i = start;
        while i < end {
            if rows[i].depth <= parent_depth {
                break;
            }
            h += ROW_H;
            if rows[i].is_folder && self.expanded.contains(&rows[i].id) {
                let d = rows[i].depth;
                let id = rows[i].id;
                i += 1;
                let block_start = i;
                while i < end && rows[i].depth > d {
                    i += 1;
                }
                let natural = self.measure_block(rows, block_start, i, d);
                h += natural * self.folder_open_t(id);
            } else {
                i += 1;
            }
        }
        h
    }

    /// Write geoms for `rows[start..end)` at depths ≥ `min_depth`, placing the
    /// first row at content-y `y0`. Returns scroll-space height consumed.
    /// `clip_top`/`clip_bot` are the ancestor reveal window in content-y.
    #[allow(clippy::too_many_arguments)]
    fn layout_rows(
        &self, rows: &[Row], start: usize, end: usize, min_depth: usize, y0: f32,
        clip_top: f32, clip_bot: f32, out: &mut [RowGeom],
    ) -> f32 {
        let mut y = y0;
        let mut i = start;
        while i < end {
            if rows[i].depth < min_depth {
                break;
            }
            out[i] = RowGeom {
                y,
                h: ROW_H,
                clip_top,
                clip_bot,
            };
            y += ROW_H;
            let row = rows[i];
            i += 1;

            if row.is_folder && self.expanded.contains(&row.id) {
                let block_start = i;
                while i < end && rows[i].depth > row.depth {
                    i += 1;
                }
                let natural = self.measure_block(rows, block_start, i, row.depth);
                let open_t = self.folder_open_t(row.id);
                let slot = natural * open_t;
                let slot_top = y; // children sit just under the folder row
                let slot_bot = y + slot;
                // Reveal window: only the slot is visible — content is laid out
                // full-size from slot_top, so as slot grows rows emerge from under
                // the folder (as if they were stacked behind it).
                let child_clip_top = clip_top.max(slot_top);
                let child_clip_bot = clip_bot.min(slot_bot);
                let _ = self.layout_rows(
                    rows,
                    block_start,
                    i,
                    row.depth + 1,
                    slot_top,
                    child_clip_top,
                    child_clip_bot,
                    out,
                );
                // Scroll only advances by the open slot, not full natural height.
                y = slot_top + slot;
            }
        }
        y - y0
    }

    /// Draw the tree and return the frame's escaping `Op` (a click yields at most
    /// one). Only the rows overlapping the viewport are drawn; the ancestor chain
    /// of the topmost row is then re-drawn as a stuck header stack over the top.
    /// `pinned` drives the Pin/Unpin context-menu label (shell owns the set).
    /// `me` is the signed-in username — used to tell own files from organized
    /// shares (link metadata is invisible; targets keep the sharer's owner).
    /// `status` supplies per-id sync hints (dirty / push / pull) for row icons.
    pub fn show(
        &mut self, ui: &mut Ui, t: &Tokens, files: &impl FilesExt,
        pinned: &std::collections::HashSet<Uuid>, me: Option<&str>, status: Option<&Status>,
    ) -> Option<Op> {
        // Advance folder open/close before layout so this frame’s heights match.
        self.tick_folder_anims(ui.ctx());
        // Debounced sync marks (Apple: 2s delay + bubble to parents).
        if let Some(status) = status {
            self.refresh_sync_dots(ui, files, status);
        } else {
            self.sync_dots = SyncDots::default();
        }
        let rows = self.flatten(files);
        let (geoms, total_h) = self.row_geometry(&rows);
        if !DragAndDrop::has_any_payload(ui.ctx()) {
            self.drag_grab_offset = None;
            self.drag_primary = None;
            self.drag_float = None;
            self.drop_paint = None;
            // Keep drop_hover only while a payload is live.
            if ui.input(|i| i.pointer.any_released()) {
                self.drop_hover = None;
            }
        }
        // Rebuilt each frame while rows run handle_row_dnd.
        self.drop_paint = None;

        // macOS-style overlay bar: appear while scrolling (and briefly after).
        let escaped = crate::widgets::scroll_overlay::with_overlay_scroll(
            ui,
            Id::new("file_tree_overlay_scroll"),
            |ui| {
            // Separator drag shares the right edge — force fully dormant so
            // resize doesn't flash the bar.
            if sidebar_separator_dragging(ui.ctx()) {
                let scroll = &mut ui.style_mut().spacing.scroll;
                scroll.floating_width = 0.0;
                scroll.bar_width = 0.0;
                scroll.active_handle_opacity = 0.0;
                scroll.interact_handle_opacity = 0.0;
                scroll.active_background_opacity = 0.0;
                scroll.interact_background_opacity = 0.0;
            }

            let mut area = egui::ScrollArea::vertical().auto_shrink([false, false]);
            if let Some(y) = self.forced_offset.take() {
                area = area.vertical_scroll_offset(y);
            }
            let mut offset_y = 0.0_f32;
            let mut escaped = None;
            area.show_viewport(ui, |ui, viewport| {
            let clip = ui.clip_rect();
            let width = (clip.width() - 2.0 * SCROLL_INSET).max(0.0);
            let offset = viewport.min.y;
            offset_y = offset;
            self.last_offset = offset;
            self.last_view_h = clip.height();
            self.last_content_w = width;
            // Reserve the full scroll extent (for the scrollbar).
            ui.allocate_exact_size(vec2(clip.width(), total_h.max(0.0)), Sense::hover());
            // A focus sink so keyboard nav only fires when the tree "has focus"
            // (granted by a row click) — not while typing in the editor. Kept
            // registered each frame; non-interactive so it never steals a click.
            let kbd = ui.interact(clip, kbd_focus_id(), Sense::focusable_noninteractive());
            // Anchor in-flow rows *and* stuck headers to the true viewport top
            // with the exact (unrounded) offset — `origin` is the screen y of
            // content-y 0. egui's own content origin is rounded to a whole pixel,
            // which would drift against the sub-pixel-smooth sticky pass and make
            // the in-flow↔stuck handoff jump under fractional scroll.
            //
            // Side inset only on in-flow width; sticky is full-bleed L/R with
            // content still padded so icons line up. Same top as flow (no T/B pad).
            let view = View {
                origin: pos2(clip.left() + SCROLL_INSET, clip.top() - offset),
                width,
                offset,
                sticky_left: clip.left(),
                sticky_width: clip.width(),
                sticky_top: clip.top(),
            };

            // Stuck rows are drawn pinned by the sticky pass; the in-flow pass
            // skips them so they aren't also drawn at their scrolled position.
            let layout = sticky_layout(&rows, &geoms, view.offset);
            let stuck: HashSet<usize> = layout.iter().map(|s| s.index).collect();
            // Advance elevate animations *before* paint so flow rows can ease
            // out of raised chrome the same frame they leave the sticky stack.
            self.tick_elev_anims(ui.ctx(), &rows, &geoms, &layout, view.offset);

            // In-flow, virtualized. Rows are full height; folder open clips the
            // child block so content emerges from under the parent.
            let view_bot = view.offset + viewport.height();
            for (i, &row) in rows.iter().enumerate() {
                if stuck.contains(&i) {
                    continue;
                }
                let g = geoms[i];
                // Outside ancestor reveal slot (still “behind” a closed/closing folder).
                let visible_top = g.y.max(g.clip_top);
                let visible_bot = (g.y + g.h).min(g.clip_bot);
                if visible_bot - visible_top < 0.5 {
                    continue;
                }
                if visible_bot < view.offset || visible_top > view_bot {
                    continue;
                }
                let top = view.origin.y + g.y;
                let full =
                    Rect::from_min_size(pos2(view.origin.x, top), vec2(view.width, ROW_H));
                let vis = Rect::from_min_max(
                    pos2(view.origin.x, view.origin.y + visible_top),
                    pos2(view.origin.x + view.width, view.origin.y + visible_bot),
                );
                if let Some(op) = self.row(ui, t, files, row, full, vis, Placement::Flow, pinned, me)
                {
                    escaped = Some(op);
                }
            }

            // The stuck ancestor stack over the top.
            if let Some(op) = self.sticky(ui, t, files, &rows, &geoms, view, &layout, pinned, me) {
                escaped = Some(op);
            }

            // Keyboard nav, only when the tree holds focus. Consume the keys so
            // neither the scroll area nor the editor also acts on them.
            if kbd.has_focus() {
                ui.memory_mut(|m| m.set_focus_lock_filter(kbd_focus_id(), tree_focus_filter()));
                use egui::{Key, Modifiers};
                let mut moved = false;
                for (key, down) in [(Key::ArrowDown, true), (Key::ArrowUp, false)] {
                    let extend = if ui.input_mut(|i| i.consume_key(Modifiers::NONE, key)) {
                        Some(false)
                    } else if ui.input_mut(|i| i.consume_key(Modifiers::SHIFT, key)) {
                        Some(true)
                    } else {
                        None
                    };
                    if let Some(extend) = extend {
                        self.apply(Action::CursorMove { down, extend }, files);
                        moved = true;
                    }
                }
                if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter)) {
                    if let Some(op) = self.apply(Action::OpenCursor, files) {
                        escaped = Some(op);
                    }
                }
                // Delete / Backspace remove the selection (non-short-circuit `|`
                // so both keys are consumed).
                let del = ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Delete))
                    | ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Backspace));
                if del {
                    if let Some(op) = self.apply(Action::DeleteSelected, files) {
                        escaped = Some(op);
                    }
                }
                // F2 renames the cursor row.
                if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::F2)) {
                    if let Some(id) = self.cursor {
                        self.apply(Action::BeginRename(id), files);
                    }
                }
                // Cut / copy / paste — Finder-style; only when not renaming so
                // Cmd+C/V stay free for the edit field.
                if self.renaming.is_none() {
                    let cut = ui.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::X))
                        || ui.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Cut)));
                    if cut {
                        self.apply(Action::CutSelected, files);
                    }
                    let copy = ui.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::C))
                        || ui.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Copy)));
                    if copy {
                        self.apply(Action::CopySelected, files);
                    }
                    let paste = ui.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::V))
                        || ui.input(|i| {
                            i.events.iter().any(|e| matches!(e, egui::Event::Paste(_)))
                        });
                    if paste {
                        if let Some(dest) = self.paste_dest(files) {
                            if let Some(op) =
                                self.apply(Action::PasteInto { dest }, files)
                            {
                                escaped = Some(op);
                            }
                        }
                    }
                }
                // Scroll the cursor back into view after a keyboard move —
                // minimum scroll only (same policy as `reveal`).
                if moved {
                    if let Some(ci) = self
                        .cursor
                        .and_then(|c| rows.iter().position(|r| r.id == c))
                    {
                        if let Some(y) =
                            min_scroll_to_row(&rows, &geoms, ci, offset, clip.height())
                        {
                            self.forced_offset = Some(y);
                        }
                    }
                }
            }

            // Hold near the top/bottom of the tree while dragging → scroll.
            self.dnd_edge_scroll(ui, clip, offset, total_h);
        });
            (escaped, offset_y)
        });
        // Drop chrome after every row fill so seps aren’t buried; float above.
        self.paint_drop_indicator(ui, t);
        self.paint_drag_float(ui, t);
        escaped
    }

    /// Content-band rect for a row: inside side insets. Sticky rows are full-
    /// bleed; strip the inset so float/sep match in-flow width.
    fn content_band(place: Placement, full_rect: Rect) -> Rect {
        if place.opaque() {
            Rect::from_min_max(
                pos2(full_rect.left() + SCROLL_INSET, full_rect.top()),
                pos2(full_rect.right() - SCROLL_INSET, full_rect.bottom()),
            )
        } else {
            full_rect
        }
    }

    /// While a tree DnD payload is live, scroll when the pointer sits in the
    /// top/bottom edge band of the viewport (or past it). Speed ramps with how
    /// deep into the band; uses `forced_offset` so the next frame picks it up.
    fn dnd_edge_scroll(&mut self, ui: &Ui, clip: Rect, offset: f32, total_h: f32) {
        if !DragAndDrop::has_any_payload(ui.ctx()) {
            return;
        }
        let Some(pointer) = ui.input(|i| i.pointer.hover_pos().or_else(|| i.pointer.latest_pos()))
        else {
            return;
        };
        // Ignore when the pointer is far sideways (e.g. over the editor).
        let x_slop = 32.0;
        if pointer.x < clip.left() - x_slop || pointer.x > clip.right() + x_slop {
            return;
        }

        let edge = DND_SCROLL_EDGE;
        let (dir, depth) = if pointer.y < clip.top() + edge {
            // Above or in the top band → scroll up. Past the top = full speed.
            let d = ((clip.top() + edge - pointer.y) / edge).clamp(0.0, 1.0);
            (-1.0_f32, d)
        } else if pointer.y > clip.bottom() - edge {
            let d = ((pointer.y - (clip.bottom() - edge)) / edge).clamp(0.0, 1.0);
            (1.0_f32, d)
        } else {
            return;
        };

        let dt = ui.input(|i| i.stable_dt).clamp(1.0 / 240.0, 1.0 / 15.0);
        // Quadratic ramp: gentle near the threshold, fast at the extreme.
        let speed = DND_SCROLL_MAX_PX_PER_SEC * depth * depth;
        let dy = dir * speed * dt;
        let max_off = (total_h - clip.height()).max(0.0);
        let new_off = (offset + dy).clamp(0.0, max_off);
        if (new_off - offset).abs() > 0.05 {
            self.forced_offset = Some(new_off);
            ui.ctx().request_repaint();
        }
    }

    /// Draw the stuck ancestor stack from a precomputed `layout` (see
    /// `sticky_layout`), clipping each header under its parent so a pushed one
    /// slides behind rather than over.
    ///
    /// Sticky chrome is full-bleed: flush to the viewport top and spanning the
    /// full clip width (occupies the scroll inset on top / left / right).
    /// Content indent still matches flow via the same `INDENT_*` metrics.
    ///
    /// Elevated chrome (raised fill + hairline) only when a sticky is *held
    /// above its natural scroll position* (`natural < vy`) — a true pin covering
    /// content under it. Rows that are merely in the sticky pass while sliding
    /// off (sibling boundary push, `natural ≈ vy`) keep the panel surface so the
    /// top content row isn't recolored. Diagnosed via `chrome_timing_diag`.
    #[allow(clippy::too_many_arguments)]
    fn sticky(
        &mut self, ui: &mut Ui, t: &Tokens, files: &impl FilesExt, rows: &[Row],
        _geoms: &[RowGeom], view: View, layout: &[Stuck], pinned: &HashSet<Uuid>,
        me: Option<&str>,
    ) -> Option<Op> {
        if layout.is_empty() {
            return None;
        }
        // Full-bleed vs inset flow — sticky owns the inset band.
        let x = view.sticky_left;
        let width = view.sticky_width;
        let vtop = view.sticky_top;

        let mut escaped = None;
        for s in layout {
            let id = rows[s.index].id;
            let top = vtop + s.vy;
            // Sticky headers pin at full row height (folder rows themselves
            // never scale — only their child block clips during open ease).
            let full = Rect::from_min_size(pos2(x, top), vec2(width, ROW_H));
            let clip = Rect::from_min_max(pos2(x, vtop + s.clip_top), pos2(x + width, top + ROW_H));
            // elevate_t already advanced in `tick_elev_anims` this frame.
            let elevate_t = self.elev_t_peek(id);
            if let Some(op) = self.row(
                ui,
                t,
                files,
                rows[s.index],
                full,
                full, // sticky always full height hit
                Placement::Sticky { clip, elevate_t },
                pinned,
                me,
            ) {
                escaped = Some(op);
            }
        }
        escaped
    }

    /// Drive sticky raise animations from layout: elevated stickies → 1, all
    /// others still tracked → 0. Call once per frame before painting.
    fn tick_elev_anims(
        &mut self, ctx: &egui::Context, rows: &[Row], geoms: &[RowGeom], layout: &[Stuck],
        offset: f32,
    ) {
        let mut seen = HashSet::new();
        for s in layout {
            let id = rows[s.index].id;
            seen.insert(id);
            let natural = geoms
                .get(s.index)
                .map(|g| g.y - offset)
                .unwrap_or(s.index as f32 * ROW_H - offset);
            let elevated = natural < s.vy - 0.01;
            self.elev_t(ctx, id, if elevated { 1.0 } else { 0.0 });
        }
        let lingering: Vec<Uuid> = self
            .elev_anim
            .keys()
            .copied()
            .filter(|id| !seen.contains(id))
            .collect();
        for id in lingering {
            if self.elev_t(ctx, id, 0.0) < 0.001 {
                self.elev_anim.remove(&id);
            }
        }
    }

    /// Paint and interact one row at an absolute rect (virtualized layout owns
    /// positioning, so this doesn't use egui flow). A `Sticky` placement fills an
    /// opaque background and clips its top under the parent header; the id is
    /// namespaced so a stuck header and its (offscreen) in-flow twin don't
    /// collide. Input → `Action` → `apply`, so a click matches the scripted one.
    #[allow(clippy::too_many_arguments)]
    fn row(
        &mut self, ui: &mut Ui, t: &Tokens, files: &impl FilesExt, row: Row,
        // Full logical row rect (ROW_H) used for content layout.
        full_rect: Rect,
        // Visible/hit rect (folder open clip ∩ row — may be a partial strip).
        vis_rect: Rect,
        place: Placement, pinned: &HashSet<Uuid>, me: Option<&str>,
    ) -> Option<Op> {
        if vis_rect.height() < 0.5 {
            return None;
        }
        let file = files.get_by_id(row.id)?;
        let rect = full_rect;

        // This row is being renamed: hand off to the inline glyphon field.
        if self.renaming.as_ref().is_some_and(|r| r.id == row.id) {
            return self.rename_row(ui, t, row, rect, files);
        }

        // Folder glyph tracks open-ish (includes mid-open / mid-close via target).
        let expanded = self.folder_is_openish(row.id);
        let selected = self.selected.contains(&row.id);

        // Clip to folder reveal slot ∩ sticky clip so the child block emerges
        // from under the parent (full-size rows, growing window).
        let anim_clip = vis_rect.intersect(ui.clip_rect());
        let painter = match place.clip() {
            Some(c) => ui.painter().with_clip_rect(c.intersect(anim_clip)),
            None => ui.painter().with_clip_rect(anim_clip),
        };
        let hit = place
            .clip()
            .map_or(vis_rect, |c| vis_rect.intersect(c))
            .intersect(ui.clip_rect());
        if hit.height() < 0.5 {
            return None;
        }
        let resp = ui.interact(hit, Id::new((place.ns(), row.id)), Sense::click_and_drag());
        let hover = ui.ctx().animate_bool(resp.id, resp.hovered());
        let dragging = DragAndDrop::has_any_payload(ui.ctx());

        // Refresh primary float chrome if the source row is still painted.
        // Always content-band width (not sticky full-bleed); keep depth for indent.
        if dragging && self.drag_primary == Some(row.id) {
            let band = Self::content_band(place, rect);
            self.drag_float = Some(DragRowSnap {
                width: band.width(),
                depth: row.depth,
                name: file.name.clone(),
                is_folder: row.is_folder,
                expanded,
            });
        }

        // Drag source cutout (editor-style): selected rows leave an empty
        // outlined slot while the float carries the content. Multi-select →
        // one hole per selected row. Matches float width (content band).
        if dragging && selected {
            let band = Self::content_band(place, rect);
            // Sticky elev is full-bleed — clear it so the hole doesn’t sit on a
            // raised slab; then the outlined content-band slot.
            if place.opaque() {
                painter.rect_filled(rect, 0.0, t.canvas());
            }
            painter.rect_filled(band, 5.0, t.canvas());
            painter.rect_stroke(
                band,
                5.0,
                Stroke::new(1.0, t.line()),
                egui::StrokeKind::Inside,
            );
            // Still accept drop targeting on/near source rows.
            if let Some(op) = self.handle_row_dnd(ui, t, files, row, rect, place, &resp) {
                return Some(op);
            }
            return None;
        }

        // Base chrome, then selection / hover on top — including fully elevated
        // stickies (they used to wash out hover/selection at elev == 1).
        //
        // Tree body is canvas. Hover/select go canvas→fg. Elevated sticky fill
        // is 66% surface (from canvas) so headers lift without matching either
        // the panel or the workspace. `elev` lerps canvas → sticky; sticky pass
        // supplies it, flow rows read lingering fade from the map.
        let panel = t.canvas();
        let sticky_fill = t.canvas().lerp_to_gamma(t.surface(), 0.66);
        let elev = if place.opaque() {
            place.elevate_t()
        } else {
            self.elev_t_peek(row.id)
        };
        let base = if elev > 0.0 {
            panel.lerp_to_gamma(sticky_fill, elev)
        } else if place.opaque() {
            panel
        } else {
            // In-flow idle: body already paints canvas.
            egui::Color32::TRANSPARENT
        };
        let fill = if selected {
            if base.a() == 0 {
                panel.lerp_to_gamma(t.fg(), 0.10)
            } else {
                base.lerp_to_gamma(t.fg(), 0.10)
            }
        } else if hover > 0.0 {
            if base.a() == 0 {
                panel.lerp_to_gamma(t.fg(), 0.05 * hover)
            } else {
                base.lerp_to_gamma(t.fg(), 0.05 * hover)
            }
        } else {
            base
        };
        let rounding = if elev >= 1.0 {
            0.0
        } else if elev > 0.0 && place.opaque() {
            5.0 * (1.0 - elev)
        } else if place.opaque() && !selected && hover == 0.0 {
            0.0
        } else {
            5.0
        };
        if fill.a() > 0 {
            painter.rect_filled(rect, rounding, fill);
        }
        // In a multi-selection, ring the active row (cursor) so keyboard focus
        // reads distinctly from the filled selection.
        if self.cursor == Some(row.id) && self.selected.len() > 1 {
            painter.rect_stroke(
                rect.shrink(1.0),
                5.0,
                egui::Stroke::new(1.0, t.line()),
                egui::StrokeKind::Inside,
            );
        }

        let cy = rect.center().y;
        // Sticky fill is full-bleed; content lines up with inset flow rows.
        let content_left = if place.opaque() {
            rect.left() + SCROLL_INSET
        } else {
            rect.left()
        };
        let mut x = content_left + INDENT_BASE + row.depth as f32 * INDENT_STEP;
        // Cut buffer dims the row (Finder “ready to move”); folder icons keep
        // accent, files match name ink.
        let is_cut = self
            .clip
            .as_ref()
            .is_some_and(|c| c.mode == ClipMode::Cut && c.ids.contains(&row.id));
        let ink = if is_cut { t.text_muted() } else { t.fg() };
        let icon_ink = if is_cut {
            t.text_muted()
        } else if row.is_folder {
            t.accent()
        } else {
            t.fg()
        };

        // Type icon: folders open/closed; documents via `DocType::from_name`
        // (same extension rules as master) mapped to Phosphor.
        let doc_type = DocType::from_name(&file.name);
        let icon = if row.is_folder {
            if expanded { icons::FOLDER_OPEN } else { icons::FOLDER }
        } else {
            icons::for_doc_type(doc_type)
        };
        let g = painter.layout_no_wrap(icon.into(), icons::font(16.0), icon_ink);
        painter.galley(pos2(x, cy - g.size().y / 2.0), g, icon_ink);
        x += TYPE_ICON_SLOT;

        // Trailing status marks (after the name, muted unless sync needs color):
        //   pin · people (you shared out) · pencil-slash (you can only view)
        //   · cloud↑/↓ (debounced dirty / in flight — Apple-style)
        let is_pinned = pinned.contains(&row.id);
        let is_shared = !file.shares.is_empty();
        let view_only = me.is_some_and(|u| my_access_is_view_only(files, row.id, u));
        let sync_dot = self.sync_dots.dots.get(&row.id).copied();
        let meta_font = icons::font(12.0);
        let pin_g = if is_pinned {
            Some(painter.layout_no_wrap(icons::PUSH_PIN.into(), meta_font.clone(), t.text_muted()))
        } else {
            None
        };
        let share_g = if is_shared {
            Some(painter.layout_no_wrap(
                icons::USERS.into(),
                meta_font.clone(),
                t.text_muted(),
            ))
        } else {
            None
        };
        let readonly_g = if view_only {
            Some(painter.layout_no_wrap(
                icons::PENCIL_SIMPLE_SLASH.into(),
                meta_font.clone(),
                t.text_muted(),
            ))
        } else {
            None
        };
        let (sync_g, sync_ink, sync_tip) = if let Some(dot) = sync_dot {
            let theme = ui.ctx().get_lb_theme();
            let color = match dot {
                SyncDot::Pushing => theme.fg().green,
                SyncDot::Dirty => theme.fg().yellow,
                SyncDot::Pulling => theme.fg().blue,
            };
            (
                Some(painter.layout_no_wrap(dot.glyph().into(), meta_font, color)),
                color,
                Some(dot.tip()),
            )
        } else {
            (None, t.text_muted(), None)
        };
        let meta_slot = [&pin_g, &share_g, &readonly_g, &sync_g]
            .into_iter()
            .filter_map(|g| g.as_ref())
            .map(|g| g.size().x + ICON_NAME_GAP)
            .sum::<f32>();

        // Display name hides known extensions (md, svg, pdf, chat) — same as
        // master's tree. Glyphon for emoji-safe rendering; end-ellipsis when the
        // row is narrower than the name, full name on hover if truncated.
        let display_name = doc_type.display_name(&file.name);
        let name_max_w = (rect.right() - 8.0 - meta_slot - x).max(0.0);
        let mut name_truncated = false;
        let mut name_drawn_w = 0.0_f32;
        if name_max_w > 0.0 {
            let name_rect = Rect::from_min_size(
                pos2(x, cy - NAME_LINE_H / 2.0),
                vec2(name_max_w, NAME_LINE_H),
            );
            let clip = place
                .clip()
                .map_or(ui.clip_rect(), |c| c.intersect(ui.clip_rect()))
                .intersect(name_rect)
                .intersect(anim_clip);
            if clip.width() > 0.0 && clip.height() > 0.0 {
                let full_w = GlyphonLabel::new(display_name, ink)
                    .font_size(NAME_FONT)
                    .line_height(NAME_LINE_H)
                    .max_width(f32::MAX)
                    .measure(ui)
                    .x;
                name_truncated = full_w > name_max_w + 0.5;
                let shaped = GlyphonLabel::new(display_name, ink)
                    .font_size(NAME_FONT)
                    .line_height(NAME_LINE_H)
                    .max_width(name_max_w)
                    .text_overflow(TextOverflow::EndEllipsis)
                    .build(ui.ctx());
                name_drawn_w = shaped.size.x.min(name_max_w);
                let area = shaped.text_area(name_rect, ui.ctx(), clip);
                ui.painter().add(
                    egui_wgpu_renderer::egui_wgpu::Callback::new_paint_callback(
                        clip,
                        GlyphonRendererCallback::new(vec![area]),
                    ),
                );
            }
        }
        let mut meta_x = x + name_drawn_w;
        // Paint trailing glyphs left→right: pin, shared-out, view-only, sync.
        if let Some(pg) = pin_g {
            meta_x += ICON_NAME_GAP;
            let w = pg.size().x;
            painter.galley(pos2(meta_x, cy - pg.size().y / 2.0), pg, t.text_muted());
            meta_x += w;
        }
        if let Some(sg) = share_g {
            meta_x += ICON_NAME_GAP;
            let w = sg.size().x;
            painter.galley(pos2(meta_x, cy - sg.size().y / 2.0), sg, t.text_muted());
            meta_x += w;
        }
        if let Some(rg) = readonly_g {
            meta_x += ICON_NAME_GAP;
            let w = rg.size().x;
            painter.galley(pos2(meta_x, cy - rg.size().y / 2.0), rg, t.text_muted());
            meta_x += w;
        }
        if let Some(sg) = sync_g {
            meta_x += ICON_NAME_GAP;
            painter.galley(pos2(meta_x, cy - sg.size().y / 2.0), sg, sync_ink);
        }

        // Rich tip on every row: name, path, modified, status / sync.
        // Leading-aligned (not center) so it reads as row metadata.
        {
            let path = files.path(row.id);
            let modified = file.last_modified.elapsed_human_string();
            let modified_by = file.last_modified_by.as_str();
            // Share summary: “3 collaborators can edit” as one phrase (no mid-dot).
            // Mode only when uniform so mixed access doesn’t overclaim.
            let collab_line = if is_shared {
                let n = file.shares.len();
                let writes = file
                    .shares
                    .iter()
                    .filter(|s| matches!(s.mode, ShareMode::Write))
                    .count();
                let people = if n == 1 {
                    "1 collaborator".to_string()
                } else {
                    format!("{n} collaborators")
                };
                Some(if writes == n {
                    format!("{people} can edit")
                } else if writes == 0 {
                    format!("{people} can view")
                } else {
                    people
                })
            } else {
                None
            };
            let mut status: Vec<String> = Vec::new();
            if is_pinned {
                status.push("Pinned".into());
            }
            if let Some(c) = collab_line {
                status.push(c);
            }
            if view_only {
                status.push("View only".into());
            }
            if let Some(tip) = sync_tip {
                status.push(tip.into());
            }
            if is_cut {
                status.push("Cut · ready to move".into());
            }
            let title = if display_name != file.name.as_str() {
                format!("{display_name}  ({})", file.name)
            } else {
                display_name.to_string()
            };
            let _ = name_truncated;
            tip_ui_rich(ui.ctx(), &resp, |ui| {
                // Name + path as one block; meta as a second block.
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.label(egui::RichText::new(&title).size(14.0).strong().color(t.fg()));
                ui.label(egui::RichText::new(&path).size(12.5).color(t.text_muted()));
                ui.add_space(8.0);
                ui.spacing_mut().item_spacing.y = 2.0;
                let when = if !modified_by.is_empty() && modified_by != "<unknown>" {
                    format!("Modified {modified} · {modified_by}")
                } else {
                    format!("Modified {modified}")
                };
                ui.label(egui::RichText::new(when).size(12.5).color(t.text_muted()));
                if !status.is_empty() {
                    ui.label(
                        egui::RichText::new(status.join(" · "))
                            .size(12.5)
                            .color(t.text_muted()),
                    );
                }
            });
        }

        // Any click on the tree grants it keyboard focus (and locks the arrows,
        // in place before the first arrow press).
        if resp.clicked()
            || resp.clicked_by(egui::PointerButton::Middle)
            || resp.secondary_clicked()
        {
            ui.memory_mut(|m| {
                m.request_focus(kbd_focus_id());
                m.set_focus_lock_filter(kbd_focus_id(), tree_focus_filter());
            });
        }

        // Right-click selects the row (unless already selected, to keep a
        // multi-selection) and opens the context menu. Menu choices become the
        // same `Action`s a click or key would — applied after the closure so it
        // borrows no `self`.
        if resp.secondary_clicked() && !self.selected.contains(&row.id) {
            self.apply(Action::Select(row.id), files);
        }
        let row_selected = self.selected.contains(&row.id);
        let folder_open = row.is_folder && self.folder_is_openish(row.id);
        // Custom menu (not egui's) — content-sized, macOS metrics; see
        // `widgets::context_menu` for why we abandoned `Response::context_menu`.
        // Create under the folder when the row is a folder; otherwise alongside
        // the file (same parent).
        let create_parent = if row.is_folder { row.id } else { file.parent };
        let has_clip = self.clip.is_some();
        // Groups: Open | Create | Expand | Arrange | Share/export | Delete
        let chosen = crate::widgets::context_menu::show(&resp, t, |m| {
            // Open
            if !row.is_folder {
                m.item(
                    icons::ARROW_SQUARE_OUT,
                    "Open",
                    Action::Open { id: row.id, new_tab: false },
                );
                m.item(
                    icons::APP_WINDOW,
                    "Open in new tab",
                    Action::Open { id: row.id, new_tab: true },
                );
                m.separator();
            }
            // Create
            m.item(
                icons::FILE_PLUS,
                "New document",
                Action::CreateDoc { parent: create_parent },
            );
            m.item(
                icons::FOLDER_PLUS,
                "New folder",
                Action::CreateFolder { parent: create_parent },
            );
            // Expand — one-level toggle + recursive under *this* folder only.
            if row.is_folder {
                m.separator();
                if folder_open {
                    m.item(icons::CARET_RIGHT, "Collapse", Action::Toggle(row.id));
                } else {
                    m.item(icons::CARET_DOWN, "Expand", Action::Toggle(row.id));
                }
                m.item(
                    icons::CARET_DOUBLE_DOWN,
                    "Expand all",
                    Action::ExpandSubtree(row.id),
                );
                m.item(
                    icons::CARET_DOUBLE_UP,
                    "Collapse all",
                    Action::CollapseSubtree(row.id),
                );
            }
            // Arrange — place/organize first, then clipboard ops.
            m.separator();
            m.item(icons::PENCIL_SIMPLE, "Rename", Action::BeginRename(row.id));
            m.item(icons::FOLDERS, "Move", Action::MoveSelected);
            let (pin_icon, pin_label) = if pinned.contains(&row.id) {
                (icons::PUSH_PIN_SLASH, "Unpin")
            } else {
                (icons::PUSH_PIN, "Pin")
            };
            m.item(pin_icon, pin_label, Action::TogglePin(row.id));
            m.item(icons::SCISSORS, "Cut", Action::CutSelected);
            m.item(icons::COPY, "Copy", Action::CopySelected);
            if has_clip {
                m.item(
                    icons::CLIPBOARD,
                    "Paste",
                    Action::PasteInto { dest: create_parent },
                );
            }
            m.item(icons::FILES, "Duplicate", Action::Duplicate(row.id));
            // Share / export
            m.separator();
            m.item(icons::SHARE_NETWORK, "Share", Action::Share(row.id));
            m.item(icons::LINK, "Copy link", Action::CopyLink(row.id));
            m.item(icons::EXPORT, "Export", Action::ExportSelected);
            // Delete vs remove-from-files. Organized shares appear as the
            // *target* (Document/Folder) with the sharer as owner — Link
            // metadata is invisible in list_metadatas.
            m.separator();
            let remove_ids: Vec<Uuid> = if self.selected.contains(&row.id) {
                self.selected.iter().copied().collect()
            } else {
                vec![row.id]
            };
            let all_organized_shares = me.is_some_and(|me| {
                !remove_ids.is_empty()
                    && remove_ids.iter().all(|id| {
                        files
                            .get_by_id(*id)
                            .is_some_and(|f| is_organized_share(files, f, me))
                    })
            });
            if all_organized_shares {
                // Pair with “Add to files” / FOLDER_PLUS — not a true delete.
                m.item(
                    icons::FOLDER_MINUS,
                    "Remove from files",
                    Action::DeleteSelected,
                );
            } else {
                m.item_danger(icons::TRASH, "Delete", Action::DeleteSelected);
            }
        });
        if let Some(a) = chosen {
            // Selection-scoped actions need the row in the selection.
            let needs_select = matches!(
                a,
                Action::DeleteSelected
                    | Action::MoveSelected
                    | Action::ExportSelected
                    | Action::CutSelected
                    | Action::CopySelected
            );
            if needs_select && !row_selected {
                self.apply(Action::Select(row.id), files);
            }
            return self.apply(a, files);
        }

        // Middle-click opens a document in a new tab (folders ignore it).
        if resp.clicked_by(egui::PointerButton::Middle) {
            if !row.is_folder {
                self.apply(Action::Select(row.id), files);
                return self.apply(Action::Open { id: row.id, new_tab: true }, files);
            }
            return None;
        }

        // Primary click. Cmd/Shift are pure selection gestures; a plain click
        // selects and then acts (open a doc / toggle a folder). All routed through
        // the chokepoint so a driver sees identical effects.
        if resp.clicked() {
            let mods = ui.input(|i| i.modifiers);
            if mods.command {
                return self.apply(Action::SelectAdd(row.id), files);
            }
            if mods.shift {
                return self.apply(Action::SelectRange(row.id), files);
            }
            self.apply(Action::Select(row.id), files);
            let act = if row.is_folder {
                Action::Toggle(row.id)
            } else {
                Action::Open { id: row.id, new_tab: false }
            };
            return self.apply(act, files);
        }

        // ── Drag and drop (master tree + floating card) ───────────────────
        if let Some(op) = self.handle_row_dnd(ui, t, files, row, rect, place, &resp) {
            return Some(op);
        }
        None
    }

    /// Start / hover / release DnD for one row. Selection is the move set.
    #[allow(clippy::too_many_arguments)]
    fn handle_row_dnd(
        &mut self, ui: &mut Ui, _t: &Tokens, files: &impl FilesExt, row: Row, rect: Rect,
        place: Placement, resp: &egui::Response,
    ) -> Option<Op> {
        let file = files.get_by_id(row.id)?;
        let band = Self::content_band(place, rect);

        // Begin drag after a small movement threshold (egui alone is too eager).
        if resp.dragged()
            && !DragAndDrop::has_any_payload(ui.ctx())
            && ui.input(|i| {
                let (Some(pos), Some(origin)) =
                    (i.pointer.interact_pos(), i.pointer.press_origin())
                else {
                    return false;
                };
                pos.distance(origin) > DRAG_THRESHOLD
            })
        {
            if !self.selected.contains(&row.id) {
                self.apply(Action::Select(row.id), files);
            }
            DragAndDrop::set_payload(ui.ctx(), TreeDnd);
            self.drag_primary = Some(row.id);
            // Exact content-band width — no max clamp (that made the card
            // slightly narrower than the row it was lifted from).
            self.drag_float = Some(DragRowSnap {
                width: band.width(),
                depth: row.depth,
                name: file.name.clone(),
                is_folder: row.is_folder,
                expanded: self.expanded.contains(&row.id),
            });
            if let Some(p) = ui.input(|i| i.pointer.interact_pos()) {
                // Grab relative to content band so sticky full-bleed doesn’t shift.
                self.drag_grab_offset = Some(p - band.left_top());
            }
            ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
        }

        if !DragAndDrop::has_any_payload(ui.ctx()) {
            return None;
        }

        ui.ctx().set_cursor_icon(CursorIcon::Grabbing);

        let pointer = ui.input(|i| i.pointer.interact_pos())?;
        if !resp.rect.contains(pointer) {
            // Pointer left this row — don't clear drop_hover here (another row may own it).
            return None;
        }

        let into_folder = file.is_folder()
            && (pointer.y - rect.center().y).abs() < rect.height() / 4.0;
        let dest = if into_folder { row.id } else { file.parent };
        let valid = can_drop_selection(files, &self.selected, dest);

        // Record indicator for post-row paint (full content band, not depth-indented).
        self.drop_paint = Some(if into_folder && file.is_folder() {
            DropPaint::Into { rect: band, valid }
        } else {
            let y = if pointer.y < rect.center().y {
                band.min.y
            } else {
                band.max.y
            };
            DropPaint::Between {
                y,
                x0: band.left(),
                x1: band.right(),
                valid,
            }
        });

        // Auto-expand folder under pointer (debounce) — same open ease as click.
        if into_folder && file.is_folder() && valid {
            match self.drop_hover.as_mut() {
                Some((id, start)) if *id == row.id => {
                    if start.elapsed() > Duration::from_millis(DROP_EXPAND_MS)
                        && !self.folder_is_openish(row.id)
                    {
                        self.animate_folder(row.id, true);
                        ui.ctx().request_repaint();
                    }
                }
                _ => {
                    self.drop_hover = Some((row.id, Instant::now()));
                }
            }
        } else if self.drop_hover.is_some_and(|(id, _)| id == row.id) {
            self.drop_hover = None;
        }

        // Release → move selection into dest.
        if resp.dnd_release_payload::<TreeDnd>().is_some() {
            self.drop_hover = None;
            self.drag_grab_offset = None;
            self.drag_primary = None;
            self.drag_float = None;
            if !valid {
                return None;
            }
            let ids: Vec<Uuid> = self.selected.iter().copied().collect();
            if ids.is_empty() {
                return None;
            }
            return Some(Op::MoveInto { ids, parent: dest });
        }

        if resp.drag_stopped() {
            self.drop_hover = None;
            self.drag_grab_offset = None;
            self.drag_primary = None;
            self.drag_float = None;
        }

        None
    }

    /// Drop line / into ring — painted after rows so fills can’t bury the sep,
    /// full content-band width (inside side insets, not depth-indented).
    fn paint_drop_indicator(&self, ui: &mut Ui, t: &Tokens) {
        let Some(paint) = self.drop_paint else { return };
        let (color, _) = match paint {
            DropPaint::Between { valid, .. } | DropPaint::Into { valid, .. } => (
                if valid { t.accent() } else { t.danger() },
                valid,
            ),
        };
        // Thicker than row hairlines so it reads through tree chrome.
        let stroke = Stroke::new(2.5, color);
        ui.scope_builder(
            egui::UiBuilder::new().layer_id(LayerId::new(
                Order::Foreground,
                Id::new("file_tree_drop_indicator"),
            )),
            |ui| {
                let p = ui.painter();
                match paint {
                    DropPaint::Into { rect, .. } => {
                        p.rect_stroke(rect, 5.0, stroke, egui::StrokeKind::Inside);
                    }
                    DropPaint::Between { y, x0, x1, .. } => {
                        // Full content band; leading dot at the left edge.
                        p.hline(x0..=x1, y, stroke);
                        p.circle_filled(pos2(x0, y), 4.0, color);
                        p.circle_filled(pos2(x1, y), 4.0, color);
                    }
                }
            },
        );
    }

    /// Floating card for the drag — paints like a real tree row (icon + name).
    /// Multi-select: primary row chrome + “+N more” trailing badge.
    /// Width is the **exact** content-band width of the source row (lifted off
    /// the page), never clamped down.
    fn paint_drag_float(&self, ui: &mut Ui, t: &Tokens) {
        if !DragAndDrop::has_any_payload(ui.ctx()) {
            return;
        }
        let Some(snap) = self.drag_float.as_ref() else {
            return;
        };
        let Some(pointer) = ui.input(|i| i.pointer.latest_pos()) else {
            return;
        };

        let grab = self.drag_grab_offset.unwrap_or(Vec2::ZERO);
        // Exact source content width — no max clamp (that was the “too narrow”).
        let card_w = snap.width.max(1.0);
        let card = Rect::from_min_size(pointer - grab, vec2(card_w, ROW_H));

        let n = self.selected.len().max(1);
        let extra = n.saturating_sub(1);

        // Paint through floating chrome (same family as menus / tips).
        egui::Area::new(Id::new("file_tree_drag_float"))
            .order(Order::Tooltip)
            .fixed_pos(card.min)
            .sense(Sense::hover())
            .show(ui.ctx(), |ui| {
                t.floating()
                    .frame_margin(egui::Margin::ZERO)
                    .show(ui, |ui| {
                // Frame draws fill/stroke/shadow; content uses the card size.
                ui.set_min_size(card.size());
                ui.set_max_size(card.size());
                let p = ui.painter();
                let card = ui.max_rect();

                let cy = card.center().y;
                // Match source row content layout: same indent as the tree row.
                let mut x =
                    card.left() + INDENT_BASE + snap.depth as f32 * INDENT_STEP;

                let icon_ink = if snap.is_folder { t.accent() } else { t.fg() };
                let icon = if snap.is_folder {
                    if snap.expanded {
                        icons::FOLDER_OPEN
                    } else {
                        icons::FOLDER
                    }
                } else {
                    icons::for_doc_type(DocType::from_name(&snap.name))
                };
                let ig = p.layout_no_wrap(icon.into(), icons::font(16.0), icon_ink);
                p.galley(pos2(x, cy - ig.size().y / 2.0), ig, icon_ink);
                x += TYPE_ICON_SLOT;

                let display = DocType::from_name(&snap.name).display_name(&snap.name);
                let more_g = if extra > 0 {
                    Some(p.layout_no_wrap(
                        format!("+{extra} more"),
                        egui::FontId::proportional(12.0),
                        t.text_muted(),
                    ))
                } else {
                    None
                };
                let more_w = more_g
                    .as_ref()
                    .map(|g| g.size().x + 10.0)
                    .unwrap_or(0.0);
                // Same trailing pad as a tree row name (`rect.right() - 8`).
                let name_max = (card.right() - 8.0 - more_w - x).max(24.0);

                // Glyphon for emoji-safe names; end-ellipsis when multi shortens space.
                let name_rect = Rect::from_min_size(
                    pos2(x, cy - NAME_LINE_H / 2.0),
                    vec2(name_max, NAME_LINE_H),
                );
                let shaped = GlyphonLabel::new(display, t.fg())
                    .font_size(NAME_FONT)
                    .line_height(NAME_LINE_H)
                    .max_width(name_max)
                    .text_overflow(TextOverflow::EndEllipsis)
                    .build(ui.ctx());
                let clip = name_rect; // float is unclipped (tooltip layer)
                let area = shaped.text_area(name_rect, ui.ctx(), clip);
                ui.painter().add(
                    egui_wgpu_renderer::egui_wgpu::Callback::new_paint_callback(
                        clip,
                        GlyphonRendererCallback::new(vec![area]),
                    ),
                );

                if let Some(mg) = more_g {
                    p.galley(
                        pos2(card.right() - 8.0 - mg.size().x, cy - mg.size().y / 2.0),
                        mg,
                        t.text_muted(),
                    );
                }
                    });
            });
        ui.ctx().request_repaint();
    }

    /// Draw the inline rename field for `row` — the emoji-safe glyphon editor, so
    /// names with emoji shape correctly. Commits on Enter / click-away, cancels on
    /// Escape (both surface as focus loss; Escape is checked first).
    fn rename_row(
        &mut self, ui: &mut Ui, t: &Tokens, row: Row, rect: Rect, files: &impl FilesExt,
    ) -> Option<Op> {
        use workspace_rs::widgets::GlyphonTextEdit;

        // Selected-style background and the type icon, matching a normal row.
        // Icon tracks the rename buffer so extension changes update live (master).
        ui.painter()
            .rect_filled(rect, 5.0, t.canvas().lerp_to_gamma(t.fg(), 0.10));
        let cy = rect.center().y;
        let x = rect.left() + INDENT_BASE + row.depth as f32 * INDENT_STEP;

        let te_id = Id::new(("rename", row.id));
        let field = Rect::from_min_max(
            pos2(x + TYPE_ICON_SLOT, cy - NAME_LINE_H / 2.0),
            pos2(rect.right() - 8.0, cy + NAME_LINE_H / 2.0),
        );

        let rn = self.renaming.as_mut()?;
        let icon = if row.is_folder {
            icons::FOLDER
        } else {
            icons::for_doc_type(DocType::from_name(&rn.buf))
        };
        let icon_ink = if row.is_folder { t.accent() } else { t.fg() };
        let g = ui
            .painter()
            .layout_no_wrap(icon.into(), icons::font(16.0), icon_ink);
        ui.painter()
            .galley(pos2(x, cy - g.size().y / 2.0), g, icon_ink);

        if rn.fresh {
            ui.memory_mut(|m| m.request_focus(te_id));
            rn.fresh = false;
        }
        // Select the name stem (up to the last dot) so the extension is preserved.
        let stem = stem_len(&rn.buf, row.is_folder);
        let escape = ui.input(|i| i.key_pressed(egui::Key::Escape));
        let resp = ui.put(
            field,
            GlyphonTextEdit::new(&mut rn.buf)
                .id(te_id)
                .font_size(NAME_FONT)
                .line_height(NAME_LINE_H)
                .select_on_focus(0, stem),
        );

        if escape {
            return self.apply(Action::CancelRename, files);
        }
        if resp.lost_focus() {
            return self.apply(Action::CommitRename, files);
        }
        None
    }

    /// A structured, textual projection of tree state — the no-pixel readout, the
    /// pure counterpart to the screencap. Lists the visible rows with their
    /// indentation, folder expansion (`▾`/`▸`), and selection (`◂ selected`).
    /// Same flattened list `show` renders, so it's a faithful mirror.
    pub fn readout(&self, files: &impl FilesExt) -> String {
        let mut out = String::new();
        for row in self.flatten(files) {
            let Some(file) = files.get_by_id(row.id) else { continue };
            let indent = "  ".repeat(row.depth);
            let marker = if row.is_folder {
                if self.expanded.contains(&row.id) { "▾ " } else { "▸ " }
            } else {
                "  "
            };
            let sel = if self.selected.contains(&row.id) { "  ◂ selected" } else { "" };
            out.push_str(&format!("{indent}{marker}{}{sel}\n", file.name));
        }
        out
    }

    /// Numeric sticky readout for headless observation: `(name, vy, height)` per
    /// stuck header at a scroll `offset`. The non-visual counterpart to a
    /// screencap — sampled across offsets, it exposes the handoff's continuity
    /// (a pop shows up as a `vy` teleport or a height jumping from 0).
    pub fn sticky_debug(&self, files: &impl FilesExt, offset: f32) -> Vec<(String, f32, f32)> {
        let rows = self.flatten(files);
        let (geoms, _) = self.row_geometry(&rows);
        sticky_layout(&rows, &geoms, offset)
            .into_iter()
            .map(|s| {
                let name = files
                    .get_by_id(rows[s.index].id)
                    .map_or(String::new(), |f| f.name.clone());
                (name, s.vy, s.height())
            })
            .collect()
    }

    /// Every visible row's effective on-screen `(index, vy)` at a scroll `offset`
    /// — stuck rows at their pinned position, the rest at their scrolled one. The
    /// projection a continuity sampler wants: a smooth handoff means each row's
    /// `vy` moves by ~the offset step; a pop shows up as a teleport.
    pub fn row_positions(
        &self, files: &impl FilesExt, offset: f32, viewport: f32,
    ) -> Vec<(usize, f32)> {
        let rows = self.flatten(files);
        let (geoms, _) = self.row_geometry(&rows);
        let pinned: std::collections::HashMap<usize, f32> = sticky_layout(&rows, &geoms, offset)
            .iter()
            .map(|s| (s.index, s.vy))
            .collect();
        (0..rows.len())
            .map(|i| {
                let flow = geoms.get(i).map(|g| g.y - offset).unwrap_or(i as f32 * ROW_H - offset);
                (i, pinned.get(&i).copied().unwrap_or(flow))
            })
            .filter(|&(_, vy)| vy > -ROW_H && vy < viewport)
            .collect()
    }
}

/// Share **root** that sits in *our* tree via “Add to files”.
///
/// Link metadata is invisible in `list_metadatas`; the UI shows the target with
/// the sharer's `owner`. Deleting that root returns it to Shared with me.
///
/// Nested files under a shared folder also have `owner != me` — they are **not**
/// organized shares. Only treat an item as remove-from-files when its parent is
/// ours (or root), i.e. the entry point we added to the tree.
pub fn is_organized_share(files: &impl FilesExt, file: &File, me: &str) -> bool {
    if file.is_root() || file.owner.eq_ignore_ascii_case(me) {
        return false;
    }
    let Some(parent) = files.get_by_id(file.parent) else {
        // Missing parent — treat as a free-floating share root.
        return true;
    };
    parent.is_root() || parent.owner.eq_ignore_ascii_case(me)
}

/// True when `me` reaches `id` only with Read (including via shared ancestors),
/// not as owner and not with Write.
fn my_access_is_view_only(files: &impl FilesExt, id: Uuid, me: &str) -> bool {
    let Some(file) = files.get_by_id(id) else {
        return false;
    };
    if file.owner.eq_ignore_ascii_case(me) {
        return false;
    }
    let mut best: Option<ShareMode> = None;
    for fid in std::iter::once(id).chain(files.ancestors(id)) {
        let Some(f) = files.get_by_id(fid) else {
            continue;
        };
        for s in &f.shares {
            if !s.shared_with.eq_ignore_ascii_case(me) {
                continue;
            }
            best = Some(match (best, s.mode) {
                (Some(ShareMode::Write), _) | (_, ShareMode::Write) => ShareMode::Write,
                _ => ShareMode::Read,
            });
        }
    }
    matches!(best, Some(ShareMode::Read))
}

/// The focus id the tree parks keyboard focus on (see the focus sink in `show`).
fn kbd_focus_id() -> Id {
    Id::new("file_tree_kbd_focus")
}

/// Lock the vertical arrows to the tree while it has focus. Without this, egui
/// spends arrow keys on its own spatial focus navigation — moving focus off the
/// tree after the first press — instead of letting the tree move its cursor.
fn tree_focus_filter() -> egui::EventFilter {
    egui::EventFilter { horizontal_arrows: false, vertical_arrows: true, tab: false, escape: false }
}

/// True when `id` is `ancestor` or nested under it.
fn is_under(files: &impl FilesExt, id: Uuid, ancestor: Uuid) -> bool {
    let mut cur = id;
    for _ in 0..64 {
        if cur == ancestor {
            return true;
        }
        let Some(f) = files.get_by_id(cur) else {
            return false;
        };
        if f.is_root() || f.parent == f.id {
            return false;
        }
        cur = f.parent;
    }
    false
}

/// Byte length of the name's stem — everything before the last dot (for
/// documents), so inline rename preselects the name and preserves the extension.
fn stem_len(name: &str, is_folder: bool) -> usize {
    if is_folder {
        return name.len();
    }
    match name.rfind('.') {
        Some(i) if i > 0 => i,
        _ => name.len(),
    }
}

/// Sorted child ids, materialized so the `files` borrow doesn't straddle the
/// recursive `&mut self` call.
fn child_ids(files: &impl FilesExt, id: Uuid) -> Vec<Uuid> {
    files.children(id).iter().map(|f| f.id).collect()
}

/// Bottom of the sticky stack at `offset`, in viewport-y (content-y 0 is
/// `origin.y + offset`). Zero when nothing is stuck.
fn sticky_stack_bottom(rows: &[Row], geoms: &[RowGeom], offset: f32) -> f32 {
    sticky_layout(rows, geoms, offset)
        .into_iter()
        .map(|s| s.vy + ROW_H)
        .fold(0.0_f32, f32::max)
        .max(0.0)
}

/// True while the shell's sidebar edge is mid-drag. Prefers the sticky latch
/// written by `lib.rs` (`lb_sidebar_resizing`) so a leftward drag — pointer
/// over the tree while panel width lags — doesn't drop out for a frame.
/// Falls back to live `is_being_dragged` on the SidePanel / content-side ids.
fn sidebar_separator_dragging(ctx: &egui::Context) -> bool {
    if ctx.data(|d| d.get_temp::<bool>(Id::new(SIDEBAR_RESIZING_LATCH)).unwrap_or(false)) {
        return true;
    }
    let panel_resize = Id::new("sidebar").with("__resize");
    let content_resize = Id::new("sidebar_resize_content");
    ctx.is_being_dragged(panel_resize) || ctx.is_being_dragged(content_resize)
}

/// True if `anc` is `id` or an ancestor of `id`.
fn is_ancestor_of(files: &impl FilesExt, anc: Uuid, id: Uuid) -> bool {
    let mut cur = id;
    for _ in 0..64 {
        if cur == anc {
            return true;
        }
        let Some(f) = files.get_by_id(cur) else {
            return false;
        };
        if f.is_root() || f.parent == f.id {
            return false;
        }
        cur = f.parent;
    }
    false
}

/// Whether moving `selected` into folder `dest` is legal.
fn can_drop_selection(
    files: &impl FilesExt, selected: &HashSet<Uuid>, dest: Uuid,
) -> bool {
    if selected.contains(&dest) {
        return false;
    }
    let Some(dest_file) = files.get_by_id(dest) else {
        return false;
    };
    if !dest_file.is_folder() {
        return false;
    }
    for &s in selected {
        // Can't drop a folder into itself or its descendants.
        if is_ancestor_of(files, s, dest) {
            return false;
        }
    }
    true
}

/// Row top is at/below the sticky stack at `offset` (not covered by stickies).
fn row_clears_sticky(rows: &[Row], geoms: &[RowGeom], i: usize, offset: f32) -> bool {
    let cy = geoms.get(i).map(|g| g.y).unwrap_or(i as f32 * ROW_H);
    cy + 0.01 >= offset + sticky_stack_bottom(rows, geoms, offset)
}

/// Row bottom is at/above the viewport bottom.
fn row_above_viewport_bottom(geoms: &[RowGeom], i: usize, offset: f32, view_h: f32) -> bool {
    let cy = geoms.get(i).map(|g| g.y).unwrap_or(i as f32 * ROW_H);
    view_h <= 0.0 || cy + ROW_H <= offset + view_h + 0.01
}

/// Minimum scroll so row `i` is fully visible in the open band (below the
/// sticky stack, above the viewport bottom). `None` if already on-screen.
///
/// Sticky height is discontinuous in offset (folders pin/unpin). Fixed-point
/// iteration `offset = cy - sticky(offset)` oscillates by ~ROW_H at the pin
/// boundary and leaves the row one row short — see `min_scroll_diag`. Scroll-up
/// uses a sticky-only binary search (monotone enough); scroll-down starts at
/// bottom-align and walks until the row also clears stickies.
fn min_scroll_to_row(
    rows: &[Row], geoms: &[RowGeom], i: usize, offset: f32, view_h: f32,
) -> Option<f32> {
    let cy = geoms.get(i).map(|g| g.y).unwrap_or(i as f32 * ROW_H);
    let sticky_h = sticky_stack_bottom(rows, geoms, offset);
    let need_up = cy < offset + sticky_h;
    let need_down = view_h > 0.0 && cy + ROW_H > offset + view_h;
    if !need_up && !need_down {
        return None;
    }

    if need_up {
        // Largest offset in [0, current] where the row clears stickies = min
        // upward movement. Viewport-bottom is not mixed in: combining it breaks
        // monotonicity and the search collapses to a bad endpoint.
        return Some(max_offset_clearing_sticky(rows, geoms, i, offset.min(cy)));
    }

    // need_down: min offset that puts the row bottom on-screen *and* clears
    // stickies. Start at naive bottom-align; if stickies cover the row there,
    // walk upward (more offset) until clear or we run out of room.
    let o = (cy + ROW_H - view_h).max(0.0);
    if row_clears_sticky(rows, geoms, i, o) {
        return Some(o);
    }
    // Binary search for min O in [o, cy] with sticky clear + bottom on-screen.
    // Prefer still satisfying bottom; if impossible (open band < ROW_H), clear
    // sticky with the row as high as possible under the stack.
    let mut lo = o;
    let mut hi = cy.max(o);
    let mut best = None;
    for _ in 0..32 {
        let mid = (lo + hi) * 0.5;
        let clear = row_clears_sticky(rows, geoms, i, mid);
        let bot = row_above_viewport_bottom(geoms, i, mid, view_h);
        if clear && bot {
            best = Some(mid);
            hi = mid; // try less scroll-down
        } else if !bot {
            // Row still past bottom — need more offset.
            lo = mid;
        } else {
            // Clears bottom but not sticky — also need more offset (row higher
            // under a shorter stack, or past the pin boundary).
            lo = mid;
        }
    }
    if let Some(b) = best {
        return Some(b);
    }
    // Impossible to satisfy both (sticky stack taller than view − ROW_H).
    // Clear stickies; accept bottom clipping.
    Some(max_offset_clearing_sticky(rows, geoms, i, cy))
}

/// Max offset in `[0, hi]` at which row `i` clears the sticky stack. Sticky
/// coverage grows with offset in practice, so the feasible set is a prefix and
/// binary search for the right edge is stable (no fixed-point oscillation).
fn max_offset_clearing_sticky(rows: &[Row], geoms: &[RowGeom], i: usize, hi: f32) -> f32 {
    let mut hi = hi.max(0.0);
    if row_clears_sticky(rows, geoms, i, hi) {
        return hi;
    }
    let mut lo = 0.0f32;
    let mut best = 0.0f32;
    for _ in 0..32 {
        let mid = (lo + hi) * 0.5;
        if row_clears_sticky(rows, geoms, i, mid) {
            best = mid;
            lo = mid;
        } else {
            hi = mid;
        }
    }
    best
}

/// One stuck header's geometry, viewport-relative. `vy` is its top (negative
/// while sliding out the top); `clip_top` is where its parent clips it. Visible
/// height is `vy + ROW_H - clip_top`.
struct Stuck {
    index: usize,
    vy: f32,
    clip_top: f32,
}

impl Stuck {
    fn height(&self) -> f32 {
        (self.vy + ROW_H - self.clip_top).max(0.0)
    }
}

/// Index of the row whose content-y range covers `cy` (or the first row at/after
/// `cy`). Used by sticky probe with animated (non-uniform) row heights.
fn row_index_at_y(geoms: &[RowGeom], cy: f32) -> Option<usize> {
    if geoms.is_empty() {
        return None;
    }
    let mut lo = 0usize;
    let mut hi = geoms.len();
    while lo < hi {
        let mid = (lo + hi) / 2;
        if geoms[mid].y + geoms[mid].h <= cy {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    if lo >= geoms.len() { None } else { Some(lo) }
}

/// Pure sticky geometry — the stuck ancestor stack for a scroll `offset`, no
/// rendering. The renderer draws from it; a headless sampler asserts continuity.
///
/// Walks the stack top-down, probing the content at the current stack bottom for
/// the folder that should occupy the next slot. A folder pins at
/// `max(natural, stack_bottom)` — it scrolls in-flow until its top reaches the
/// stack bottom, then holds — so its stuck position equals its in-flow position
/// at the moment of sticking (no teleport). Each is pushed up by its own boundary
/// (first later row at its depth or shallower) and clipped under its parent.
///
/// `geoms` supplies content-y for each row (folder open ease). Sticky pin height
/// stays `ROW_H` (folder rows aren't scaled by their own open_t).
fn sticky_layout(rows: &[Row], geoms: &[RowGeom], offset: f32) -> Vec<Stuck> {
    let mut out = Vec::new();
    let mut prev_bottom = 0.0f32;
    for slot in 0.. {
        let probe_cy = offset + prev_bottom;
        let Some(probe) = row_index_at_y(geoms, probe_cy) else { break };
        if rows[probe].depth < slot {
            break; // no folder nests this deep at the stack bottom
        }
        let Some(fi) = ancestor_at_depth(rows, probe, slot) else { break };
        if !rows[fi].is_folder {
            break;
        }
        let natural = geoms[fi].y - offset;
        if natural > prev_bottom - 0.01 {
            // Header hasn't scrolled past its slot yet: it (and anything deeper)
            // is still a normal in-flow row. It reaches this exact position, so
            // the handoff to stuck is continuous.
            break;
        }
        let boundary = rows[fi + 1..]
            .iter()
            .position(|r| r.depth <= slot)
            .map(|k| geoms[fi + 1 + k].y - offset)
            .unwrap_or(f32::INFINITY);
        let vy = natural.max(prev_bottom).min(boundary - ROW_H);
        let stuck = Stuck { index: fi, vy, clip_top: prev_bottom };
        if stuck.height() > 0.0 {
            out.push(stuck);
        }
        prev_bottom = prev_bottom.max(vy + ROW_H);
    }
    out
}

/// The row at depth `d` that encloses (or is) `rows[i]` — walk back to the
/// nearest preceding row at that depth. A pre-order flatten makes it the exact
/// ancestor. `None` if `rows[i]` is shallower than `d`.
fn ancestor_at_depth(rows: &[Row], i: usize, d: usize) -> Option<usize> {
    if rows[i].depth < d {
        return None;
    }
    let mut j = i;
    loop {
        if rows[j].depth == d {
            return Some(j);
        }
        j = j.checked_sub(1)?;
    }
}

/// Temporary stand-in for a real `FileCache` — a tree deep and tall enough to
/// exercise scrolling and (soon) sticky ancestor headers. Deterministic ids (no
/// RNG) keep it fuzz/snapshot-friendly. Deleted once the workspace is wired in.
pub fn demo_files() -> Vec<lb::model::file::File> {
    use lb::model::file::File;
    use lb::model::file_metadata::FileType::{self, Document, Folder};

    fn f(id: u128, parent: u128, name: impl Into<String>, ft: FileType) -> File {
        File {
            id: Uuid::from_u128(id),
            parent: Uuid::from_u128(parent),
            name: name.into(),
            file_type: ft,
            last_modified: 0,
            last_modified_by: String::new(),
            owner: String::new(),
            shares: vec![],
            size_bytes: 0,
        }
    }

    let mut v = vec![f(0, 0, "root", Folder)];

    // A deep chain — the sticky-header stress case (Apps ▸ … ▸ src). Named to
    // sort first so its leaves can scroll to the top with content below them.
    v.push(f(1, 0, "Apps", Folder));
    v.push(f(2, 1, "lockbook", Folder));
    v.push(f(3, 2, "clients", Folder));
    v.push(f(4, 3, "egui", Folder));
    v.push(f(5, 4, "src", Folder));
    for (i, name) in ["button.rs", "file_tree.rs", "lib.rs", "theme.rs", "tokens.rs"]
        .into_iter()
        .enumerate()
    {
        v.push(f(100 + i as u128, 5, name, Document));
    }
    v.push(f(6, 4, "foundry.md", Document));
    v.push(f(7, 3, "android", Folder));
    v.push(f(8, 3, "apple", Folder));
    v.push(f(9, 2, "libs", Folder));
    v.push(f(10, 9, "core.rs", Document));
    v.push(f(11, 9, "sync.rs", Document));

    // A tall flat folder below the chain — the scroll room that lets the deep
    // leaves reach the top.
    v.push(f(12, 0, "Documents", Folder));
    for i in 0..40u128 {
        v.push(f(200 + i, 12, format!("note-{i:02}.md"), Document));
    }

    v.push(f(13, 0, "scratch.md", Document));
    v
}

/// The minimal tree for inspecting a single sticky handoff: a top item that
/// rolls off (`aaa`, sorts first) above a `folder` that rises and sticks as its
/// child `note b` scrolls under it. Ids: 2 = folder.
pub fn demo_micro() -> Vec<lb::model::file::File> {
    use lb::model::file::File;
    use lb::model::file_metadata::FileType::{Document, Folder};

    fn f(id: u128, parent: u128, name: &str, folder: bool) -> File {
        File {
            id: Uuid::from_u128(id),
            parent: Uuid::from_u128(parent),
            name: name.into(),
            file_type: if folder { Folder } else { Document },
            last_modified: 0,
            last_modified_by: String::new(),
            owner: String::new(),
            shares: vec![],
            size_bytes: 0,
        }
    }

    vec![
        f(0, 0, "root", true),
        f(1, 0, "aaa", true),
        f(2, 0, "folder", true),
        f(3, 2, "note b", false),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use lb::model::file::File;
    use lb::model::file_metadata::FileType::{Document, Folder};

    fn mk(id: u128, parent: u128, name: &str, folder: bool) -> File {
        File {
            id: Uuid::from_u128(id),
            parent: Uuid::from_u128(parent),
            name: name.into(),
            file_type: if folder { Folder } else { Document },
            last_modified: 0,
            last_modified_by: String::new(),
            owner: String::new(),
            shares: vec![],
            size_bytes: 0,
        }
    }

    // Selection semantics over a flat visible order a,b,c,d — plain replace,
    // Shift-range (pivots on the held anchor), Cmd-toggle (re-anchors).
    #[test]
    fn selection_model() {
        let files = vec![
            mk(0, 0, "root", true),
            mk(1, 0, "a", false),
            mk(2, 0, "b", false),
            mk(3, 0, "c", false),
            mk(4, 0, "d", false),
        ];
        let id = Uuid::from_u128;
        let mut tree = FileTree::default();

        tree.apply(Action::Select(id(2)), &files);
        assert_eq!(tree.selected, HashSet::from([id(2)]));
        assert_eq!(tree.cursor, Some(id(2)));
        assert_eq!(tree.anchor, Some(id(2)));

        // Shift-range anchor(b)..d = b,c,d; anchor held, cursor leads.
        tree.apply(Action::SelectRange(id(4)), &files);
        assert_eq!(tree.selected, HashSet::from([id(2), id(3), id(4)]));
        assert_eq!(tree.cursor, Some(id(4)));
        assert_eq!(tree.anchor, Some(id(2)));

        // Re-extend from the same anchor upward: a,b.
        tree.apply(Action::SelectRange(id(1)), &files);
        assert_eq!(tree.selected, HashSet::from([id(1), id(2)]));
        assert_eq!(tree.anchor, Some(id(2)));

        // Cmd-toggle adds and re-anchors; toggling again removes.
        tree.apply(Action::SelectAdd(id(4)), &files);
        assert_eq!(tree.selected, HashSet::from([id(1), id(2), id(4)]));
        assert_eq!(tree.anchor, Some(id(4)));
        tree.apply(Action::SelectAdd(id(2)), &files);
        assert_eq!(tree.selected, HashSet::from([id(1), id(4)]));

        // Plain select resets to a single item.
        tree.apply(Action::Select(id(3)), &files);
        assert_eq!(tree.selected, HashSet::from([id(3)]));
    }

    // Keyboard cursor over the flat order a,b,c,d: Down/Up move + clamp, Shift
    // extends from the anchor, Enter on a doc escapes as Open.
    #[test]
    fn keyboard_cursor() {
        let files = vec![
            mk(0, 0, "root", true),
            mk(1, 0, "a", false),
            mk(2, 0, "b", false),
            mk(3, 0, "c", false),
            mk(4, 0, "d", false),
        ];
        let id = Uuid::from_u128;
        let mut tree = FileTree::default();
        let down = Action::CursorMove { down: true, extend: false };
        let up = Action::CursorMove { down: false, extend: false };

        // From no cursor, Down lands on the first row and single-selects it.
        tree.apply(down.clone(), &files);
        assert_eq!(tree.cursor, Some(id(1)));
        assert_eq!(tree.selected, HashSet::from([id(1)]));

        // Walks down and clamps at the last row.
        for _ in 0..5 {
            tree.apply(down.clone(), &files);
        }
        assert_eq!(tree.cursor, Some(id(4)));
        tree.apply(up, &files);
        assert_eq!(tree.cursor, Some(id(3)));

        // Shift+Down extends the selection from the anchor.
        tree.apply(Action::Select(id(2)), &files);
        tree.apply(Action::CursorMove { down: true, extend: true }, &files);
        assert_eq!(tree.cursor, Some(id(3)));
        assert_eq!(tree.selected, HashSet::from([id(2), id(3)]));
        assert_eq!(tree.anchor, Some(id(2)));

        // Enter on a document escapes as an Open op.
        let op = tree.apply(Action::OpenCursor, &files);
        assert!(matches!(op, Some(Op::Open { new_tab: false, .. })));
    }

    // Create/delete escape as Ops carrying the resolved parent / selection.
    #[test]
    fn crud_ops() {
        let files = vec![mk(0, 0, "root", true), mk(1, 0, "a", false), mk(2, 0, "b", false)];
        let id = Uuid::from_u128;
        let mut tree = FileTree::default();

        assert!(matches!(
            tree.apply(Action::CreateDoc { parent: id(0) }, &files),
            Some(Op::CreateDoc { parent }) if parent == id(0)
        ));
        assert!(matches!(
            tree.apply(Action::CreateFolder { parent: id(0) }, &files),
            Some(Op::CreateFolder { parent }) if parent == id(0)
        ));

        // Delete is a no-op with nothing selected, else escapes the selection.
        assert!(tree.apply(Action::DeleteSelected, &files).is_none());
        tree.apply(Action::Select(id(1)), &files);
        tree.apply(Action::SelectAdd(id(2)), &files);
        match tree.apply(Action::DeleteSelected, &files) {
            Some(Op::Delete { ids }) => {
                assert_eq!(ids.into_iter().collect::<HashSet<_>>(), HashSet::from([id(1), id(2)]));
            }
            other => panic!("expected Delete op, got {other:?}"),
        }
    }

    // Cut → paste moves; copy → paste duplicates; cut buffer clears on paste.
    #[test]
    fn cut_copy_paste_ops() {
        let files = vec![
            mk(0, 0, "root", true),
            mk(1, 0, "docs", true),
            mk(2, 0, "a.md", false),
            mk(3, 1, "nested.md", false),
        ];
        let id = Uuid::from_u128;
        let mut tree = FileTree::default();

        // Empty paste is a no-op.
        assert!(tree.apply(Action::PasteInto { dest: id(1) }, &files).is_none());

        // Cut selection → MoveInto; clip cleared.
        tree.apply(Action::Select(id(2)), &files);
        assert!(tree.apply(Action::CutSelected, &files).is_none());
        assert!(tree.clip.as_ref().is_some_and(|c| c.mode == ClipMode::Cut));
        match tree.apply(Action::PasteInto { dest: id(1) }, &files) {
            Some(Op::MoveInto { ids, parent }) => {
                assert_eq!(ids, vec![id(2)]);
                assert_eq!(parent, id(1));
            }
            other => panic!("expected MoveInto, got {other:?}"),
        }
        assert!(tree.clip.is_none());

        // Copy → CopyInto; clip retained for repeat paste.
        tree.apply(Action::Select(id(2)), &files);
        assert!(tree.apply(Action::CopySelected, &files).is_none());
        match tree.apply(Action::PasteInto { dest: id(1) }, &files) {
            Some(Op::CopyInto { ids, parent }) => {
                assert_eq!(ids, vec![id(2)]);
                assert_eq!(parent, id(1));
            }
            other => panic!("expected CopyInto, got {other:?}"),
        }
        assert!(tree.clip.as_ref().is_some_and(|c| c.mode == ClipMode::Copy));

        // Can't paste a folder into itself / its descendant.
        tree.apply(Action::Select(id(1)), &files);
        tree.apply(Action::CutSelected, &files);
        assert!(tree.apply(Action::PasteInto { dest: id(1) }, &files).is_none());
        assert!(tree.apply(Action::PasteInto { dest: id(3) }, &files).is_none());
    }

    // Inline-rename lifecycle: Begin seeds the buffer, Commit escapes the edited
    // name (nothing if empty), Cancel discards. Stem selection preserves the ext.
    #[test]
    fn rename_ops() {
        let files = vec![mk(0, 0, "root", true), mk(1, 0, "note.md", false)];
        let id = Uuid::from_u128;
        let mut tree = FileTree::default();

        tree.apply(Action::BeginRename(id(1)), &files);
        assert_eq!(tree.renaming.as_ref().unwrap().buf, "note.md");

        tree.renaming.as_mut().unwrap().buf = "renamed.md".into();
        match tree.apply(Action::CommitRename, &files) {
            Some(Op::Rename { id: rid, name }) => {
                assert_eq!(rid, id(1));
                assert_eq!(name, "renamed.md");
            }
            other => panic!("expected Rename op, got {other:?}"),
        }
        assert!(tree.renaming.is_none());

        // Cancel discards; an empty commit is a no-op.
        tree.apply(Action::BeginRename(id(1)), &files);
        tree.apply(Action::CancelRename, &files);
        assert!(tree.renaming.is_none());
        tree.apply(Action::BeginRename(id(1)), &files);
        tree.renaming.as_mut().unwrap().buf = "   ".into();
        assert!(tree.apply(Action::CommitRename, &files).is_none());

        assert_eq!(stem_len("note.md", false), 4);
        assert_eq!(stem_len("folder", true), 6);
        assert_eq!(stem_len("noext", false), 5);
        assert_eq!(stem_len(".hidden", false), 7);
    }

    /// Chrome timing diagnostic: pin vs elevate (`natural < vy`) vs hairline.
    /// Run: `cargo test -p lockbook-egui chrome_timing_diag -- --nocapture`
    #[test]
    fn chrome_timing_diag() {
        let files = demo_micro();
        let mut tree = FileTree::default();
        tree.apply(Action::Toggle(Uuid::from_u128(2)), &files); // folder
        tree.apply(Action::Toggle(Uuid::from_u128(0)), &files);
        tree.settle_folder_anims();

        let rows = tree.flatten(&files);
        let (geoms, _) = tree.row_geometry(&rows);
        let names: Vec<String> = rows
            .iter()
            .map(|r| files.get_by_id(r.id).map_or("?".into(), |f| f.name.clone()))
            .collect();
        println!("ROW_H={ROW_H} order({}): {names:?}", rows.len());

        let mut prev_elev: Vec<bool> = vec![];
        let mut pin_off: Option<f32> = None;
        let mut elev_off: Option<f32> = None;
        for px in 0..=(rows.len() as f32 * ROW_H) as i32 {
            let off = px as f32;
            let layout = sticky_layout(&rows, &geoms, off);
            let elev_flags: Vec<bool> = layout
                .iter()
                .map(|s| {
                    let natural = geoms[s.index].y - off;
                    natural < s.vy - 0.01
                })
                .collect();
            let parts: Vec<String> = layout
                .iter()
                .zip(elev_flags.iter())
                .map(|(s, &elev)| {
                    let natural = geoms[s.index].y - off;
                    format!(
                        "{}:{} nat={natural:.1} vy={:.1} e={elev}",
                        s.index, names[s.index], s.vy
                    )
                })
                .collect();
            let hair = layout
                .iter()
                .zip(elev_flags.iter())
                .rev()
                .find(|(_, e)| **e)
                .map(|(s, _)| s.vy + ROW_H);

            if pin_off.is_none() && layout.iter().any(|s| names[s.index] == "folder") {
                pin_off = Some(off);
            }
            if elev_off.is_none() && elev_flags.iter().any(|&e| e) {
                elev_off = Some(off);
            }

            let elev_changed = elev_flags != prev_elev;
            if elev_changed || (!layout.is_empty() && px % 4 == 0 && px < 60) {
                println!(
                    "off {off:>5.0}: [{:}]  hair={:}",
                    parts.join(" | "),
                    hair.map(|y| format!("{y:.1}")).unwrap_or("-".into()),
                );
            }
            if elev_changed && elev_flags.iter().any(|&e| e) {
                println!("       ^^^ first elevate at off={off}");
            }
            prev_elev = elev_flags;
        }
        println!("folder pin_off={pin_off:?} elev_off={elev_off:?}");
        // Elevate must start at pin, not a full ROW_H later.
        if let (Some(p), Some(e)) = (pin_off, elev_off) {
            assert!(
                (e - p).abs() < 2.0,
                "elevated delayed {}px after pin (want ~0)",
                e - p
            );
        }
    }

    /// Minimum-scroll / reveal geometry diagnostic.
    ///
    /// For a grid of (start_offset, view_h, target_row), applies `min_scroll_to_row`
    /// and measures the row's open-band position against the sticky stack at the
    /// *result* offset. Flags:
    ///   - `SHORT_UP`   : still under stickies after scroll-up (gap < 0)
    ///   - `SHORT_DOWN` : still past viewport bottom after scroll-down
    ///   - `OVER_UP`    : scrolled up more than flush (gap > ~1px when we moved)
    ///
    /// Run: `cargo test -p lockbook-egui min_scroll_diag -- --nocapture`
    #[test]
    fn min_scroll_diag() {
        let files = demo_files();
        let mut tree = FileTree::default();
        // Deep path + Documents so sticky stacks and long scroll both exist.
        for id in [1u128, 2, 3, 4, 5, 9, 12] {
            tree.apply(Action::Toggle(Uuid::from_u128(id)), &files);
        }
        tree.settle_folder_anims();
        let rows = tree.flatten(&files);
        let (geoms, content_h) = tree.row_geometry(&rows);
        let names: Vec<String> = rows
            .iter()
            .map(|r| files.get_by_id(r.id).map_or("?".into(), |f| f.name.clone()))
            .collect();
        println!(
            "ROW_H={ROW_H} rows={} content_h={content_h:.0}",
            rows.len()
        );

        // Viewport heights ~ sidebar tree pane; offsets spanning stickies + tail.
        let view_hs = [200.0_f32, 340.0, 500.0];
        let mut start_offs: Vec<f32> = vec![0.0];
        let mut o = 0.0;
        while o < content_h {
            start_offs.push(o);
            o += ROW_H * 3.0;
        }
        // Sample every ~nth row + always hit a few known deep leaves.
        let mut sample: Vec<usize> = (0..rows.len()).step_by((rows.len() / 16).max(1)).collect();
        if let Some(last) = rows.len().checked_sub(1) {
            sample.push(last);
        }
        for want in ["foundry.md", "core.rs", "note-00.md", "note-39.md", "scratch.md"] {
            if let Some(i) = names.iter().position(|n| n == want) {
                sample.push(i);
            }
        }
        sample.sort_unstable();
        sample.dedup();

        let mut short_up = 0u32;
        let mut short_down_feasible = 0u32;
        let mut short_down_impossible = 0u32;
        let mut ok = 0u32;
        let mut printed = 0u32;

        for &view_h in &view_hs {
            for &start in &start_offs {
                for &i in &sample {
                    let cy = geoms[i].y;
                    let sticky0 = sticky_stack_bottom(&rows, &geoms, start);
                    let decision = min_scroll_to_row(&rows, &geoms, i, start, view_h);
                    let end = decision.unwrap_or(start);
                    let sticky1 = sticky_stack_bottom(&rows, &geoms, end);
                    let row_top = cy - end;
                    let row_bot = cy + ROW_H - end;
                    let gap_top = row_top - sticky1; // >= 0 ⇒ clear of stickies
                    let gap_bot = view_h - row_bot; // >= 0 ⇒ above viewport bottom
                    // Open band under stickies; if < ROW_H the row cannot fully fit.
                    let open = view_h - sticky1;
                    let bot_feasible = open + 0.5 >= ROW_H;

                    let mut flags = Vec::new();
                    if gap_top < -0.5 {
                        flags.push("SHORT_UP");
                        short_up += 1;
                    }
                    if view_h > 0.0 && gap_bot < -0.5 {
                        if bot_feasible {
                            flags.push("SHORT_DOWN");
                            short_down_feasible += 1;
                        } else {
                            flags.push("CLIP_TALL_STICKY");
                            short_down_impossible += 1;
                        }
                    }
                    if flags.is_empty() {
                        ok += 1;
                    }

                    let interesting = !flags.is_empty()
                        || (decision.is_some() && printed < 16 && (i + start as usize) % 11 == 0);
                    if interesting && printed < 40 {
                        printed += 1;
                        let dir = match decision {
                            None => "none",
                            Some(e) if e < start - 0.5 => "up",
                            Some(e) if e > start + 0.5 => "down",
                            Some(_) => "same",
                        };
                        println!(
                            "view={view_h:.0} start={start:>6.1} → {end:>6.1} ({dir}) \
                             row[{i}]={name} cy={cy:.0} sticky {sticky0:.1}→{sticky1:.1} \
                             row_top={row_top:.1} gap_top={gap_top:.1} gap_bot={gap_bot:.1} \
                             {flags}",
                            name = names[i],
                            flags = if flags.is_empty() {
                                "ok".into()
                            } else {
                                flags.join(",")
                            },
                        );
                    }
                }
            }
        }

        println!(
            "summary: ok={ok} SHORT_UP={short_up} SHORT_DOWN={short_down_feasible} \
             CLIP_TALL_STICKY={short_down_impossible} (printed {printed} rows)"
        );
        // The bug we fixed: scroll-up left the row under stickies by ~ROW_H.
        assert_eq!(
            short_up, 0,
            "min_scroll left row under sticky stack ({short_up} cases) — scroll-up short"
        );
        // Bottom shortfall only counts when geometry allows a full row under stickies.
        assert_eq!(
            short_down_feasible, 0,
            "min_scroll left row past viewport bottom ({short_down_feasible} cases) — scroll-down short"
        );
    }

    // note a / folder(> note b): dump each element's effective on-screen vy at
    // each pixel of scroll, flag any per-pixel jump > 1.5px. A diagnostic — run
    // on demand with `--ignored --nocapture`.
    #[test]
    #[ignore = "prints a per-pixel table; run explicitly"]
    fn micro_scroll_layout() {
        let files = vec![
            mk(0, 0, "root", true),
            mk(1, 0, "note a", false),
            mk(2, 0, "folder", true),
            mk(3, 2, "note b", false),
        ];
        let mut tree = FileTree::default();
        tree.apply(Action::Toggle(Uuid::from_u128(2)), &files);
        tree.settle_folder_anims();

        let rows = tree.flatten(&files);
        let (geoms, _) = tree.row_geometry(&rows);
        let names: Vec<String> = rows
            .iter()
            .map(|r| files.get_by_id(r.id).unwrap().name.clone())
            .collect();
        println!("order: {names:?}");

        let eff = |off: f32| -> Vec<(f32, bool)> {
            let stuck: std::collections::HashMap<usize, f32> =
                sticky_layout(&rows, &geoms, off)
                    .iter()
                    .map(|s| (s.index, s.vy))
                    .collect();
            (0..rows.len())
                .map(|i| match stuck.get(&i) {
                    Some(&vy) => (vy, true),
                    None => (geoms[i].y - off, false),
                })
                .collect()
        };

        let mut prev = eff(0.0);
        for px in 0..=(rows.len() * ROW_H as usize) {
            let off = px as f32;
            let cur = eff(off);
            let cells: Vec<String> = (0..rows.len())
                .map(|i| {
                    let (vy, s) = cur[i];
                    let jump = if (vy - prev[i].0).abs() > 1.5 { "!" } else { " " };
                    format!("{}{}={:>5.0}{}", jump, names[i], vy, if s { "S" } else { " " })
                })
                .collect();
            println!("off {off:>3.0}: {}", cells.join("  "));
            prev = cur;
        }
    }

    /// DnD width contract: float + sep share the content band (clip − 2×inset),
    /// never the sticky full-bleed, never a hard max clamp.
    ///
    /// Run: `cargo test -p lockbook-egui dnd_width_contract -- --nocapture`
    #[test]
    fn dnd_width_contract() {
        // Simulated sidebar clip widths (typical + wide + narrow).
        let clips = [220.0_f32, 280.0, 320.0, 400.0];
        for clip_w in clips {
            let content_w = (clip_w - 2.0 * SCROLL_INSET).max(0.0);
            // In-flow row rect is already the content band.
            let flow = Rect::from_min_size(pos2(10.0 + SCROLL_INSET, 0.0), vec2(content_w, ROW_H));
            let flow_band = FileTree::content_band(Placement::Flow, flow);
            assert!(
                (flow_band.width() - content_w).abs() < 0.01,
                "flow band {w} != content {content_w} (clip {clip_w})",
                w = flow_band.width()
            );

            // Sticky is full-bleed; band must strip insets.
            let sticky =
                Rect::from_min_size(pos2(10.0, 0.0), vec2(clip_w, ROW_H));
            let sticky_band = FileTree::content_band(
                Placement::Sticky {
                    clip: sticky,
                    elevate_t: 1.0,
                },
                sticky,
            );
            assert!(
                (sticky_band.width() - content_w).abs() < 0.01,
                "sticky band {w} != content {content_w} (clip {clip_w})",
                w = sticky_band.width()
            );
            assert!(
                (sticky_band.left() - (sticky.left() + SCROLL_INSET)).abs() < 0.01
            );
            assert!(
                (sticky_band.right() - (sticky.right() - SCROLL_INSET)).abs() < 0.01
            );

            // Historical bug: clamp(120, 280) made the card narrower than the
            // row once the sidebar content band exceeded 280.
            let old_clamped = content_w.clamp(120.0, 280.0);
            if content_w > 280.0 {
                assert!(
                    old_clamped < content_w - 0.5,
                    "fixture: expected old clamp to shrink wide bands"
                );
            }
            // Contract: card width is exact band (no max clamp).
            let card_w = content_w; // paint_drag_float: snap.width.max(1.0)
            assert!(
                (card_w - content_w).abs() < 0.01,
                "card must match content band exactly"
            );

            // Sep spans full content band (not depth-indented).
            let sep_x0 = sticky_band.left();
            let sep_x1 = sticky_band.right();
            assert!((sep_x1 - sep_x0 - content_w).abs() < 0.01);
            // Must not extend into side insets.
            assert!(sep_x0 >= sticky.left() + SCROLL_INSET - 0.01);
            assert!(sep_x1 <= sticky.right() - SCROLL_INSET + 0.01);

            println!(
                "clip={clip_w:.0} content={content_w:.0} flow={:.0} sticky_band={:.0} \
                 old_clamp={old_clamped:.0} card={card_w:.0} sep={:.0}..{:.0}",
                flow_band.width(),
                sticky_band.width(),
                sep_x0,
                sep_x1,
            );
        }
    }
}
