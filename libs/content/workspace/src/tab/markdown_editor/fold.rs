//! Section folding. A fold tag ([`FOLD_TAG`]) at the end of a heading or
//! list item's first line hides the section's contents and renders as a
//! `···` chip. The chip *is* the folded text: a cursor right of it has
//! cursored *past* the section, a selection containing it stands for the
//! contents it hides, and edits against it resolve whole-fold — grow
//! over the contents, engulf the tag, or unfold — never touching hidden
//! text. Find reveals hidden contents without unfolding; everywhere
//! else, a selection endpoint landing in hidden contents unfolds the
//! section, keeping every cursor position visible.

use comrak::Arena;
use comrak::nodes::{AstNode, NodeHeading, NodeValue};
use lb_rs::model::text::buffer;
use lb_rs::model::text::offset_types::{Grapheme, IntoRangeExt as _, RangeExt as _};
use lb_rs::model::text::operation_types::{Operation, Replace};

use crate::tab::markdown_editor::bounds::FoldBounds;
use crate::tab::markdown_editor::input::{Advance, Bound, Increment, Region};
use crate::tab::markdown_editor::widget::utils::wrap_layout::{
    FontFamily, Format, Layout, StyleInfo,
};
use crate::tab::markdown_editor::{Event, MdEdit, MdRender};
use crate::theme::icons::Icon;

pub const FOLD_TAG: &str = "<!-- {\"fold\":true} -->";

/// Visible glyph of the chip a folded section's tag renders as — a
/// single icon, so the dots are spaced as drawn rather than as three
/// text glyphs ([`Icon::DOTS_HORIZONTAL`]).
pub const FOLD_CHIP_TEXT: &str = Icon::DOTS_HORIZONTAL.icon;

/// Click-target id for a fold's chip. Keyed by tag range so layout and
/// interaction handling derive the same id without the AST node.
fn fold_chip_id_salt(tag: (Grapheme, Grapheme)) -> egui::Id {
    egui::Id::new(("md_fold_chip", tag.start().0, tag.end().0))
}

// ─── fold bounds ─────────────────────────────────────────────────────

impl<'ast> MdRender {
    /// The hidden contents of a foldable node — [`Self::heading_contents`]
    /// or [`Self::item_contents`] by node type.
    pub fn fold_contents(&self, node: &'ast AstNode<'ast>) -> Option<(Grapheme, Grapheme)> {
        match node.data.borrow().value {
            NodeValue::Heading(_) => Some(self.heading_contents(node)),
            NodeValue::Item(_) | NodeValue::TaskItem(_) => Some(self.item_contents(node)),
            _ => None,
        }
    }

    /// Recompute [`super::bounds::Bounds::folds`] — the tag and hidden-
    /// contents ranges of each actively folded section. Depends on text
    /// only.
    pub fn calc_fold_bounds(&mut self, root: &'ast AstNode<'ast>) {
        let mut folds = Vec::new();
        for node in root.descendants() {
            if let Some(fold) = self.fold(node) {
                if let Some(contents) = self.fold_contents(node) {
                    folds.push(FoldBounds { tag: self.node_range(fold), contents });
                }
            }
        }
        folds.sort_unstable_by_key(|f| f.tag);
        self.bounds.folds = folds;
    }

    /// The active fold whose tag occupies exactly `tag`, if any. A
    /// `FOLD_TAG` comment that isn't folding anything (e.g. in a plain
    /// paragraph) has no entry and renders as regular inline html.
    pub fn active_fold_at_tag(&self, tag: (Grapheme, Grapheme)) -> Option<FoldBounds> {
        self.bounds.folds.iter().find(|f| f.tag == tag).copied()
    }
}

// ─── find reveal ─────────────────────────────────────────────────────

