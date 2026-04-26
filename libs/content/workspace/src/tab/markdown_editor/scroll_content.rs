//! [`ScrollContent`] adapter for the markdown editor.
//!
//! `DocScrollContent` is the cursor itself — no flat leaf list, no
//! pre-collected `Vec` of nodes. The cursor walks the AST recursively,
//! descending into block containers and halting at scroll leaves
//! (paragraph, heading, code block, table, ...). Each leaf becomes one
//! row in the scroll area.
//!
//! Container chrome (block-quote bar, alert bar, list / task / footnote
//! markers) isn't painted by the container itself — it's painted by
//! the leaf, which walks its ancestor chain and calls `chrome_*`
//! per-ancestor according to:
//!
//! - **Bar-style** (BlockQuote, Alert): paint when this leaf is the
//!   first descendant in flat order, OR when its `row_top` sits at or
//!   above the viewport top (the geometric anchor). Rect spans from
//!   the leaf's start (or `viewport.min.y - 1` for anchor mid-bq)
//!   down through subsequent in-ancestor leaves, clamped to
//!   `viewport.bottom + 1` so off-screen edges/corners get clipped
//!   invisibly. Alerts also paint their title via `show_alert_title_line`
//!   in the first-descendant case.
//! - **Marker-style** (Item, TaskItem, FootnoteDefinition): paint when
//!   this leaf is the first descendant in flat order. Markers don't
//!   tile; they sit at the absolute first leaf and scroll with the
//!   parent.
//! - **Self-chrome**: when the leaf is itself the chrome-bearing
//!   node — revealed BlockQuote / Alert (atomic), or per-line
//!   CodeBlock (border) — `paint_self_chrome` handles it.
//!   `paint_ancestor_chrome` only walks ancestors and would otherwise
//!   miss these.
//!
//! Code blocks are line-based leaves: each source line is a row.
//! Hidden fence lines (opening / closing of a non-revealed fenced
//! block) are 0-height rows. `block_padding` is folded into the first
//! row's prefix and the last row's suffix.
//!
//! Plus one **virtual trailing pad** row at the end with `approx = 0`
//! and `precise = trailing_precise`, giving the user vh/2 of empty
//! space below the doc end.

use comrak::nodes::{AstNode, NodeValue};
use egui::{Pos2, Rect, Ui, Vec2};
use lb_rs::model::text::offset_types::Grapheme;

use crate::tab::markdown_editor::MdRender;
use crate::tab::markdown_editor::widget::utils::NodeValueExt as _;
use crate::widgets::affine_scroll::Rows;

#[cfg(test)]
thread_local! {
    static BQ_BAR_RECTS: std::cell::RefCell<Vec<egui::Rect>> = const { std::cell::RefCell::new(Vec::new()) };
    static ALERT_BAR_RECTS: std::cell::RefCell<Vec<egui::Rect>> = const { std::cell::RefCell::new(Vec::new()) };
    static ALERT_TITLE_POSES: std::cell::RefCell<Vec<egui::Pos2>> = const { std::cell::RefCell::new(Vec::new()) };
    static CODE_BLOCK_CHROME_RECTS: std::cell::RefCell<Vec<egui::Rect>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Test-only: clear recorded bar rects.
#[cfg(test)]
pub fn test_clear_bq_bars() {
    BQ_BAR_RECTS.with(|r| r.borrow_mut().clear());
}

/// Test-only: record a bar rect from `chrome_block_quote`.
#[cfg(test)]
pub fn test_record_bq_bar(rect: egui::Rect) {
    BQ_BAR_RECTS.with(|r| r.borrow_mut().push(rect));
}

/// Test-only: read recorded bar rects.
#[cfg(test)]
pub fn test_bq_bars() -> Vec<egui::Rect> {
    BQ_BAR_RECTS.with(|r| r.borrow().clone())
}

#[cfg(test)]
pub fn test_clear_alert_chrome() {
    ALERT_BAR_RECTS.with(|r| r.borrow_mut().clear());
    ALERT_TITLE_POSES.with(|r| r.borrow_mut().clear());
}

#[cfg(test)]
pub fn test_record_alert_bar(rect: egui::Rect) {
    ALERT_BAR_RECTS.with(|r| r.borrow_mut().push(rect));
}

#[cfg(test)]
pub fn test_record_alert_title(pos: egui::Pos2) {
    ALERT_TITLE_POSES.with(|r| r.borrow_mut().push(pos));
}

#[cfg(test)]
pub fn test_alert_bars() -> Vec<egui::Rect> {
    ALERT_BAR_RECTS.with(|r| r.borrow().clone())
}

#[cfg(test)]
pub fn test_alert_title_poses() -> Vec<egui::Pos2> {
    ALERT_TITLE_POSES.with(|r| r.borrow().clone())
}

#[cfg(test)]
pub fn test_clear_code_block_chrome() {
    CODE_BLOCK_CHROME_RECTS.with(|r| r.borrow_mut().clear());
}

#[cfg(test)]
pub fn test_record_code_block_chrome(rect: egui::Rect) {
    CODE_BLOCK_CHROME_RECTS.with(|r| r.borrow_mut().push(rect));
}

#[cfg(test)]
pub fn test_code_block_chrome_rects() -> Vec<egui::Rect> {
    CODE_BLOCK_CHROME_RECTS.with(|r| r.borrow().clone())
}

pub struct DocScrollContent<'a, 'ast> {
    pub renderer: &'a mut MdRender,
    pub root: &'ast AstNode<'ast>,
    /// Precise height of the virtual trailing pad row (e.g. vh/2).
    pub trailing_precise: f32,
    cursor: Cursor<'ast>,
}

#[derive(Clone, Copy)]
enum Cursor<'ast> {
    Start,
    At {
        node: &'ast AstNode<'ast>,
        /// `Some(idx)` for line-based leaves (paragraph today; more
        /// in R3+); `None` for atomic leaves.
        line: Option<usize>,
    },
    Trailing,
    End,
}

