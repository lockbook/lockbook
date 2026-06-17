//! Drag-to-reorder for list items. A per-frame geometry index
//! ([`BlockBox`]) mirrors `fragments`: cleared each frame, filled during
//! the render DFS, read for pointer hit-testing and drop-gap math.
//!
//! v1 covers list items only (`Item` and `TaskItem`) and reorders among
//! siblings under the same list. No reparenting. The release is a pure
//! text op: rewrite the dragged item's sibling run with the items
//! permuted — computed by [`MdRender::plan_block_move`] so tests can
//! exercise it without simulating a mouse drag.

use comrak::nodes::{AstNode, NodeValue};
use egui::{CursorIcon, Pos2, Rect, Ui, Vec2};
use lb_rs::model::text::offset_types::{Grapheme, Graphemes, RangeExt as _};

use crate::tab::markdown_editor::MdRender;

/// What a list-item marker reported this frame, for `MdEdit::show` to act on.
#[derive(Clone, Copy, Debug)]
pub enum BlockDragAction {
    Started(BlockDrag),
    Dragged(Pos2),
    Released(Pos2),
}

/// One reorderable list item's geometry for the frame.
#[derive(Clone, Copy, Debug)]
pub struct BlockBox {
    /// The item node's source range. Containers' children (a nested
    /// list, more items) live inside this range — moving the item moves
    /// them too, no separate section field required.
    pub node_range: (Grapheme, Grapheme),
    /// Painted extent of the item.
    pub rect: Rect,
    /// `node_range.start` of this item's parent `List` — the sibling
    /// group identifier. Two items are reorder siblings iff their
    /// `parent_start` matches.
    pub parent_start: Grapheme,
}

/// State held across frames while a list-item drag is in flight.
#[derive(Clone, Copy, Debug)]
pub struct BlockDrag {
    /// The full dragged span — one item, or several selected sibling items.
    pub section_range: (Grapheme, Grapheme),
    /// The node range of the specific item whose marker was grabbed.
    pub grabbed: (Grapheme, Grapheme),
    pub parent_start: Grapheme,
    /// Vector from the source item's top-left to the grab pointer at
    /// grab time — scroll-invariant, unlike a stored screen-space
    /// `origin`. Used to anchor the floating render (`pointer -
    /// grab_offset` is the floating card's top-left, regardless of how
    /// far the doc has scrolled since) and to position the drop
    /// indicator.
    pub grab_offset: Vec2,
}

/// A resolved drop target within the dragged item's sibling group.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DropGap {
    /// The source offset at which the dragged item's text is inserted.
    pub insert_offset: Grapheme,
    /// Index of the gap among the sibling group (0 = before first item).
    pub gap_index: usize,
    /// Y of the insertion line indicator.
    pub y: i32,
}