impl MdRender {
    /// Ranges that reveal *folded contents* without unfolding: the
    /// current find match and preview. The selection is deliberately
    /// excluded — a selection endpoint landing in folded contents
    /// unfolds for real instead ([`Self::unfold_ops_for_selection`]).
    pub fn fold_reveal_ranges(&self) -> impl Iterator<Item = (Grapheme, Grapheme)> + '_ {
        self.find_current_match
            .into_iter()
            .chain(self.preview_match)
    }

    /// Returns true if `range` contains any fold-reveal range.
    pub fn range_contains_fold_revealed(
        &self, range: (Grapheme, Grapheme), allow_empty_range: bool, allow_empty_selection: bool,
    ) -> bool {
        self.fold_reveal_ranges()
            .any(|rr| range.contains_range(&rr, allow_empty_range, allow_empty_selection))
    }

    /// Returns true if a fold-reveal range force-reveals this fold's
    /// contents.
    pub fn fold_revealed(&self, fold: &FoldBounds) -> bool {
        self.range_contains_fold_revealed(fold.contents, false, true)
    }
}

// ─── chip rendering + interaction ────────────────────────────────────

impl<'ast> MdRender {
    /// Lay out an active fold's tag: a `···` chip while the contents are
    /// hidden, a zero-width anchor while a find match force-reveals them
    /// (the chip would misread as hidden content). Never tag source.
    pub fn layout_fold_chip(
        &self, layout: &mut Layout, parent: &'ast AstNode<'ast>, fold: FoldBounds,
        node_range: (Grapheme, Grapheme),
    ) {
        if self.fold_revealed(&fold) {
            layout.push_override(node_range, "", self.text_format_html_inline(parent));
            return;
        }
        // Breathing room between the preceding text and the chip,
        // outside the capsule. Zero-length source: a click here lands
        // left of the chip.
        layout.push_override(node_range.start().into_range(), " ", self.text_format(parent));
        // Interaction outside the style scope so the capsule's side
        // pads are part of the click target.
        let format = Format { family: FontFamily::Icons, ..self.text_format_html_inline(parent) };
        layout.interaction_open(fold_chip_id_salt(fold.tag), egui::Sense::click());
        layout.style_open(StyleInfo {
            format: format.clone(),
            source_range: node_range,
            chip: true,
        });
        layout.push_override(node_range, FOLD_CHIP_TEXT, format);
        layout.style_close();
        layout.interaction_close();
    }

    /// Expand a folded section when its `···` chip is clicked.
    /// Must run after `interact_fragments`.
    pub fn handle_fold_interactions(&mut self, ui: &egui::Ui) {
        if !self.interactive {
            return;
        }
        let parent_base = ui.id();
        for i in 0..self.bounds.folds.len() {
            let fold = self.bounds.folds[i];
            let id = parent_base.with(fold_chip_id_salt(fold.tag));
            let Some(response) = self.interaction_responses.get(&id).cloned() else {
                continue;
            };

            // iOS routes touches through `touch_consuming_rects` —
            // without this entry a tap on the chip would place the
            // cursor instead of reaching the expand handler below.
            self.touch_consuming_rects.push(response.rect);

            if response.hovered() {
                ui.ctx()
                    .output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
            }
            if response.clicked() {
                self.render_events.push(Event::Replace {
                    region: fold.tag.into(),
                    text: "".into(),
                    advance_cursor: false,
                });
            }
            response.on_hover_text("Show Contents");
        }
    }
}

// ─── navigation + selection atoms ────────────────────────────────────