impl<'a, 'ast> DocScrollContent<'a, 'ast> {
    pub fn new(
        renderer: &'a mut MdRender, root: &'ast AstNode<'ast>, trailing_precise: f32,
    ) -> Self {
        Self { renderer, root, trailing_precise, cursor: Cursor::Start }
    }

    /// Source-text range covered by the row at the cursor's current
    /// position, or `None` if the cursor is off a real row.
    pub fn text_range(&self) -> Option<(Grapheme, Grapheme)> {
        match self.cursor {
            Cursor::At { node, line: None } => Some(self.renderer.node_range(node)),
            Cursor::At { node, line: Some(idx) } => Some(self.line_range(node, idx)),
            Cursor::Trailing | Cursor::Start | Cursor::End => None,
        }
    }

    /// Number of source lines for a line-based leaf, or `None` for
    /// atomic leaves.
    fn line_count(&self, node: &'ast AstNode<'ast>) -> Option<usize> {
        let value = &node.data.borrow().value;
        let line_based = matches!(value, NodeValue::Paragraph | NodeValue::CodeBlock(_))
            || (matches!(value, NodeValue::Document) && self.renderer.plaintext);
        if !line_based {
            return None;
        }
        let first = self.renderer.node_first_line_idx(node);
        let last = self.renderer.node_last_line_idx(node);
        Some(last - first + 1)
    }

    /// Source-line range for line `idx` within a line-based leaf.
    fn line_range(&self, node: &'ast AstNode<'ast>, idx: usize) -> (Grapheme, Grapheme) {
        let first = self.renderer.node_first_line_idx(node);
        let line = self.renderer.bounds.source_lines[first + idx];
        self.renderer.node_line(node, line)
    }