impl<'ast> MdRender {
    /// Whether `node` is an `Item`/`TaskItem` belonging to a permutable
    /// list — its parent has ≥2 item children. An only-child item
    /// affords no reorder, so it gets no handle.
    pub fn is_reorderable_item(&self, node: &'ast AstNode<'ast>) -> bool {
        if !matches!(node.data.borrow().value, NodeValue::Item(_) | NodeValue::TaskItem(_)) {
            return false;
        }
        let Some(parent) = node.parent() else { return false };
        parent
            .children()
            .filter(|c| {
                matches!(c.data.borrow().value, NodeValue::Item(_) | NodeValue::TaskItem(_))
            })
            .nth(1)
            .is_some()
    }

    /// The range a drag should carry: when `node` is part of a multi-item
    /// selection, the union of the selected sibling items (guaranteed
    /// contiguous); otherwise just `node`'s own node range.
    pub(crate) fn drag_span(&self, node: &'ast AstNode<'ast>) -> (Grapheme, Grapheme) {
        let own = self.node_range(node);
        if !self.selected_block(node) {
            return own;
        }
        let Some(parent) = node.parent() else { return own };
        let mut span = own;
        for c in parent.children() {
            let is_item =
                matches!(c.data.borrow().value, NodeValue::Item(_) | NodeValue::TaskItem(_));
            if is_item && self.selected_block(c) {
                let r = self.node_range(c);
                span = (span.0.min(r.0), span.1.max(r.1));
            }
        }
        span
    }

    /// Push an item box for the frame's geometry index. Called from the
    /// render DFS for every reorderable item child.
    pub fn push_block_box(
        &mut self, node: &'ast AstNode<'ast>, rect: Rect, parent_start: Grapheme,
    ) {
        let node_range = self.node_range(node);
        self.block_boxes
            .push(BlockBox { node_range, rect, parent_start });
    }

    /// Process the marker's interaction response into a drag action and
    /// the right cursor icon. Shared by `show_item` (bullet/ordered
    /// marker) and `show_task_item` (checkbox response). Early-returns
    /// for non-reorderable items (only child) and in read-only mode.
    pub fn handle_item_drag_resp(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, resp: &egui::Response,
    ) {
        if self.readonly || !self.interactive || !self.is_reorderable_item(node) {
            return;
        }
        let pointer = ui.input(|i| i.pointer.latest_pos());
        let dragging_this = {
            let r = self.node_range(node);
            self.in_progress_block_drag.is_some_and(|d| {
                d.section_range.start() <= r.start() && r.end() <= d.section_range.end()
            })
        };
        let active = resp.hovered() || dragging_this;
        if active {
            let grabbing = resp.dragged() || dragging_this;
            ui.output_mut(|o| {
                o.cursor_icon = if grabbing { CursorIcon::Grabbing } else { CursorIcon::Grab };
            });
        }
        if resp.drag_started() {
            let origin = pointer.unwrap_or(resp.rect.center());
            let Some(parent) = node.parent() else { return };
            let parent_start = self.node_range(parent).start();
            let section_range = self.drag_span(node);
            // Measure `grab_offset` from the *section's* top-left, not
            // the grabbed item's. For a multi-item drag the section can
            // span many items above the grabbed one; `draw_dragged_overlay`
            // positions the floating card by `pointer - grab_offset`, so
            // anchoring on the section top keeps the grabbed item under
            // the cursor regardless of where in the run it sits.
            // `section_rect` reads `block_boxes`, which DFS-populates top-
            // to-bottom, so by the time the grabbed item fires
            // `drag_started`, every other in-span item is already
            // indexed.
            let span_top_left = self
                .section_rect(section_range)
                .map(|r| r.left_top())
                .unwrap_or(resp.rect.left_top());
            let grab_offset = origin - span_top_left;
            self.block_drag_action = Some(BlockDragAction::Started(BlockDrag {
                section_range,
                grabbed: self.node_range(node),
                parent_start,
                grab_offset,
            }));
        } else if resp.drag_stopped() {
            if let Some(p) = pointer {
                self.block_drag_action = Some(BlockDragAction::Released(p));
            }
        } else if resp.dragged() {
            if let Some(p) = pointer {
                self.block_drag_action = Some(BlockDragAction::Dragged(p));
            }
        }
    }

    /// Union of the painted rects of every indexed item within `span` —
    /// the on-screen extent of what a drag moves. Valid once the frame's
    /// geometry index is fully populated.
    pub fn section_rect(&self, span: (Grapheme, Grapheme)) -> Option<Rect> {
        let mut rect: Option<Rect> = None;
        for b in &self.block_boxes {
            if b.node_range.start() >= span.start() && b.node_range.end() <= span.end() {
                rect = Some(rect.map_or(b.rect, |r: Rect| r.union(b.rect)));
            }
        }
        rect
    }

    /// The drop gap nearest the dragged item's translated top, among its
    /// sibling group.
    ///
    /// Only gaps that actually move the item are offered. Dropping into
    /// the slot the item already occupies is a no-op, so it's filtered
    /// rather than left as a dead target. Staying within the item's own
    /// vertical span is a cancel (returns `None`), so a small drag snaps
    /// back.
    pub fn drop_gap_for(&self, drag: &BlockDrag, pointer: Pos2) -> Option<DropGap> {
        let mut units: Vec<BlockBox> = self
            .block_boxes
            .iter()
            .filter(|b| b.parent_start == drag.parent_start)
            .copied()
            .collect();
        if units.is_empty() {
            return None;
        }
        units.sort_by_key(|b| b.node_range.start());

        // The dragged span may cover several units (multi-item selection);
        // they move as one. Find the contiguous run [lo, hi] inside it.
        let in_span = |u: &BlockBox| {
            u.node_range.start() >= drag.section_range.start()
                && u.node_range.end() <= drag.section_range.end()
        };
        let lo = units.iter().position(in_span)?;
        let hi = units.iter().rposition(in_span)?;

        // Order by handle position — the thing you're holding. Each
        // unit's marker sits at its top (first row); the dragged run's
        // marker floats at `pointer.y - grab_offset.y`, which is the
        // screen-Y of the floating card's top (scroll-invariant). The
        // run sorts before any other unit whose marker is below the
        // floating one.
        let dragged_handle_y = pointer.y - drag.grab_offset.y;

        let others: Vec<&BlockBox> = units
            .iter()
            .enumerate()
            .filter(|(i, _)| *i < lo || *i > hi)
            .map(|(_, u)| u)
            .collect();
        let to = others
            .iter()
            .filter(|u| u.rect.top() < dragged_handle_y)
            .count();

        // `to == lo` is the run's own slot (removing it merges its
        // neighboring gaps) — no move, snap back.
        if to == lo {
            return None;
        }

        // Sit the indicator in the visual middle of the inter-item gap:
        // between two neighbors that's the midpoint of prev.bottom and
        // next.top, so for loose lists the line lands in the spacing
        // rather than on the next item's first row. For the leading /
        // trailing gap (no neighbor on one side) fall back to a half-
        // `block_spacing` offset past the lone neighbor.
        let pad = self.layout.block_spacing / 2.0;
        let prev = to.checked_sub(1).map(|i| others[i]);
        let next = others.get(to).copied();
        let y = match (prev, next) {
            (Some(p), Some(n)) => (p.rect.bottom() + n.rect.top()) / 2.0,
            (None, Some(n)) => n.rect.top() - pad,
            (Some(p), None) => p.rect.bottom() + pad,
            (None, None) => unreachable!("drop_gap_for early-returns when units.is_empty()"),
        };
        let insert_offset = match next {
            Some(n) => n.node_range.start(),
            None => self.buffer.current.segs.last_cursor_position(),
        };
        let gap_index = units
            .iter()
            .filter(|u| u.node_range.start() < insert_offset)
            .count();
        Some(DropGap { insert_offset, gap_index, y: y as i32 })
    }
}