impl MdRender {
    /// Folded regions are atoms for navigation: an advance landing
    /// inside a fold tag or its hidden contents continues out in the
    /// travel direction — forward motion past the chip resumes at the
    /// next visible position after the section. Find-revealed contents
    /// stay walkable. Loops to a fixpoint so nested folds snap all the
    /// way out.
    pub fn snap_offset_out_of_folds(&self, mut offset: Grapheme, backwards: bool) -> Grapheme {
        let last = self.buffer.current.segs.last_cursor_position();
        loop {
            let mut moved = false;
            for f in &self.bounds.folds {
                if f.tag_interior(offset) {
                    offset = if backwards { f.tag.start() } else { f.tag.end() };
                    moved = true;
                }
                if f.conceals(offset) && !self.fold_revealed(f) {
                    offset = if backwards {
                        f.contents.start()
                    } else if f.contents.end() < last {
                        f.contents.end() + 1
                    } else {
                        // hidden through the end of the doc: nothing
                        // visible forward; rest at the near edge
                        f.contents.start()
                    };
                    moved = true;
                }
            }
            if !moved {
                return offset;
            }
        }
    }

    /// Fold tags are atoms for selection: an endpoint strictly inside
    /// one snaps out, so no selection can clip a tag (and an edit over
    /// the selection can't leave partial tag text behind). A non-empty
    /// selection grows to cover the tag; a cursor collapses to the
    /// nearer edge.
    pub fn snap_selection_out_of_fold_tags(
        &self, mut range: (Grapheme, Grapheme),
    ) -> (Grapheme, Grapheme) {
        for f in &self.bounds.folds {
            if range.is_empty() {
                if f.tag_interior(range.0) {
                    let mid = f.tag.start().0 + (f.tag.end().0 - f.tag.start().0) / 2;
                    let edge = if range.0.0 < mid { f.tag.start() } else { f.tag.end() };
                    return (edge, edge);
                }
            } else {
                // Each endpoint moves away from the other (orientation-
                // agnostic), growing the selection over the atom.
                if f.tag_interior(range.0) {
                    range.0 = if range.0 <= range.1 { f.tag.start() } else { f.tag.end() };
                }
                if f.tag_interior(range.1) {
                    range.1 = if range.1 <= range.0 { f.tag.start() } else { f.tag.end() };
                }
            }
        }
        range
    }
}

// ─── capsule semantics for edits ─────────────────────────────────────

impl MdRender {
    /// The chip stands for the contents it hides: an edit range that
    /// contains a fold's whole capsule (its tag) grows over that fold's
    /// hidden contents, so deleting or typing over the selection edits
    /// what it visibly includes. Find-revealed folds are exempt — their
    /// contents are on screen and visibly outside the selection.
    /// Returns an ordered range.
    pub fn grow_range_over_selected_folds(
        &self, range: (Grapheme, Grapheme),
    ) -> (Grapheme, Grapheme) {
        let mut range = (range.start(), range.end());
        for f in &self.bounds.folds {
            if range.0 <= f.tag.start() && range.1 >= f.tag.end() && !self.fold_revealed(f) {
                range.1 = range.1.max(f.contents.end());
            }
        }
        range
    }

    /// Fold tags delete atomically: a deletion range clipping one (e.g.
    /// backspace right of the chip, whose advance snapped over the tag
    /// atom) widens to consume the whole tag, never leaving partial tag
    /// text. A deletion covering all the hidden contents takes the tag
    /// too — a fold with nothing left to hide would orphan into inert
    /// text. Returns an ordered range.
    pub fn widen_delete_over_folds(&self, range: (Grapheme, Grapheme)) -> (Grapheme, Grapheme) {
        let mut range = (range.start(), range.end());
        for f in &self.bounds.folds {
            let clips_tag = range.0 < f.tag.end() && range.1 > f.tag.start();
            let covers_contents = range.0 <= f.tag.end() && range.1 >= f.contents.end();
            if clips_tag || covers_contents {
                range.0 = range.0.min(f.tag.start());
                range.1 = range.1.max(f.tag.end());
            }
        }
        range
    }

