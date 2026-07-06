//! The file tree — the first feature widget, and the template for the whole
//! action-surface pattern. `show` draws from a borrowed `FilesExt` view and
//! turns input into `Action`s; every `Action` routes through `apply`, the one
//! chokepoint. Navigation (select, expand) folds into widget-local view state;
//! model mutations escape as an `Op` the shell runs against the workspace. That
//! chokepoint is what makes the surface scriptable and fuzzable — a driver
//! builds a `Vec<File>` and drives `apply`, no GUI required.
//!
//! Rendering runs off a *flattened* row list — a pre-order walk of expanded
//! nodes, positioned analytically at a uniform row height. That one model backs
//! virtualization, sticky headers, keyboard nav, and reveal-to-selection alike;
//! the tree's geometry lives in the list, not in the recursion.
//!
//! Wired so far: select, expand/collapse, open, virtualized scroll, sticky
//! ancestor headers. Deferred (each a future `Action`/`Op` pair): rename,
//! move/drag, cut/paste, multiselect, delete, duplicate, pin, sync dots,
//! virtual folders, keyboard nav.

use std::collections::HashSet;

use egui::{FontId, Id, Pos2, Rect, Sense, Ui, pos2, vec2};
use lb::Uuid;
use workspace_rs::file_cache::FilesExt;

use crate::theme::icons;
use crate::theme::tokens::Tokens;

/// Uniform row height. Load-bearing: analytic positioning (and thus the whole
/// sticky/virtualization math) assumes every row is exactly this tall.
const ROW_H: f32 = 26.0;

/// Left pad plus per-depth indent, in points.
const INDENT_BASE: f32 = 12.0;
const INDENT_STEP: f32 = 16.0;

/// The complete input vocabulary. Mouse, keyboard, and (later) fuzz/CLI all
/// produce `Action`s; nothing mutates tree state except `apply`.
#[derive(Clone, Debug)]
pub enum Action {
    /// Make `id` the single selection.
    Select(Uuid),
    /// Flip a folder's expansion.
    Toggle(Uuid),
    /// Open a document — escapes to the shell.
    Open { id: Uuid, new_tab: bool },
    /// Set the scroll offset. Scroll is view state like selection, so it's part
    /// of the surface — scriptable, observable, fuzzable — not a side channel.
    ScrollTo(f32),
}

/// A model mutation the tree can't perform itself; the shell fulfills it against
/// the workspace. The shell's own action enum is composed from these.
#[derive(Clone, Debug)]
pub enum Op {
    Open { id: Uuid, new_tab: bool },
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
#[derive(Clone, Copy)]
struct View {
    origin: Pos2,
    width: f32,
    offset: f32,
}

/// How a row is laid out: normal in-flow, or a stuck header clipped under its
/// parent (`clip`) so its pushed-up portion slides behind rather than over.
#[derive(Clone, Copy)]
enum Placement {
    Flow,
    Sticky { clip: Rect },
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
            Placement::Sticky { clip } => Some(clip),
        }
    }
    fn opaque(self) -> bool {
        matches!(self, Placement::Sticky { .. })
    }
}

#[derive(Default)]
pub struct FileTree {
    expanded: HashSet<Uuid>,
    selected: Option<Uuid>,
    /// Set by `Action::ScrollTo`, forces the scroll offset for the next frame
    /// then clears. `None` leaves egui in charge (user wheel/drag). The one-shot
    /// jump both scripts scroll for observation and, later, backs reveal-to-file.
    forced_offset: Option<f32>,
}

impl FileTree {
    /// The one chokepoint. Navigation folds into view state and returns `None`;
    /// model mutations return the `Op` for the shell to run.
    pub fn apply(&mut self, action: Action) -> Option<Op> {
        match action {
            Action::Select(id) => self.selected = Some(id),
            Action::Toggle(id) => {
                if !self.expanded.remove(&id) {
                    self.expanded.insert(id);
                }
            }
            Action::ScrollTo(y) => self.forced_offset = Some(y.max(0.0)),
            Action::Open { id, new_tab } => return Some(Op::Open { id, new_tab }),
        }
        None
    }

    /// Pre-order walk of expanded nodes into the positioned row list. Pure over
    /// `(expanded, files)` — no pixels — so the same list drives rendering, the
    /// readout, and (later) sticky/keyboard geometry.
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

