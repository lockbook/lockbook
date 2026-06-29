//! Drag-to-reorder for list items. A per-frame [`BlockBox`] index is
//! filled during the render DFS and read for hit-testing.
//!
//! Reorder is sibling-only (same parent `List`); release rewrites the
//! sibling run as a single text op via [`MdRender::plan_block_move`].

use comrak::nodes::{AstNode, NodeValue};
use egui::{CursorIcon, Pos2, Rect, Ui, Vec2};
use lb_rs::model::text::offset_types::{Grapheme, Graphemes, RangeExt as _};

use crate::tab::markdown_editor::{MdEdit, MdRender};

#[derive(Clone, Copy, Debug)]
pub enum BlockDragAction {
    Started(BlockDrag),
    Dragged(Pos2),
    Released(Pos2),
}

/// Touch long-press → drag-reorder gesture state. Desktop reorders
/// immediately from the marker handle; touch requires a stationary hold
/// so a pan still scrolls. Driven by `MdEdit::detect_touch_reorder`.
#[derive(Clone, Copy, Debug, Default)]
pub enum TouchReorder {
    #[default]
    Idle,
    /// Finger down on a reorderable item, disambiguating long-press
    /// (arm) from pan (cancel).
    Pending { origin: Pos2, started: f64 },
    /// Long-press fired; the reorder runs via `in_progress_block_drag`.
    /// `last` is the most recent live pointer — the drop position on
    /// release, since touch-up nulls `latest_pos` (`PointerGone`).
    Armed { last: Pos2 },
}

#[derive(Clone, Copy, Debug)]
pub struct BlockBox {
    /// Item's source range (includes nested children).
    pub node_range: (Grapheme, Grapheme),
    pub rect: Rect,
    /// Start of the parent `List`; two items are reorder siblings iff
    /// their `parent_start` matches.
    pub parent_start: Grapheme,
}

#[derive(Clone, Copy, Debug)]
pub struct BlockDrag {
    /// Full dragged span — one item, or several selected siblings.
    pub section_range: (Grapheme, Grapheme),
    pub grabbed: (Grapheme, Grapheme),
    pub parent_start: Grapheme,
    /// Pointer minus the section's top-left at grab time. Scroll-
    /// invariant, so `pointer - grab_offset` tracks the floating card.
    pub grab_offset: Vec2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DropGap {
    pub insert_offset: Grapheme,
    /// Gap index among siblings (0 = before first item).
    pub gap_index: usize,
    pub y: i32,
}

impl<'ast> MdRender {
    /// True iff `node` is an `Item`/`TaskItem` with ≥1 sibling item.
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

    /// Union of the selected sibling items including `node`, or just
    /// `node`'s range when no multi-item selection is active.
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

    pub fn push_block_box(
        &mut self, node: &'ast AstNode<'ast>, rect: Rect, parent_start: Grapheme,
    ) {
        let node_range = self.node_range(node);
        self.block_boxes
            .push(BlockBox { node_range, rect, parent_start });
    }

    /// Shared by `show_item` (marker rect) and `show_task_item`
    /// (checkbox response): translate the response into a drag action
    /// and set the cursor icon.
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
            // Anchor on the section's top-left so multi-item drags keep
            // the grabbed item under the cursor. DFS top-to-bottom fill
            // means every in-span box is indexed by now.
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

    /// Single-item drag for the reorderable list item under `pos` — the
    /// touch long-press path. `None` if `pos` isn't over an item that has
    /// a reorder sibling. The innermost (smallest) box wins, so a press on
    /// a nested item reorders it among its own siblings. Multi-select drag
    /// stays desktop-only.
    pub fn touch_reorder_target(&self, pos: Pos2) -> Option<BlockDrag> {
        let area = |b: &BlockBox| b.rect.width() * b.rect.height();
        let b = self
            .block_boxes
            .iter()
            .filter(|b| b.rect.contains(pos))
            .min_by(|a, c| area(a).total_cmp(&area(c)))?;
        let has_sibling = self
            .block_boxes
            .iter()
            .filter(|x| x.parent_start == b.parent_start)
            .nth(1)
            .is_some();
        has_sibling.then(|| BlockDrag {
            section_range: b.node_range,
            grabbed: b.node_range,
            parent_start: b.parent_start,
            grab_offset: pos - b.rect.left_top(),
        })
    }

    /// Union of indexed item rects within `span`.
    pub fn section_rect(&self, span: (Grapheme, Grapheme)) -> Option<Rect> {
        let mut rect: Option<Rect> = None;
        for b in &self.block_boxes {
            if b.node_range.start() >= span.start() && b.node_range.end() <= span.end() {
                rect = Some(rect.map_or(b.rect, |r: Rect| r.union(b.rect)));
            }
        }
        rect
    }

    /// Drop gap nearest the dragged item's translated top within its
    /// sibling group. `None` if hovering the item's own slot (cancel).
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