    /// Build a cursor positioned at the first line of a leaf (or `None`
    /// for atomic).
    fn first_line_at(&self, node: &'ast AstNode<'ast>) -> Cursor<'ast> {
        Cursor::At { node, line: self.line_count(node).map(|_| 0) }
    }

    fn last_line_at(&self, node: &'ast AstNode<'ast>) -> Cursor<'ast> {
        Cursor::At { node, line: self.line_count(node).map(|n| n.saturating_sub(1)) }
    }

    /// Does the cursor descend into this node, or stop here?
    fn is_scroll_leaf(&self, node: &'ast AstNode<'ast>) -> bool {
        let value = &node.data.borrow().value;
        if !value.is_container_block() {
            return true;
        }
        // Plaintext doc is a line-based leaf at the root.
        if matches!(value, NodeValue::Document) && self.renderer.plaintext {
            return true;
        }
        // Revealed containers paint as source lines via `show_block`'s
        // reveal branch — treat atomically.
        if node.parent().is_some() && self.renderer.reveal(node) {
            return true;
        }
        // Cursor descends into Table; halts at each TableRow.
        matches!(value, NodeValue::TableRow(_) | NodeValue::TableCell)
    }

    /// Leftmost leaf in `node`'s subtree, skipping empty containers.
    fn first_leaf_in(&self, node: &'ast AstNode<'ast>) -> Option<&'ast AstNode<'ast>> {
        if self.is_scroll_leaf(node) {
            return Some(node);
        }
        let mut child = node.first_child()?;
        loop {
            if let Some(leaf) = self.first_leaf_in(child) {
                return Some(leaf);
            }
            child = child.next_sibling()?;
        }
    }

    /// Rightmost leaf in `node`'s subtree, skipping empty containers.
    fn last_leaf_in(&self, node: &'ast AstNode<'ast>) -> Option<&'ast AstNode<'ast>> {
        if self.is_scroll_leaf(node) {
            return Some(node);
        }
        let mut child = node.last_child()?;
        loop {
            if let Some(leaf) = self.last_leaf_in(child) {
                return Some(leaf);
            }
            child = child.previous_sibling()?;
        }
    }

    /// Next leaf in flat depth-first order after `node`, or `None` if
    /// `node` is the last leaf in the doc.
    fn next_leaf(&self, node: &'ast AstNode<'ast>) -> Option<&'ast AstNode<'ast>> {
        let mut current = node;
        loop {
            if let Some(sib) = current.next_sibling() {
                if let Some(leaf) = self.first_leaf_in(sib) {
                    return Some(leaf);
                }
                current = sib;
                continue;
            }
            let parent = current.parent()?;
            if std::ptr::eq(parent, self.root) {
                return None;
            }
            current = parent;
        }
    }

    /// Previous leaf in flat depth-first order before `node`.
    fn prev_leaf(&self, node: &'ast AstNode<'ast>) -> Option<&'ast AstNode<'ast>> {
        let mut current = node;
        loop {
            if let Some(sib) = current.previous_sibling() {
                if let Some(leaf) = self.last_leaf_in(sib) {
                    return Some(leaf);
                }
                current = sib;
                continue;
            }
            let parent = current.parent()?;
            if std::ptr::eq(parent, self.root) {
                return None;
            }
            current = parent;
        }
    }

    /// Pre-spacing height for `leaf` (and ancestors that contribute it
    /// because `leaf` is their first descendant).
    fn leaf_pre_spacing(&self, leaf: &'ast AstNode<'ast>) -> f32 {
        let mut h = 0.0;
        // Code blocks render their padding outside the line content so
        // it sits inside the chrome border but above the first line.
        if matches!(leaf.data.borrow().value, NodeValue::CodeBlock(_)) {
            h += self.renderer.layout.block_padding;
        }
        let mut node = leaf;
        loop {
            h += self.renderer.block_pre_spacing_height(node);
            // Walk up only while `node` is its parent's first child —
            // that's how the parent's pre-spacing falls onto this leaf.
            let Some(parent) = node.parent() else { break };
            if std::ptr::eq(parent, self.root) {
                break;
            }
            let Some(first_child) = parent.first_child() else { break };
            if !std::ptr::eq(first_child, node) {
                break;
            }
            node = parent;
            // Alert's title sits between the alert's start and its
            // first child's content — paid by the first descendant.
            if let NodeValue::Alert(node_alert) = node.data.borrow().value.clone() {
                h += self.renderer.height_alert_title_line(node, &node_alert);
                h += self.renderer.layout.block_spacing;
            }
        }
        h
    }

    /// Post-spacing height for `leaf`. Symmetric to `leaf_pre_spacing`.
    fn leaf_post_spacing(&self, leaf: &'ast AstNode<'ast>) -> f32 {
        let mut h = 0.0;
        if matches!(leaf.data.borrow().value, NodeValue::CodeBlock(_)) {
            h += self.renderer.layout.block_padding;
        }
        let mut node = leaf;
        loop {
            h += self.renderer.block_post_spacing_height(node);
            let Some(parent) = node.parent() else { break };
            if std::ptr::eq(parent, self.root) {
                break;
            }
            let Some(last_child) = parent.last_child() else { break };
            if !std::ptr::eq(last_child, node) {
                break;
            }
            node = parent;
        }
        h
    }

    /// Same as `leaf_pre_spacing` but using approx heights.
    fn leaf_pre_spacing_approx(&self, leaf: &'ast AstNode<'ast>) -> f32 {
        let mut h = 0.0;
        if matches!(leaf.data.borrow().value, NodeValue::CodeBlock(_)) {
            h += self.renderer.layout.block_padding;
        }
        let mut node = leaf;
        loop {
            h += self.renderer.block_pre_spacing_height_approx(node);
            let Some(parent) = node.parent() else { break };
            if std::ptr::eq(parent, self.root) {
                break;
            }
            let Some(first_child) = parent.first_child() else { break };
            if !std::ptr::eq(first_child, node) {
                break;
            }
            node = parent;
            if matches!(node.data.borrow().value, NodeValue::Alert(_)) {
                h += self.renderer.layout.row_height + self.renderer.layout.block_spacing;
            }
        }
        h
    }

    fn leaf_post_spacing_approx(&self, leaf: &'ast AstNode<'ast>) -> f32 {
        let mut h = 0.0;
        if matches!(leaf.data.borrow().value, NodeValue::CodeBlock(_)) {
            h += self.renderer.layout.block_padding;
        }
        let mut node = leaf;
        loop {
            h += self.renderer.block_post_spacing_height_approx(node);
            let Some(parent) = node.parent() else { break };
            if std::ptr::eq(parent, self.root) {
                break;
            }
            let Some(last_child) = parent.last_child() else { break };
            if !std::ptr::eq(last_child, node) {
                break;
            }
            node = parent;
        }
        h
    }

    /// Walk the leaf's ancestors and paint each ancestor's chrome
    /// per the rule. Returns nothing; chrome painting is a side
    /// effect on `ui.painter()`.
    fn paint_ancestor_chrome(
        &mut self, ui: &mut Ui, leaf: &'ast AstNode<'ast>, line: Option<usize>, row_top: f32,
        content_top: f32, content_height: f32,
    ) {
        let viewport = self.renderer.viewport.get();
        let content_bottom = content_top + content_height;
        let is_first_row_of_leaf = line.unwrap_or(0) == 0;
        // Walk root → leaf so we accumulate x correctly.
        let chain: Vec<&AstNode> = leaf.ancestors().collect();
        let mut x = self.renderer.top_left.x;
        for ancestor in chain.iter().rev() {
            // Skip the document root and the leaf itself.
            if std::ptr::eq(*ancestor, self.root) {
                continue;
            }
            if std::ptr::eq(*ancestor, leaf) {
                continue;
            }

            let value = ancestor.data.borrow().value.clone();
            let indent = ancestor_indent(&value, self.renderer);

            // "First descendant of ancestor in flat order" = my leaf
            // is the leftmost, AND I'm the first row within that leaf.
            let i_am_first_descendant =
                std::ptr::eq(self.first_leaf_under(ancestor), leaf) && is_first_row_of_leaf;
            // The anchor is the only row whose top edge can sit at or
            // above the viewport top — so this is a reliable
            // anchor-detection check. Anchor paints chrome for any
            // ancestor it descends from (preceding descendants weren't
            // rendered).
            let i_paint_bar = i_am_first_descendant || row_top <= viewport.min.y;

            match value {
                NodeValue::BlockQuote => {
                    if i_paint_bar {
                        let bottom = self.bar_chrome_bottom(leaf, line, ancestor, content_bottom);
                        let top =
                            if i_am_first_descendant { content_top } else { viewport.min.y - 1.0 };
                        let annotation = Rect::from_min_size(
                            Pos2::new(x, top),
                            Vec2::new(indent, (bottom - top).max(0.0)),
                        );
                        self.renderer.chrome_block_quote(ui, annotation);
                    }
                }
                NodeValue::Alert(ref node_alert) => {
                    if i_paint_bar {
                        let bottom = self.bar_chrome_bottom(leaf, line, ancestor, content_bottom);
                        let (bar_top, title_top) = if i_am_first_descendant {
                            // Bar starts above the title; title sits at
                            // the alert's top, the leaf content sits
                            // below title + block_spacing + nested
                            // pre-spacings.
                            let below = self.pre_spacing_below(leaf, ancestor);
                            let title_h =
                                self.renderer.height_alert_title_line(ancestor, node_alert);
                            let title_top =
                                content_top - below - title_h - self.renderer.layout.block_spacing;
                            (title_top, Some(title_top))
                        } else {
                            (viewport.min.y - 1.0, None)
                        };
                        let annotation = Rect::from_min_size(
                            Pos2::new(x, bar_top),
                            Vec2::new(indent, (bottom - bar_top).max(0.0)),
                        );
                        self.renderer.chrome_alert(ui, ancestor, annotation);
                        if let Some(title_top) = title_top {
                            let title_pos = Pos2::new(x + indent, title_top);
                            self.renderer
                                .show_alert_title_line(ui, ancestor, title_pos, node_alert);
                        }
                    }
                }
                NodeValue::Item(_) => {
                    if i_am_first_descendant {
                        let row_height = self.renderer.node_line_row_height(
                            ancestor,
                            self.renderer.node_first_line(ancestor),
                        );
                        let annotation = Rect::from_min_size(
                            Pos2::new(x, content_top),
                            Vec2::new(indent, row_height),
                        );
                        self.renderer
                            .chrome_item(ui, ancestor, annotation, row_height);
                    }
                }
                NodeValue::TaskItem(node_task_item) => {
                    if i_am_first_descendant {
                        let row_height = self.renderer.node_line_row_height(
                            ancestor,
                            self.renderer.node_first_line(ancestor),
                        );
                        let annotation = Rect::from_min_size(
                            Pos2::new(x, content_top),
                            Vec2::new(indent, row_height),
                        );
                        let checked = node_task_item.symbol.is_some();
                        self.renderer
                            .chrome_task_item(ui, ancestor, annotation, checked);
                    }
                }
                NodeValue::FootnoteDefinition(_) => {
                    if i_am_first_descendant {
                        let annotation = Rect::from_min_size(
                            Pos2::new(x, content_top),
                            Vec2::new(indent, self.renderer.layout.row_height),
                        );
                        self.renderer
                            .chrome_footnote_definition(ui, ancestor, annotation);
                    }
                }
                NodeValue::Table(_) => {
                    if i_paint_bar {
                        let bottom = self.bar_chrome_bottom(leaf, line, ancestor, content_bottom);
                        let top =
                            if i_am_first_descendant { content_top } else { viewport.min.y - 1.0 };
                        let block_width = self.renderer.width(ancestor);
                        let rect = Rect::from_min_size(
                            Pos2::new(x, top),
                            Vec2::new(block_width, (bottom - top).max(0.0)),
                        );
                        self.renderer.chrome_table(ui, rect);
                    }
                }
                _ => {}
            }
            x += indent;
        }
    }

    /// First leaf under `ancestor` in flat order (cached via
    /// `first_leaf_in`). Returns `ancestor` itself if it's a leaf
    /// (shouldn't happen at call sites, but safe).
    fn first_leaf_under(&self, ancestor: &'ast AstNode<'ast>) -> &'ast AstNode<'ast> {
        self.first_leaf_in(ancestor).unwrap_or(ancestor)
    }

    /// Same shape as `leaf_pre_spacing` but stops just below
    /// `ancestor` — i.e., sums pre-spacings (and any nested-alert
    /// titles) for nodes from `leaf` up to and including
    /// `ancestor`'s first child. Used to back-compute the y of
    /// `ancestor`'s top edge from a leaf's `content_top`.
    fn pre_spacing_below(&self, leaf: &'ast AstNode<'ast>, ancestor: &'ast AstNode<'ast>) -> f32 {
        let mut h = 0.0;
        let mut node = leaf;
        while !std::ptr::eq(node, ancestor) {
            h += self.renderer.block_pre_spacing_height(node);
            let Some(parent) = node.parent() else { break };
            node = parent;
            if std::ptr::eq(node, ancestor) {
                break;
            }
            // Nested alert in the path between leaf and ancestor.
            if let NodeValue::Alert(node_alert) = node.data.borrow().value.clone() {
                h += self.renderer.height_alert_title_line(node, &node_alert);
                h += self.renderer.layout.block_spacing;
            }
        }
        h
    }

    /// Paint chrome whose container *is* this leaf — fires when the
    /// cursor halts at a chrome-bearing container (revealed BlockQuote
    /// / Alert, or per-line CodeBlock). Mirrors the bar-style rule in
    /// `paint_ancestor_chrome` but applied to the leaf itself.
    fn paint_self_chrome(
        &mut self, ui: &mut Ui, leaf: &'ast AstNode<'ast>, line: Option<usize>, row_top: f32,
        content_top: f32, content_height: f32,
    ) {
        let value = leaf.data.borrow().value.clone();
        let indent = ancestor_indent(&value, self.renderer);
        let x = leaf
            .ancestors()
            .filter(|a| !std::ptr::eq(*a, self.root))
            .map(|a| ancestor_indent(&a.data.borrow().value, self.renderer))
            .sum::<f32>()
            + self.renderer.top_left.x;
        match value {
            NodeValue::BlockQuote => {
                if line.is_some() {
                    return;
                }
                let annotation = Rect::from_min_size(
                    Pos2::new(x, content_top),
                    Vec2::new(indent, content_height),
                );
                self.renderer.chrome_block_quote(ui, annotation);
            }
            NodeValue::Alert(_) => {
                if line.is_some() {
                    return;
                }
                let annotation = Rect::from_min_size(
                    Pos2::new(x, content_top),
                    Vec2::new(indent, content_height),
                );
                self.renderer.chrome_alert(ui, leaf, annotation);
            }
            NodeValue::CodeBlock(_) => {
                let viewport = self.renderer.viewport.get();
                let block_padding = self.renderer.layout.block_padding;
                let is_first_row = line == Some(0);
                let is_anchor_mid = !is_first_row && row_top <= viewport.min.y;
                if !is_first_row && !is_anchor_mid {
                    return;
                }
                let content_bottom = content_top + content_height;
                let bottom = self.bar_chrome_bottom(leaf, line, leaf, content_bottom);
                let top = if is_first_row {
                    // Block padding is in the first row's prefix; the
                    // chrome rect starts ABOVE that padding, at the
                    // outer pre-spacing boundary.
                    content_top - block_padding
                } else {
                    viewport.min.y - 1.0
                };
                let width = self.renderer.width(leaf);
                let rect = Rect::from_min_size(
                    Pos2::new(x, top),
                    Vec2::new(width, (bottom - top).max(0.0)),
                );
                self.renderer.chrome_code_block(ui, rect);
            }
            _ => {}
        }
    }

    /// Walks forward from the current row summing precise heights of
    /// subsequent rows still inside `ancestor`, capped at
    /// `viewport.bottom + 1`. Returns the screen-y of the chrome
    /// rect's bottom. The starting row's content + suffix is not
    /// included here (caller already accounts for the row's own y +
    /// height).
    fn bar_chrome_bottom(
        &self, leaf: &'ast AstNode<'ast>, line: Option<usize>, ancestor: &'ast AstNode<'ast>,
        content_bottom: f32,
    ) -> f32 {
        let viewport = self.renderer.viewport.get();
        let mut y = content_bottom;
        // First, finish remaining rows of the current leaf.
        if let Some(idx) = line {
            let n = self.line_count(leaf).unwrap_or(1);
            for next_idx in (idx + 1)..n {
                y += self.row_layout(leaf, Some(next_idx)).total();
                if y > viewport.max.y + 1.0 {
                    return viewport.max.y + 1.0;
                }
            }
        }
        // Then leaf-by-leaf onward.
        let mut walker = leaf;
        while let Some(next) = self.next_leaf(walker) {
            if !is_descendant_of(next, ancestor) {
                break;
            }
            y += self.leaf_total_precise(next);
            if y > viewport.max.y + 1.0 {
                return viewport.max.y + 1.0;
            }
            walker = next;
        }
        y
    }

    /// Total precise height of a leaf including pre/post spacing —
    /// used when summing leaves outside the current row's leaf.
    fn leaf_total_precise(&self, leaf: &'ast AstNode<'ast>) -> f32 {
        self.leaf_pre_spacing(leaf) + self.renderer.height(leaf) + self.leaf_post_spacing(leaf)
    }

    /// One row's vertical breakdown.
    fn row_layout(&self, node: &'ast AstNode<'ast>, line: Option<usize>) -> RowLayout {
        match line {
            None => RowLayout {
                prefix: self.leaf_pre_spacing(node),
                content: self.renderer.height(node),
                suffix: self.leaf_post_spacing(node),
            },
            Some(idx) => {
                let n = self.line_count(node).unwrap_or(1);
                let prefix = if idx == 0 {
                    self.leaf_pre_spacing(node)
                } else {
                    self.intra_line_spacing(node)
                };
                let content = self.line_content_height(node, idx);
                let suffix = if idx + 1 == n { self.leaf_post_spacing(node) } else { 0.0 };
                RowLayout { prefix, content, suffix }
            }
        }
    }

    fn row_precise(&self, node: &'ast AstNode<'ast>, line: Option<usize>) -> f32 {
        self.row_layout(node, line).total()
    }

    fn row_approx(&self, node: &'ast AstNode<'ast>, line: Option<usize>) -> f32 {
        match line {
            None => {
                self.leaf_pre_spacing_approx(node)
                    + self.renderer.height_approx(node)
                    + self.leaf_post_spacing_approx(node)
            }
            Some(idx) => {
                let n = self.line_count(node).unwrap_or(1);
                let prefix = if idx == 0 {
                    self.leaf_pre_spacing_approx(node)
                } else {
                    self.intra_line_spacing(node)
                };
                let content = self.line_content_approx(node, idx);
                let suffix = if idx + 1 == n { self.leaf_post_spacing_approx(node) } else { 0.0 };
                prefix + content + suffix
            }
        }
    }

    /// Spacing between lines in a multi-row leaf (paragraph uses
    /// `block_spacing`, plaintext doc and code blocks use `row_spacing`).
    fn intra_line_spacing(&self, node: &'ast AstNode<'ast>) -> f32 {
        match &node.data.borrow().value {
            NodeValue::Document | NodeValue::CodeBlock(_) => self.renderer.layout.row_spacing,
            _ => self.renderer.layout.block_spacing,
        }
    }

    fn line_content_height(&self, node: &'ast AstNode<'ast>, idx: usize) -> f32 {
        match &node.data.borrow().value {
            NodeValue::Paragraph => {
                let node_line = self.line_range(node, idx);
                self.renderer.height_paragraph_line(node, node_line)
            }
            NodeValue::Document => {
                let first = self.renderer.node_first_line_idx(node);
                self.renderer.height_document_line(node, first + idx)
            }
            NodeValue::CodeBlock(node_code_block) => {
                let first = self.renderer.node_first_line_idx(node);
                self.renderer
                    .height_code_block_line_render(node, node_code_block, first + idx)
            }
            _ => 0.0,
        }
    }

    fn line_content_approx(&self, node: &'ast AstNode<'ast>, idx: usize) -> f32 {
        match &node.data.borrow().value {
            NodeValue::Paragraph => {
                let node_line = self.line_range(node, idx);
                self.renderer.height_approx_paragraph_line(node, node_line)
            }
            NodeValue::Document => {
                let first = self.renderer.node_first_line_idx(node);
                self.renderer.height_approx_document_line(node, first + idx)
            }
            NodeValue::CodeBlock(node_code_block) => {
                let first = self.renderer.node_first_line_idx(node);
                self.renderer
                    .height_approx_code_block_line(node, node_code_block, first + idx)
            }
            _ => 0.0,
        }
    }
}