    /// Operations that unfold any folded section a selection endpoint
    /// landed strictly inside of, so every cursor position is visible.
    /// Plain navigation steps past folded regions instead; this catches
    /// the remaining paths (programmatic selects, edits that strand the
    /// selection in hidden text). A selection that contains the entire
    /// folded contents (e.g. select-all) spans the fold rather than
    /// entering it and keeps it folded.
    pub fn unfold_ops_for_selection(&self) -> Vec<Operation> {
        let selection = self.buffer.current.selection;
        let mut ops = Vec::new();
        for f in &self.bounds.folds {
            if selection.contains_range(&f.contents, true, true) {
                continue;
            }
            if f.conceals(selection.start()) || f.conceals(selection.end()) {
                ops.push(Operation::Replace(Replace { range: f.tag, text: "".into() }));
            }
        }
        ops
    }
}

// ─── event hooks ─────────────────────────────────────────────────────

impl<'ast> MdEdit {
    /// Apply [`MdRender::unfold_ops_for_selection`], reparsing on
    /// change so this frame renders the unfolded state.
    pub fn unfold_at_selection<'a>(&mut self, arena: &'a Arena<'a>) -> buffer::Response {
        let ops = self.renderer.unfold_ops_for_selection();
        if ops.is_empty() {
            return buffer::Response::default();
        }
        self.renderer.buffer.queue(ops);
        let response = self.renderer.buffer.update();
        self.renderer.bump_text_seq();
        self.renderer.reparse(arena);
        response
    }

    /// Enter right of a folded node's `···` chip creates the new
    /// heading / list item *after* the hidden contents instead of
    /// splitting into them. True if handled (ops + cursor pushed).
    pub fn fold_newline(&self, root: &'ast AstNode<'ast>, operations: &mut Vec<Operation>) -> bool {
        // selection must be empty
        let Some(offset) = self.renderer.selection_offset() else {
            return false;
        };
        for node in root.descendants() {
            if self.renderer.fold(node).is_none() {
                continue;
            }
            let Some(contents) = self.renderer.fold_contents(node) else {
                continue;
            };
            if offset != contents.start() {
                continue;
            }
            let prefix = match &node.data.borrow().value {
                NodeValue::Heading(NodeHeading { level, .. }) => "#".repeat(*level as usize) + " ",
                _ => match self.renderer.insertion_prefix(node) {
                    Some(prefix) => prefix,
                    None => return false,
                },
            };
            let insert_at = contents.end().into_range();
            operations.push(Operation::Replace(Replace {
                range: insert_at,
                text: format!("\n{prefix}"),
            }));
            // at the insertion point, so advances past the inserted text
            operations.push(Operation::Select(insert_at));
            return true;
        }
        false
    }

    /// Deleting against a folded region unfolds it instead of editing
    /// hidden text: backspace at the first position after the section
    /// (which would join visible text into it) or forward-delete right
    /// of the chip removes the fold tag and nothing else. True if
    /// handled (ops + cursor pushed).
    pub fn fold_delete(&self, region: Region, operations: &mut Vec<Operation>) -> bool {
        let Region::SelectionOrAdvance {
            advance: Advance::Next(Bound::Word) | Advance::By(Increment::Char),
            backwards,
        } = region
        else {
            return false;
        };
        // selection must be empty
        let Some(offset) = self.renderer.selection_offset() else {
            return false;
        };
        for f in &self.renderer.bounds.folds {
            let against = if backwards {
                offset == f.contents.end() + 1
            } else {
                offset == f.contents.start()
            };
            if !against {
                continue;
            }
            // Backspace on an *empty* line after the folded region
            // deletes the line: nothing visible joins into hidden text,
            // and the cursor comes to rest right of the chip.
            if backwards && self.renderer.line_at_offset(offset).is_empty() {
                operations.push(Operation::Replace(Replace {
                    range: (f.contents.end(), f.contents.end() + 1),
                    text: "".into(),
                }));
                operations.push(Operation::Select(f.contents.start().into_range()));
                return true;
            }
            // Otherwise the deletion would edit hidden text (join into
            // it / erase from it): unfold instead.
            operations.push(Operation::Replace(Replace { range: f.tag, text: "".into() }));
            operations.push(Operation::Select(offset.into_range()));
            return true;
        }
        false
    }
}