/// A planned item move as a single text edit. `moved_range` is the new
/// selection in post-move document offsets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockMovePlan {
    /// The contiguous run of sibling items to replace.
    pub run_range: (Grapheme, Grapheme),
    /// The run rebuilt with the dragged item moved to its new slot.
    pub new_run: String,
    /// The moved item's range after the edit (for selection).
    pub moved_range: (Grapheme, Grapheme),
}

impl<'ast> MdRender {
    /// Plan a sibling reorder as a positional permutation: move the
    /// item at `section_range` to the gap at `insert_offset`, keeping
    /// every separator in its slot. `None` for a no-op or if the item
    /// can't be located.
    ///
    /// Separators are positional pseudoblocks: the run `B0 G0 B1 G1 …
    /// Bn` is rebuilt with the items permuted and gaps `Gi` untouched.
    /// Same-depth siblings share a line prefix, so item text moves
    /// verbatim — no re-prefixing for v1 (reorder only, no reparent).
    pub fn plan_block_move(
        &self, root: &'ast AstNode<'ast>, section_range: (Grapheme, Grapheme),
        insert_offset: Grapheme,
    ) -> Option<BlockMovePlan> {
        // Locate the first item in the span. `section_range` may cover
        // several sibling items (multi-item selection).
        let dragged = root.descendants().find(|n| {
            matches!(n.data.borrow().value, NodeValue::Item(_) | NodeValue::TaskItem(_)) && {
                let r = self.node_range(n);
                r.start() == section_range.start() && r.end() <= section_range.end()
            }
        })?;
        let parent = dragged.parent()?;

        let mut units: Vec<(Grapheme, Grapheme)> = parent
            .children()
            .filter(|c| {
                matches!(c.data.borrow().value, NodeValue::Item(_) | NodeValue::TaskItem(_))
            })
            .map(|c| self.node_range(c))
            .collect();
        units.sort_by_key(|s| s.start());
        if units.len() < 2 {
            return None;
        }

        // The selected run [lo, hi] (one item for a single drag) moves
        // together. The destination index `to` is in unit-space.
        let lo = units
            .iter()
            .position(|u| u.start() >= section_range.start())?;
        let hi = units.iter().rposition(|u| u.end() <= section_range.end())?;
        if hi < lo {
            return None;
        }
        let to = units.iter().filter(|u| u.start() < insert_offset).count();

        // Whole-line item text and the gaps between consecutive items.
        let lines = &self.bounds.source_lines;
        let block_range = |u: (Grapheme, Grapheme)| {
            let fi = self.line_idx_for_offset(u.start());
            let li = self.last_line_idx_for_end(u.end());
            (lines[fi].start(), lines[li].end())
        };
        let branges: Vec<(Grapheme, Grapheme)> = units.iter().map(|u| block_range(*u)).collect();
        let blocks: Vec<String> = branges
            .iter()
            .map(|b| self.buffer[*b].to_string())
            .collect();
        let gaps: Vec<String> = (0..branges.len() - 1)
            .map(|i| self.buffer[(branges[i].1, branges[i + 1].0)].to_string())
            .collect();
        let run_range = (branges[0].0, branges[branges.len() - 1].1);

        // Collapse the selected run into a single super-item (carrying
        // its internal gaps), so the positional permutation moves it as
        // one. Gaps outside the run stay in their slots.
        let mut merged = String::new();
        for i in lo..=hi {
            merged.push_str(&blocks[i]);
            if i < hi {
                merged.push_str(&gaps[i]);
            }
        }
        let mut rblocks: Vec<String> = Vec::new();
        rblocks.extend(blocks[..lo].iter().cloned());
        rblocks.push(merged);
        rblocks.extend(blocks[hi + 1..].iter().cloned());
        let mut rgaps: Vec<String> = Vec::new();
        rgaps.extend(gaps[..lo].iter().cloned());
        rgaps.extend(gaps[hi..].iter().cloned());

        // Map the full-unit destination into the reduced list (run
        // collapsed to one slot at `lo`), then to the post-removal
        // insert index.
        let to_reduced = if to <= lo { to } else { to - (hi - lo) };
        let to_adj = if to_reduced > lo { to_reduced - 1 } else { to_reduced };
        if to_adj == lo {
            return None;
        }

        let moved = rblocks.remove(lo);
        rblocks.insert(to_adj, moved);

        use unicode_segmentation::UnicodeSegmentation as _;
        let g = |s: &str| -> Graphemes { s.graphemes(true).count().into() };
        let mut moved_start = run_range.0;
        for i in 0..to_adj {
            moved_start += g(&rblocks[i]);
            if i < rgaps.len() {
                moved_start += g(&rgaps[i]);
            }
        }
        let moved_range = (moved_start, moved_start + g(&rblocks[to_adj]));

        let mut new_run = String::new();
        for i in 0..rblocks.len() {
            new_run.push_str(&rblocks[i]);
            if i < rgaps.len() {
                new_run.push_str(&rgaps[i]);
            }
        }

        Some(BlockMovePlan { run_range, new_run, moved_range })
    }

    fn line_idx_for_offset(&self, offset: Grapheme) -> usize {
        let lines = &self.bounds.source_lines;
        lines
            .iter()
            .position(|l| l.contains(offset, true, true))
            .unwrap_or(lines.len().saturating_sub(1))
    }

    /// Index of the source line whose end is at (or contains) an
    /// exclusive range end. A section end on a line boundary belongs to
    /// the preceding line.
    fn last_line_idx_for_end(&self, end: Grapheme) -> usize {
        let lines = &self.bounds.source_lines;
        for i in (0..lines.len()).rev() {
            let l = lines[i];
            if l.end() == end || l.contains(end, true, false) {
                return i;
            }
        }
        self.line_idx_for_offset(end)
    }
}
