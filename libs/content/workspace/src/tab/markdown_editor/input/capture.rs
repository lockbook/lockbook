use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use crate::tab::markdown_editor;
use lb_rs::text::offset_types::{DocCharOffset, RangeExt as _, RangeIterExt};
use lb_rs::text::unicode_segs::UnicodeSegs;
use markdown_editor::appearance::{Appearance, CaptureCondition};
use markdown_editor::ast::{Ast, AstTextRangeType};
use markdown_editor::bounds::{AstTextRanges, Bounds, RangesExt as _};
use markdown_editor::galleys::Galleys;
use markdown_editor::input::{cursor::PointerState, mutation};

pub const HOVER_REVEAL_DEBOUNCE: Duration = Duration::from_millis(300);

pub struct CaptureState {
    unprocessed_changes: bool,
    now: Instant,
    hovered_at_by_ast_text_range: HashMap<usize, Instant>,
}

impl Default for CaptureState {
    fn default() -> Self {
        Self::new()
    }
}

impl CaptureState {
    pub fn new() -> Self {
        Self {
            unprocessed_changes: false,
            now: Instant::now(),
            hovered_at_by_ast_text_range: HashMap::new(),
        }
    }

    /// Updates the state of the hover reveal mechanism. Call this every frame after text layout so that the galleys
    /// are synchronized with other parameters.
    pub fn update(
        &mut self, now: Instant, pointer_state: &PointerState, galleys: &Galleys,
        segs: &UnicodeSegs, bounds: &Bounds, ast: &Ast,
    ) {
        if self
            .hovered_at_by_ast_text_range
            .iter()
            .any(|(_, &hovered_at)| {
                let reveal_at = hovered_at + HOVER_REVEAL_DEBOUNCE;
                self.now < reveal_at && reveal_at <= now
            })
        {
            self.unprocessed_changes |= true;
        }
        self.now = now;

        if let Some(pos) = pointer_state.pointer_pos {
            let pointer = mutation::pos_to_char_offset(pos, galleys, segs, &bounds.text);

            // revealed ranges are those whose ast nodes are hovered
            let mut revealed_ranges = HashSet::new();
            for ast_range_idx in bounds.ast.find_containing(pointer, true, true).iter() {
                let ast_text_range = &bounds.ast[ast_range_idx];
                for &ancestor_ast_node_idx in ast_text_range.ancestors.iter() {
                    let ast_node = &ast.nodes[ancestor_ast_node_idx];

                    if !ast_node.head_range().is_empty() {
                        let head_range_idx =
                            bounds.ast.find_containing(ast_node.range.0, true, false).0;
                        revealed_ranges.insert(head_range_idx);
                    }
                    if !ast_node.tail_range().is_empty() {
                        let tail_range_idx =
                            bounds.ast.find_containing(ast_node.range.1, false, true).0;
                        revealed_ranges.insert(tail_range_idx);
                    }
                }
            }
            revealed_ranges.retain(|&range| bounds.ast[range].range_type != AstTextRangeType::Text);

            // remove ranges that are no longer hovered
            let pre_count = self.hovered_at_by_ast_text_range.len();
            self.hovered_at_by_ast_text_range
                .retain(|ast_range_idx, _| revealed_ranges.contains(ast_range_idx));
            if pre_count != self.hovered_at_by_ast_text_range.len() {
                self.unprocessed_changes |= true;
            }

            // add ranges that are newly hovered
            for ast_range_idx in revealed_ranges {
                let ast_text_range = &bounds.ast[ast_range_idx];
                if ast_text_range.range_type == AstTextRangeType::Text {
                    continue; // only head and tail ranges are ever captured
                }

                self.hovered_at_by_ast_text_range
                    .entry(ast_range_idx)
                    .or_insert(now);
            }
        }
    }

    /// Marks changes to capture state as processed. Returns true if there were unprocessed changes.
    pub fn mark_changes_processed(&mut self) -> bool {
        let unprocessed_changes = self.unprocessed_changes;
        self.unprocessed_changes = false;
        unprocessed_changes
    }

    /// Returns true if the given AST text range should be captured for any reason, including cursor selection or hover
    /// reveal. Debounce is evaluated using the time of last update rather than the current time to facilitate change
    /// detection.
    pub fn captured(
        &self, selection: (DocCharOffset, DocCharOffset), ast: &Ast, ast_ranges: &AstTextRanges,
        ast_range_idx: usize, selecting: bool, appearance: &Appearance,
    ) -> bool {
        let ast_text_range = &ast_ranges[ast_range_idx];
        if ast_text_range.range_type == AstTextRangeType::Text {
            return false;
        }

        // check if the ast node for this range intersects the selection
        let ast_node = &ast.nodes[ast_text_range.ancestors.last().copied().unwrap_or_default()];
        let node_intersects_selection = ast_node.range.intersects(&selection, true) && !selecting;

        // check if the pointer is hovering this text range with a satisfied debounce
        let hovered = self
            .hovered_at_by_ast_text_range
            .get(&ast_range_idx)
            .map(|hovered_at| *hovered_at + HOVER_REVEAL_DEBOUNCE <= self.now)
            .unwrap_or(false);

        match appearance.markdown_capture(ast_text_range.node(ast).node_type()) {
            CaptureCondition::Always => true,
            CaptureCondition::NoCursor => !(node_intersects_selection || hovered),
            CaptureCondition::Never => false,
        }
    }

    /// Returns the duration until hover reveal should be recalculated, if any. This is the minimum time until debounce
    /// is newly satisfied for a hovered AST range or zero if an AST range has been un-hovered since the last call to
    /// `clear`. Call this when determining when to repaint (after calling `update`, which affects the result).
    pub fn recalc_after(&self) -> Option<Duration> {
        if self.unprocessed_changes {
            return Some(Duration::ZERO);
        }

        let mut reveals = Vec::new();
        for &hovered_at in self.hovered_at_by_ast_text_range.values() {
            let reveal_at = hovered_at + HOVER_REVEAL_DEBOUNCE;
            if reveal_at > self.now {
                reveals.push(reveal_at - self.now);
            }
        }

        reveals.iter().min().copied()
    }
}