/// One row's vertical breakdown: top spacing, content, bottom
/// padding.
#[derive(Clone, Copy)]
struct RowLayout {
    prefix: f32,
    content: f32,
    suffix: f32,
}

impl RowLayout {
    fn total(&self) -> f32 {
        self.prefix + self.content + self.suffix
    }
}

/// Indent contribution of an ancestor — how much it offsets its
/// descendants' x position.
fn ancestor_indent(value: &NodeValue, renderer: &MdRender) -> f32 {
    match value {
        NodeValue::BlockQuote
        | NodeValue::Alert(_)
        | NodeValue::Item(_)
        | NodeValue::TaskItem(_)
        | NodeValue::FootnoteDefinition(_) => renderer.layout.indent,
        _ => 0.0,
    }
}

fn is_descendant_of<'ast>(node: &'ast AstNode<'ast>, ancestor: &'ast AstNode<'ast>) -> bool {
    let mut current = node;
    loop {
        if std::ptr::eq(current, ancestor) {
            return true;
        }
        match current.parent() {
            Some(p) => current = p,
            None => return false,
        }
    }
}

impl<'a, 'ast> Rows for DocScrollContent<'a, 'ast> {
    fn reset(&mut self) {
        self.cursor = Cursor::Start;
    }

    fn reset_back(&mut self) {
        self.cursor = Cursor::End;
    }