    /// Draw the tree and return the frame's escaping `Op` (a click yields at most
    /// one). Only the rows overlapping the viewport are drawn; the ancestor chain
    /// of the topmost row is then re-drawn as a stuck header stack over the top.
    pub fn show(&mut self, ui: &mut Ui, t: &Tokens, files: &impl FilesExt) -> Option<Op> {
        let rows = self.flatten(files);
        let total_h = rows.len() as f32 * ROW_H;
        let mut escaped = None;

        let mut area = egui::ScrollArea::vertical().auto_shrink([false, false]);
        if let Some(y) = self.forced_offset.take() {
            area = area.vertical_scroll_offset(y);
        }
        area.show_viewport(ui, |ui, viewport| {
            let clip = ui.clip_rect();
            let width = clip.width();
            let offset = viewport.min.y;
            // Reserve the full scroll extent (for the scrollbar).
            ui.allocate_exact_size(vec2(width, total_h), Sense::hover());
            // Anchor in-flow rows *and* stuck headers to the true viewport top
            // with the exact (unrounded) offset — `origin` is the screen y of
            // content-y 0. egui's own content origin is rounded to a whole pixel,
            // which would drift against the sub-pixel-smooth sticky pass and make
            // the in-flow↔stuck handoff jump under fractional scroll.
            let view = View { origin: pos2(clip.left(), clip.top() - offset), width, offset };

            // Stuck rows are drawn pinned by the sticky pass; the in-flow pass
            // skips them so they aren't also drawn at their scrolled position.
            let layout = sticky_layout(&rows, view.offset);
            let stuck: HashSet<usize> = layout.iter().map(|s| s.index).collect();

            // In-flow, virtualized: only the rows overlapping the viewport.
            let first = (view.offset / ROW_H).floor().max(0.0) as usize;
            let last = ((viewport.max.y / ROW_H).ceil() as usize).min(rows.len());
            for (i, &row) in rows.iter().enumerate().take(last).skip(first) {
                if stuck.contains(&i) {
                    continue;
                }
                let top = view.origin.y + i as f32 * ROW_H;
                let rect = Rect::from_min_size(pos2(view.origin.x, top), vec2(width, ROW_H));
                if let Some(op) = self.row(ui, t, files, row, rect, Placement::Flow) {
                    escaped = Some(op);
                }
            }

            // The stuck ancestor stack over the top.
            if let Some(op) = self.sticky(ui, t, files, &rows, view, &layout) {
                escaped = Some(op);
            }
        });
        escaped
    }

    /// Draw the stuck ancestor stack from a precomputed `layout` (see
    /// `sticky_layout`), clipping each header under its parent so a pushed one
    /// slides behind rather than over. Anchored to the same `view` origin as the
    /// in-flow rows (`origin.y + offset` == the viewport top), so a header's
    /// stuck position lines up exactly with where it sat in flow.
    fn sticky(
        &mut self, ui: &mut Ui, t: &Tokens, files: &impl FilesExt, rows: &[Row], view: View,
        layout: &[Stuck],
    ) -> Option<Op> {
        if layout.is_empty() {
            return None;
        }
        let x = view.origin.x;
        let width = view.width;
        let vtop = view.origin.y + view.offset;

        let mut escaped = None;
        for s in layout {
            let top = vtop + s.vy;
            let rect = Rect::from_min_size(pos2(x, top), vec2(width, ROW_H));
            let clip = Rect::from_min_max(pos2(x, vtop + s.clip_top), pos2(x + width, top + ROW_H));
            if let Some(op) =
                self.row(ui, t, files, rows[s.index], rect, Placement::Sticky { clip })
            {
                escaped = Some(op);
            }
        }
        escaped
    }