        // [lo, hi] is the contiguous selected run inside `units`.
        let in_span = |u: &BlockBox| {
            u.node_range.start() >= drag.section_range.start()
                && u.node_range.end() <= drag.section_range.end()
        };
        let lo = units.iter().position(in_span)?;
        let hi = units.iter().rposition(in_span)?;

        // Sort by handle position (the marker top) — for the dragged
        // run that's the floating card's top.
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

        // `to == lo` is the run's own slot — snap back.
        if to == lo {
            return None;
        }

        // Indicator y sits in the visual middle of the inter-item gap.
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockMovePlan {
    pub run_range: (Grapheme, Grapheme),
    pub new_run: String,
    /// Moved item's range in post-move offsets (for selection).
    pub moved_range: (Grapheme, Grapheme),
}

impl<'ast> MdRender {
    /// Plan a sibling reorder as a positional permutation: items are
    /// permuted, separator gaps `Gi` stay in their slots. Same-depth
    /// siblings share a line prefix, so item text moves verbatim.
    /// `None` for a no-op or unlocatable item.
    pub fn plan_block_move(
        &self, root: &'ast AstNode<'ast>, section_range: (Grapheme, Grapheme),
        insert_offset: Grapheme,
    ) -> Option<BlockMovePlan> {
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

        let lo = units
            .iter()
            .position(|u| u.start() >= section_range.start())?;
        let hi = units.iter().rposition(|u| u.end() <= section_range.end())?;
        if hi < lo {
            return None;
        }
        let to = units.iter().filter(|u| u.start() < insert_offset).count();

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
        // its internal gaps) so it permutes as one unit.
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

        // Destination in reduced-list space, then post-removal index.
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

    /// Source-line index for an exclusive range end. An end on a line
    /// boundary belongs to the preceding line.
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

impl MdEdit {
    /// Touch long-press → drag-reorder. The body owns the press so a pan
    /// still scrolls; a stationary hold past `LONG_PRESS` arms the reorder.
    /// No-op unless touch, editable, and the keyboard is hidden (keyboard up
    /// → long-press is text selection). Emits into `block_drag_action`,
    /// consumed by `handle_block_drag`.
    pub(crate) fn detect_touch_reorder(&mut self, ui: &mut Ui, keyboard_visible: bool) {
        // Structural conditions don't change mid-gesture.
        if !self.renderer.touch_mode || self.renderer.readonly || !self.renderer.interactive {
            self.touch_reorder = TouchReorder::Idle;
            return;
        }

        const LONG_PRESS: f64 = 0.4;
        const SLOP: f32 = 12.0;

        let (any_down, any_pressed, origin, latest, time, t0) = ui.input(|i| {
            (
                i.pointer.any_down(),
                i.pointer.any_pressed(),
                i.pointer.press_origin(),
                i.pointer.latest_pos(),
                i.time,
                i.pointer.press_start_time(),
            )
        });
        // Reliable "finger up": on iOS a touch-up over a click-sense widget
        // (e.g. the task checkbox) can arrive as a bare `PointerGone`, leaving
        // egui's button state stuck `down` — but it always nulls the live
        // pointer, so `latest.is_none()` is the dependable lift signal.
        let lifted = !any_down || latest.is_none();

        match self.touch_reorder {
            TouchReorder::Idle => {
                // Keyboard up → long-press is text selection, not reorder.
                if let (true, false, Some(origin), Some(started)) =
                    (any_pressed, keyboard_visible, origin, t0)
                {
                    if self.renderer.touch_reorder_target(origin).is_some() {
                        self.touch_reorder = TouchReorder::Pending { origin, started };
                    }
                }
            }
            TouchReorder::Pending { origin, started } => {
                let moved = latest.map(|p| (p - origin).length()).unwrap_or(0.0);
                if keyboard_visible || lifted {
                    self.touch_reorder = TouchReorder::Idle; // tapped, or keyboard rose
                } else if moved > SLOP {
                    self.touch_reorder = TouchReorder::Idle; // pan → leave it to scroll
                } else if time - started >= LONG_PRESS {
                    match self.renderer.touch_reorder_target(origin) {
                        Some(drag) => {
                            self.renderer.block_drag_action = Some(BlockDragAction::Started(drag));
                            self.touch_reorder =
                                TouchReorder::Armed { last: latest.unwrap_or(origin) };
                        }
                        None => self.touch_reorder = TouchReorder::Idle,
                    }
                } else {
                    ui.ctx().request_repaint(); // tick toward the threshold
                }
            }
            TouchReorder::Armed { last } => {
                // Drop where the pointer last lived (touch-up nulls `latest`).
                let pointer = latest.unwrap_or(last);
                if lifted {
                    self.renderer.block_drag_action = Some(BlockDragAction::Released(pointer));
                    self.touch_reorder = TouchReorder::Idle;
                } else {
                    self.touch_reorder = TouchReorder::Armed { last: pointer };
                }
            }
        }
    }
}