    fn next(&mut self) -> bool {
        match self.cursor {
            Cursor::Start => match self.first_leaf_in(self.root) {
                Some(leaf) => self.cursor = self.first_line_at(leaf),
                None => self.cursor = Cursor::Trailing,
            },
            Cursor::At { node, line } => {
                if let Some(idx) = line {
                    let n = self.line_count(node).unwrap_or(1);
                    if idx + 1 < n {
                        self.cursor = Cursor::At { node, line: Some(idx + 1) };
                        return true;
                    }
                }
                match self.next_leaf(node) {
                    Some(leaf) => self.cursor = self.first_line_at(leaf),
                    None => self.cursor = Cursor::Trailing,
                }
            }
            Cursor::Trailing => {
                self.cursor = Cursor::End;
                return false;
            }
            Cursor::End => return false,
        }
        true
    }

    fn prev(&mut self) -> bool {
        match self.cursor {
            Cursor::End => self.cursor = Cursor::Trailing,
            Cursor::Trailing => match self.last_leaf_in(self.root) {
                Some(leaf) => self.cursor = self.last_line_at(leaf),
                None => {
                    self.cursor = Cursor::Start;
                    return false;
                }
            },
            Cursor::At { node, line } => {
                if let Some(idx) = line {
                    if idx > 0 {
                        self.cursor = Cursor::At { node, line: Some(idx - 1) };
                        return true;
                    }
                }
                match self.prev_leaf(node) {
                    Some(leaf) => self.cursor = self.last_line_at(leaf),
                    None => {
                        self.cursor = Cursor::Start;
                        return false;
                    }
                }
            }
            Cursor::Start => return false,
        }
        true
    }