    /// Paint and interact one row at an absolute rect (virtualized layout owns
    /// positioning, so this doesn't use egui flow). A `Sticky` placement fills an
    /// opaque background and clips its top under the parent header; the id is
    /// namespaced so a stuck header and its (offscreen) in-flow twin don't
    /// collide. Input → `Action` → `apply`, so a click matches the scripted one.
    fn row(
        &mut self, ui: &mut Ui, t: &Tokens, files: &impl FilesExt, row: Row, rect: Rect,
        place: Placement,
    ) -> Option<Op> {
        let file = files.get_by_id(row.id)?;
        let expanded = self.expanded.contains(&row.id);
        let selected = self.selected == Some(row.id);

        // A stuck header draws through a clipped painter so its pushed-up portion
        // slides behind its parent; interaction follows the visible region.
        let painter = match place.clip() {
            Some(c) => ui.painter().with_clip_rect(c.intersect(ui.clip_rect())),
            None => ui.painter().clone(),
        };
        let hit = place.clip().map_or(rect, |c| rect.intersect(c));
        let resp = ui.interact(hit, Id::new((place.ns(), row.id)), Sense::click());
        let hover = ui.ctx().animate_bool(resp.id, resp.hovered());

        if place.opaque() {
            painter.rect_filled(rect, 0.0, t.surface());
        }
        // State layer — neutral, matching the button: selected reads as a resting
        // fill, hover eases in over it. No hue.
        let fill = if selected {
            t.fg().gamma_multiply(0.10)
        } else {
            t.fg().gamma_multiply(0.05 * hover)
        };
        if fill.a() > 0 {
            painter.rect_filled(rect, 5.0, fill);
        }

        let cy = rect.center().y;
        let mut x = rect.left() + INDENT_BASE + row.depth as f32 * INDENT_STEP;
        let ink = if selected { t.fg() } else { t.text_muted() };

        // Type icon: an open/closed folder, or a file. Expansion reads from the
        // icon itself, so there's no separate caret.
        let icon = if row.is_folder {
            if expanded { icons::FOLDER_OPEN } else { icons::FOLDER }
        } else {
            icons::FILE
        };
        let g = painter.layout_no_wrap(icon.into(), icons::font(16.0), ink);
        painter.galley(pos2(x, cy - g.size().y / 2.0), g, ink);
        x += 24.0;

        let g = painter.layout_no_wrap(file.name.clone(), FontId::proportional(14.0), ink);
        painter.galley(pos2(x, cy - g.size().y / 2.0), g, ink);

        // Selection first, then the type-specific act; both go through the
        // chokepoint so a driver sees identical effects.
        if resp.clicked() {
            let new_tab = ui.input(|i| i.modifiers.command);
            let act = if row.is_folder {
                Action::Toggle(row.id)
            } else {
                Action::Open { id: row.id, new_tab }
            };
            let mut op = None;
            for a in [Action::Select(row.id), act] {
                if let Some(o) = self.apply(a) {
                    op = Some(o);
                }
            }
            return op;
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
            let sel = if self.selected == Some(row.id) { "  ◂ selected" } else { "" };
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
        sticky_layout(&rows, offset)
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
        let pinned: std::collections::HashMap<usize, f32> = sticky_layout(&rows, offset)
            .iter()
            .map(|s| (s.index, s.vy))
            .collect();
        (0..rows.len())
            .map(|i| (i, pinned.get(&i).copied().unwrap_or(i as f32 * ROW_H - offset)))
            .filter(|&(_, vy)| vy > -ROW_H && vy < viewport)
            .collect()
    }
}

/// Sorted child ids, materialized so the `files` borrow doesn't straddle the
/// recursive `&mut self` call.
fn child_ids(files: &impl FilesExt, id: Uuid) -> Vec<Uuid> {
    files.children(id).iter().map(|f| f.id).collect()
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

/// Pure sticky geometry — the stuck ancestor stack for a scroll `offset`, no
/// rendering. The renderer draws from it; a headless sampler asserts continuity.
///
/// Walks the stack top-down, probing the content at the current stack bottom for
/// the folder that should occupy the next slot. A folder pins at
/// `max(natural, stack_bottom)` — it scrolls in-flow until its top reaches the
/// stack bottom, then holds — so its stuck position equals its in-flow position
/// at the moment of sticking (no teleport). Each is pushed up by its own boundary
/// (first later row at its depth or shallower) and clipped under its parent.
fn sticky_layout(rows: &[Row], offset: f32) -> Vec<Stuck> {
    let mut out = Vec::new();
    let mut prev_bottom = 0.0f32;
    for slot in 0.. {
        let probe = ((offset + prev_bottom) / ROW_H).floor();
        if probe < 0.0 || probe as usize >= rows.len() {
            break;
        }
        let probe = probe as usize;
        if rows[probe].depth < slot {
            break; // no folder nests this deep at the stack bottom
        }
        let Some(fi) = ancestor_at_depth(rows, probe, slot) else { break };
        if !rows[fi].is_folder {
            break;
        }
        let natural = fi as f32 * ROW_H - offset;
        if natural > prev_bottom - 0.01 {
            // Header hasn't scrolled past its slot yet: it (and anything deeper)
            // is still a normal in-flow row. It reaches this exact position, so
            // the handoff to stuck is continuous.
            break;
        }
        let boundary = rows[fi + 1..]
            .iter()
            .position(|r| r.depth <= slot)
            .map(|k| (fi + 1 + k) as f32 * ROW_H - offset)
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
    for (i, name) in ["button.rs", "design_system.rs", "file_tree.rs", "lib.rs", "theme.rs"]
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
        tree.apply(Action::Toggle(Uuid::from_u128(2)));

        let rows = tree.flatten(&files);
        let names: Vec<String> = rows
            .iter()
            .map(|r| files.get_by_id(r.id).unwrap().name.clone())
            .collect();
        println!("order: {names:?}");

        let eff = |off: f32| -> Vec<(f32, bool)> {
            let stuck: std::collections::HashMap<usize, f32> = sticky_layout(&rows, off)
                .iter()
                .map(|s| (s.index, s.vy))
                .collect();
            (0..rows.len())
                .map(|i| match stuck.get(&i) {
                    Some(&vy) => (vy, true),
                    None => (i as f32 * ROW_H - off, false),
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
}