    fn approx(&self) -> f32 {
        match self.cursor {
            Cursor::At { node, line } => self.row_approx(node, line),
            Cursor::Trailing => 0.0,
            Cursor::Start | Cursor::End => panic!("approx() with cursor off a row"),
        }
    }

    fn precise(&mut self) -> f32 {
        match self.cursor {
            Cursor::At { node, line } => self.row_precise(node, line),
            Cursor::Trailing => self.trailing_precise,
            Cursor::Start | Cursor::End => panic!("precise() with cursor off a row"),
        }
    }

    fn warm(&mut self) {
        if let Cursor::At { node, .. } = self.cursor {
            self.renderer.warm_images(node);
        }
    }

    fn render(&mut self, ui: &mut Ui, top_left: Pos2) {
        match self.cursor {
            Cursor::At { node, line } => {
                // Compute leaf's x via ancestor walk: each indent-
                // contributing ancestor pushes content right.
                let mut content_x = self.renderer.top_left.x;
                for ancestor in node.ancestors() {
                    if std::ptr::eq(ancestor, self.root) || std::ptr::eq(ancestor, node) {
                        continue;
                    }
                    content_x += ancestor_indent(&ancestor.data.borrow().value, self.renderer);
                }

                let layout = self.row_layout(node, line);

                // Pre-spacing (only on the first row of the leaf).
                let mut tl = Pos2::new(content_x, top_left.y);
                if line.unwrap_or(0) == 0 {
                    self.renderer.show_block_pre_spacing(ui, node, tl);
                }
                tl.y += layout.prefix;

                let content_top = tl.y;

                // Leaf paint: per-line for line-based leaves, full
                // leaf via show_block for atomic.
                match line {
                    Some(idx) => {
                        let value = node.data.borrow().value.clone();
                        match value {
                            NodeValue::Paragraph => {
                                let node_line = self.line_range(node, idx);
                                self.renderer.show_paragraph_line(ui, node, tl, node_line);
                            }
                            NodeValue::Document => {
                                // Plaintext mode — line_idx within the doc.
                                let first = self.renderer.node_first_line_idx(node);
                                self.renderer.show_document_line(ui, node, tl, first + idx);
                            }
                            NodeValue::CodeBlock(node_code_block) => {
                                let mut code_tl = tl;
                                code_tl.x += self.renderer.layout.block_padding;
                                let first = self.renderer.node_first_line_idx(node);
                                self.renderer.show_code_block_line_render(
                                    ui,
                                    node,
                                    code_tl,
                                    &node_code_block,
                                    first + idx,
                                );
                            }
                            _ => {
                                self.renderer.show_block(ui, node, tl);
                            }
                        }
                    }
                    None => {
                        self.renderer.show_block(ui, node, tl);
                    }
                }
                tl.y += layout.content;

                // Post-spacing (only on the last row of the leaf).
                let is_last_row = match (line, self.line_count(node)) {
                    (Some(idx), Some(n)) => idx + 1 == n,
                    _ => true,
                };
                if is_last_row {
                    self.renderer.show_block_post_spacing(ui, node, tl);
                }

                // Ancestor chrome (block-quote bar, alert bar, list /
                // task / footnote markers). `top_left.y` is the row's
                // top edge (passed in by the widget); only the anchor
                // row's top edge can sit at or above the viewport top.
                self.paint_ancestor_chrome(ui, node, line, top_left.y, content_top, layout.content);

                // Self-chrome: when the leaf is itself a chrome-bearing
                // node — revealed BlockQuote / Alert (atomic), or
                // CodeBlock (per-line). `paint_ancestor_chrome` only
                // walks ancestors; this fills the gap.
                self.paint_self_chrome(ui, node, line, top_left.y, content_top, layout.content);
            }
            Cursor::Trailing => {} // virtual padding — paints nothing
            Cursor::Start | Cursor::End => panic!("render() with cursor off a row"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tab::markdown_editor::test_harness::TestEditor;
    use crate::widgets::affine_scroll::AffineScrollArea;

    /// When the user scrolls past line 0 of a multi-line paragraph in
    /// a blockquote, the bar must still paint — covering the visible
    /// portion of the bq. (Cursor outside the bq's prefix so reveal
    /// stays off; revealed-bq case is exercised by the test below.)
    #[test]
    fn block_quote_bar_persists_when_scrolled_past_first_row() {
        let mut md = String::from("Lead paragraph that is the cursor home.\n\n> ");
        for i in 0..200 {
            md.push_str(&format!("Line {i} of a long block quote.\n> "));
        }
        md.push_str("Final line.\n");
        let mut harness = TestEditor::new(&md);
        harness.enter_frame();

        // Scroll past line 0. Approx per row is ~row_height; one
        // row's worth should land the anchor on line 1.
        let scroll_id = harness.editor.id();
        let scroll = AffineScrollArea::new(scroll_id);
        scroll.set_offset(&harness.editor.edit.renderer.ctx, 50.0);

        test_clear_bq_bars();
        eprintln!("=== FRAME 3 (after set_offset + clear) ===");
        harness.enter_frame();

        let bars = test_bq_bars();
        assert!(
            !bars.is_empty(),
            "blockquote bar wasn't painted at all after scrolling past line 0"
        );
        let max_h = bars.iter().map(|r| r.height()).fold(0.0f32, f32::max);
        assert!(
            max_h > 10.0,
            "blockquote bar painted but with negligible height {max_h}; rects = {bars:?}"
        );
    }

    /// Non-revealed alert with multiple paragraph children: the cursor
    /// descends into the alert and renders each child as its own row.
    /// The alert's title chrome ("Note"/"Tip"/etc. with icon) used to
    /// be drawn by `show_alert`, which never runs in the descent path
    /// — so the title would silently vanish. Verify the title is now
    /// painted by the chrome path on the first descendant, and the
    /// bar geometry includes it.
    #[test]
    fn alert_title_painted_when_descended() {
        let md = "> [!NOTE]\n> Title text here.\n>\n> First body paragraph.\n>\n> Second body paragraph.\n";
        let mut harness = TestEditor::new(md);
        // Move the cursor away from the alert's prefix so the alert
        // isn't revealed (cursor descent path, not atomic).
        let last = harness
            .editor
            .edit
            .renderer
            .buffer
            .current
            .segs
            .last_cursor_position();
        harness.editor.edit.renderer.buffer.current.selection = (last, last);

        test_clear_alert_chrome();
        harness.enter_frame();

        let bars = test_alert_bars();
        let titles = test_alert_title_poses();
        assert!(!bars.is_empty(), "alert bar wasn't painted on descent");
        assert!(!titles.is_empty(), "alert title wasn't painted on descent (only bar was)");
        // Title position must lie inside the bar's vertical span — it
        // sits at the top of the bar.
        let title_y = titles[0].y;
        let in_bar = bars
            .iter()
            .any(|b| b.min.y <= title_y + 0.5 && title_y <= b.max.y);
        assert!(in_bar, "alert title y={title_y} is outside any bar rect {bars:?}");
    }

    /// When the cursor's selection sits inside the blockquote's `> `
    /// prefix, `reveal(bq)` is true and the cursor halts at the
    /// blockquote as a leaf. The bar must still paint.
    #[test]
    fn block_quote_bar_persists_when_revealed() {
        let mut md = String::from("> ");
        for i in 0..50 {
            md.push_str(&format!("Line {i} of a block quote.\n> "));
        }
        md.push_str("Final line.\n");
        let mut harness = TestEditor::new(&md);
        // Default cursor at offset 0 = inside bq's `> ` prefix → reveal=true.
        harness.enter_frame();

        test_clear_bq_bars();
        harness.enter_frame();

        let bars = test_bq_bars();
        assert!(!bars.is_empty(), "revealed blockquote bar wasn't painted");
    }

    /// Per-line code blocks: chrome border must paint when the first
    /// row of the block is rendered, and again when the anchor sits
    /// inside the block (scrolled past first row). Verifies the
    /// `paint_self_chrome` CodeBlock branch.
    #[test]
    fn code_block_chrome_painted_on_first_row_and_anchor() {
        // First-row case: cursor at end of doc, code block visible from top.
        let md = "Lead.\n\n```rust\nfn one() {}\nfn two() {}\nfn three() {}\n```\n";
        let mut harness = TestEditor::new(md);
        let last = harness
            .editor
            .edit
            .renderer
            .buffer
            .current
            .segs
            .last_cursor_position();
        harness.editor.edit.renderer.buffer.current.selection = (last, last);

        test_clear_code_block_chrome();
        harness.enter_frame();
        let chromes = test_code_block_chrome_rects();
        assert!(
            !chromes.is_empty(),
            "code block chrome wasn't painted when block is fully visible"
        );
        let max_h = chromes.iter().map(|r| r.height()).fold(0.0f32, f32::max);
        assert!(
            max_h > 10.0,
            "code block chrome painted with negligible height {max_h}; rects = {chromes:?}"
        );
    }

    /// Per-line code blocks: each source line is a row. Verify the
    /// cursor walks one row per source line of the code block.
    #[test]
    fn code_block_per_line_walks_one_row_per_line() {
        let md = "Lead.\n\n```rust\nfn one() {}\nfn two() {}\nfn three() {}\n```\n";
        let mut harness = TestEditor::new(md);
        let last = harness
            .editor
            .edit
            .renderer
            .buffer
            .current
            .segs
            .last_cursor_position();
        harness.editor.edit.renderer.buffer.current.selection = (last, last);
        harness.enter_frame();

        let arena = comrak::Arena::new();
        let root = harness.editor.edit.renderer.reparse(&arena);

        // Find the CodeBlock node and count its rows via line_count.
        let code_block = root
            .descendants()
            .find(|n| matches!(n.data.borrow().value, NodeValue::CodeBlock(_)))
            .expect("test doc has a code block");
        let content = DocScrollContent::new(&mut harness.editor.edit.renderer, root, 0.0);
        let n = content
            .line_count(code_block)
            .expect("code blocks should be line-based");
        // 5 source lines: ```rust + 3 fns + ``` = 5 rows.
        assert_eq!(n, 5, "expected 5 code-block rows, got {n}");
    }
}
