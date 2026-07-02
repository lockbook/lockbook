//! Container blocks (lists, items, blockquotes, alerts) and per-
//! line prefix math. Prefixes are graphemes (`line_own_prefix`,
//! `extension_own_prefix`); nesting depth is columns
//! (`deindent_level_cols` + helpers in [`super::super::utils`]).

use comrak::nodes::{AstNode, NodeValue};
use egui::{Pos2, Rect, Ui};
use lb_rs::model::text::offset_types::{
    Grapheme, Graphemes, IntoRangeExt, RangeExt as _, RangeIterExt as _,
};

use crate::tab::markdown_editor::MdRender;
use crate::tab::markdown_editor::bounds::RangesExt as _;
use crate::tab::markdown_editor::widget::utils::NodeValueExt as _;
use crate::tab::markdown_editor::widget::utils::wrap_layout::{
    Fragment, FragmentContent, FragmentInset,
};

pub(crate) mod alert;
pub(crate) mod block_quote;
pub(crate) mod document;
pub(crate) mod footnote_definition;
pub(crate) mod item;
pub(crate) mod list;
pub(crate) mod table;
pub(crate) mod table_row;
pub(crate) mod task_item;

impl<'ast> MdRender {
    // the height of a block that contains blocks is the sum of the heights of the blocks it contains
    pub fn block_children_height(&self, node: &'ast AstNode<'ast>) -> f32 {
        let mut height_sum = 0.0;
        for child in node.children() {
            height_sum += self.block_pre_spacing_height(child);
            height_sum += self.height(child);
            height_sum += self.block_post_spacing_height(child);
        }
        height_sum
    }

    /// Returns the document offset ranges for which fragments must
    /// exist, covering the selection ± 1 source line so arrow-key
    /// navigation across the viewport edge has a fragment to land
    /// on. Also includes the current find match.
    pub fn fragment_required_ranges(
        &self, in_progress_selection: Option<(Grapheme, Grapheme)>,
        find_match: Option<(Grapheme, Grapheme)>,
    ) -> Vec<(Grapheme, Grapheme)> {
        if self.bounds.source_lines.is_empty() {
            return Vec::new();
        }
        let mut ranges = Vec::new();
        let selection = in_progress_selection.unwrap_or(self.buffer.current.selection);
        ranges.push(self.source_line_range(selection));
        if let Some(match_range) = find_match {
            ranges.push(self.source_line_range(match_range));
        }
        if let Some(preview_range) = self.preview_match {
            ranges.push(self.source_line_range(preview_range));
        }
        ranges
    }

    fn source_line_range(&self, range: (Grapheme, Grapheme)) -> (Grapheme, Grapheme) {
        let first_line = self
            .bounds
            .source_lines
            .find_containing(range.start(), true, true)
            .0
            .saturating_sub(1);
        let last_line = self
            .bounds
            .source_lines
            .find_containing(range.end(), true, true)
            .1
            .min(self.bounds.source_lines.len() - 1);
        let start = self.bounds.source_lines[first_line].start();
        let end = self.bounds.source_lines[last_line].end();
        (start, end)
    }

    // blocks are stacked vertically; only visible blocks and blocks
    // whose node range intersects `fragment_required_ranges` are shown
    pub fn show_block_children(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, mut top_left: Pos2,
    ) {
        let required_ranges =
            self.fragment_required_ranges(self.in_progress_selection, self.find_current_match);
        let viewport = ui.clip_rect();
        let buffer = viewport.height();

        let intersects_any_required = |range: &(Grapheme, Grapheme)| -> bool {
            required_ranges.iter().any(|rr| range.intersects(rr, true))
        };
        let past_all_required =
            |offset: Grapheme| -> bool { required_ranges.iter().all(|rr| offset > rr.end()) };

        for child in node.children() {
            let child_range = self.node_range(child);
            let pre_lines = self.pre_spacing_lines(child);
            let post_lines = self.post_spacing_lines(child);

            // add pre-spacing
            let pre_spacing = self.block_pre_spacing_height(child);
            let pre_spacing_below_viewport = viewport.max.y < top_left.y;
            let pre_spacing_above_viewport = viewport.min.y > top_left.y + pre_spacing;
            let pre_spacing_visible = !pre_spacing_above_viewport && !pre_spacing_below_viewport;
            let pre_spacing_needed = intersects_any_required(&self.spacing_range(&pre_lines));
            if pre_spacing_visible || pre_spacing_needed {
                self.show_block_pre_spacing(ui, child, top_left);
            }
            top_left.y += pre_spacing;

            // add block
            let child_height = self.height(child);

            if self.debug {
                self.show_debug_block_highlight(
                    ui,
                    child,
                    top_left,
                    self.width(child),
                    child_height,
                );
            }

            let block_below_viewport = viewport.max.y < top_left.y;
            let block_above_viewport = viewport.min.y > top_left.y + child_height;
            let block_visible = !block_above_viewport && !block_below_viewport;
            let block_needed = intersects_any_required(&child_range);
            if block_visible || block_needed {
                self.show_block(ui, child, top_left);

                // Index list items for drag-to-reorder; the parent
                // `List`'s start identifies the sibling group.
                if matches!(child.data.borrow().value, NodeValue::Item(_) | NodeValue::TaskItem(_))
                {
                    let block_rect = Rect::from_min_size(
                        top_left,
                        egui::Vec2::new(self.width(child), child_height),
                    );
                    let parent_start = self.node_range(node).start();
                    self.push_block_box(child, block_rect, parent_start);
                }
            } else {
                let in_buffer = top_left.y + child_height > viewport.min.y - buffer
                    && top_left.y < viewport.max.y + buffer;
                if in_buffer {
                    // walks all descendants, not just those within the buffer —
                    // a tall container may warm images beyond the zone
                    self.warm_images(child);
                }
            }
            top_left.y += child_height;

            // add post-spacing
            let post_spacing = self.block_post_spacing_height(child);
            let post_spacing_below_viewport = viewport.max.y < top_left.y;
            let post_spacing_above_viewport = viewport.min.y > top_left.y + post_spacing;
            let post_spacing_visible = !post_spacing_above_viewport && !post_spacing_below_viewport;
            let post_spacing_needed = intersects_any_required(&self.spacing_range(&post_lines));
            if post_spacing_visible || post_spacing_needed {
                self.show_block_post_spacing(ui, child, top_left);
            }
            top_left.y += post_spacing;

            // safe to stop: everything remaining is below the buffer zone and
            // past all ranges that need galleys
            let past_buffer = top_left.y > viewport.max.y + buffer;
            if past_buffer && past_all_required(child_range.start()) {
                break;
            }
        }
    }

    /// How many leading characters on the given line belong to the given node
    /// and its ancestors? For example, in "`> * p`", the line prefix len of the
    /// block quote is 2 and of the list item is 4. This function only processes
    /// container blocks, so the paragraph is not supported.
    ///
    /// This fn's second return value indicates whether the given line is a lazy
    /// continuation line of the given node. For example, in:
    /// ```md
    /// > block quote line one
    /// block quote line two
    /// ```
    /// the second line is a lazy continuation line of the block quote.
    // "It is tempting to think of this in terms of columns: the continuation
    // blocks must be indented at least to the column of the first
    // non-whitespace character after the list marker. However, that is not
    // quite right. The spaces after the list marker determine how much relative
    // indentation is needed. Which column this indentation reaches will depend
    // on how the list item is embedded in other constructions, as shown by this
    // example:
    //
    //    > > 1.  one
    // >>
    // >>     two
    //
    // Here two occurs in the same column as the list marker 1., but is actually
    // contained in the list item, because there is sufficient indentation after
    // the last containing blockquote marker."
    //
    // https://github.github.com/gfm/#list-items
    pub fn line_prefix_len(
        &self, node: &'ast AstNode<'ast>, line: (Grapheme, Grapheme),
    ) -> (Graphemes, bool) {
        if let Some(cached) = self.get_cached_line_prefix_len(node, line) {
            return cached;
        }

        let Some(parent) = node.parent() else {
            return (0.into(), false); // document has no prefix
        };
        let (parent_prefix_len, parent_lazy) = self.line_prefix_len(parent, line);
        if parent_lazy {
            // lazy continuation status is inherited
            return (parent_prefix_len, parent_lazy);
        }

        let value = &node.data.borrow().value;
        let sp = &node.data.borrow().sourcepos;
        let maybe_own_prefix_len = match value {
            NodeValue::FrontMatter(_) => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::Raw(_) => unimplemented!("can only be created programmatically"),

            // container_block
            NodeValue::Alert(_) => self.own_prefix_len_alert(node, line),
            NodeValue::BlockQuote => self.own_prefix_len_block_quote(node, line),
            NodeValue::DescriptionItem(_) => unimplemented!("extension disabled"),
            NodeValue::DescriptionList => unimplemented!("extension disabled"),
            NodeValue::Document => Some(0.into()),
            NodeValue::FootnoteDefinition(_) => self.own_prefix_len_footnote_definition(node, line),
            NodeValue::Item(node_list) => self.own_prefix_len_item(node, line, node_list),
            NodeValue::List(_) => Some(0.into()),
            NodeValue::MultilineBlockQuote(_) => unimplemented!("extension disabled"),
            NodeValue::Table(_) => Some(0.into()),
            NodeValue::TableRow(_) => Some(0.into()),
            NodeValue::TaskItem(_) => self.own_prefix_len_task_item(node, line),

            // inline
            NodeValue::Image(_)
            | NodeValue::Code(_)
            | NodeValue::Emph
            | NodeValue::Escaped
            | NodeValue::EscapedTag(_)
            | NodeValue::FootnoteReference(_)
            | NodeValue::Highlight
            | NodeValue::HtmlInline(_)
            | NodeValue::LineBreak
            | NodeValue::Link(_)
            | NodeValue::Math(_)
            | NodeValue::ShortCode(_)
            | NodeValue::SoftBreak
            | NodeValue::SpoileredText
            | NodeValue::Strikethrough
            | NodeValue::Strong
            | NodeValue::Subscript
            | NodeValue::Subtext
            | NodeValue::Superscript
            | NodeValue::Text(_)
            | NodeValue::Underline
            | NodeValue::WikiLink(_) => unreachable!("not a container block: {} {:?}", sp, value),

            // leaf_block
            NodeValue::CodeBlock(_)
            | NodeValue::DescriptionDetails
            | NodeValue::DescriptionTerm
            | NodeValue::Heading(_)
            | NodeValue::HtmlBlock(_)
            | NodeValue::Paragraph
            | NodeValue::TableCell
            | NodeValue::ThematicBreak => {
                unimplemented!("not a container block: {} {:?}", sp, value)
            }
        };
        let result = if let Some(own_prefix_len) = maybe_own_prefix_len {
            (parent_prefix_len + own_prefix_len, false)
        } else {
            (parent_prefix_len, true)
        };

        self.set_cached_line_prefix_len(node, line, result);
        result
    }

    /// Returns the range representing the portion of the line that belongs to
    /// the given node's ancestors; comes before [`line_own_prefix`] which
    /// comes before [`line_content`].
    ///
    /// In the following example:
    ///
    /// ```md
    /// > * quoted list item
    /// ```
    ///
    /// * For the list item:
    ///   * the line ancestors prefix is "`> `"
    ///   * the line own prefix is "`* `"
    ///   * the line content is "`quoted list item`"
    /// * For the block quote:
    ///   * the line ancestors prefix is ""
    ///   * the line own prefix is "`> `"
    ///   * the line content is "`* quoted list item`"
    pub fn line_ancestors_prefix(
        &self, node: &'ast AstNode<'ast>, line: (Grapheme, Grapheme),
    ) -> (Grapheme, Grapheme) {
        let Some(parent) = node.parent() else { return line.start().into_range() }; // document has no ancestors
        let (parent_prefix_len, _) = self.line_prefix_len(parent, line);
        (line.start(), line.start() + parent_prefix_len)
    }

    /// Returns the range representing the portion of the line that constitutes
    /// the given node's prefix; comes after [`line_ancestors_prefix`] and
    /// before [`line_content`].
    ///
    /// See [`line_ancestors_prefix`] for an example.
    pub fn line_own_prefix(
        &self, node: &'ast AstNode<'ast>, line: (Grapheme, Grapheme),
    ) -> (Grapheme, Grapheme) {
        let Some(parent) = node.parent() else { return line.start().into_range() }; // document has no prefix
        let (parent_prefix_len, _) = self.line_prefix_len(parent, line);
        let (prefix_len, _) = self.line_prefix_len(node, line);

        (line.start() + parent_prefix_len, line.start() + prefix_len)
    }

    /// Returns the range representing the portion of the line after the given
    /// node's prefix; comes after [`line_ancestors_prefix`] and
    /// [`line_own_prefix`].
    ///
    /// See [`line_ancestors_prefix`] for an example.
    pub fn line_content(
        &self, node: &'ast AstNode<'ast>, line: (Grapheme, Grapheme),
    ) -> (Grapheme, Grapheme) {
        let (prefix_len, _) = self.line_prefix_len(node, line);
        (line.start() + prefix_len, line.end())
    }

    /// Returns the range representing the portion of the line before the
    /// [`node_content`]. Equivalent to [`line_ancestors_prefix`] +
    /// [`line_own_prefix`]. Always has length == [`line_prefix_len`].
    pub fn line_prefix(
        &self, node: &'ast AstNode<'ast>, line: (Grapheme, Grapheme),
    ) -> (Grapheme, Grapheme) {
        let (prefix_len, _) = self.line_prefix_len(node, line);
        (line.start(), line.start() + prefix_len)
    }

    /// Returns the string that could be prepended to a line to make that line
    /// an extension of the given node e.g. "`> `" for block quotes or "` `" for
    /// list items. Returns `None` if the node is not supported e.g. tables.
    pub fn extension_own_prefix(&self, node: &'ast AstNode<'ast>) -> Option<String> {
        let line = self.node_first_line(node);
        let own_prefix = self.line_own_prefix(node, line);

        Some(match &node.data.borrow().value {
            NodeValue::FrontMatter(_) | NodeValue::Raw(_) => {
                unreachable!("not a container block")
            }

            // container_block
            NodeValue::Alert(_) => self.buffer[own_prefix].into(),
            NodeValue::BlockQuote => self.buffer[own_prefix].into(),
            NodeValue::DescriptionItem(_) => {
                unimplemented!("extension disabled")
            }
            NodeValue::DescriptionList => unimplemented!("extension disabled"),
            NodeValue::Document => {
                return None;
            }
            NodeValue::FootnoteDefinition(_) => "  ".into(),

            NodeValue::Item(_) => " ".repeat(own_prefix.len().0),
            NodeValue::List(_) => {
                return None;
            }
            NodeValue::MultilineBlockQuote(_) => {
                unimplemented!("extension disabled")
            }
            NodeValue::Table(_) => {
                return None;
            }
            NodeValue::TableRow(_) => {
                return None;
            }
            NodeValue::TaskItem(_) => {
                if line == self.node_first_line(node) {
                    // ' [ ]' is not part of the list marker for indentation purposes
                    " ".repeat(own_prefix.len().0.saturating_sub(4))
                } else {
                    " ".repeat(own_prefix.len().0)
                }
            }

            // inline
            NodeValue::Image(_)
            | NodeValue::Code(_)
            | NodeValue::Emph
            | NodeValue::Escaped
            | NodeValue::EscapedTag(_)
            | NodeValue::FootnoteReference(_)
            | NodeValue::Highlight
            | NodeValue::HtmlInline(_)
            | NodeValue::LineBreak
            | NodeValue::Link(_)
            | NodeValue::Math(_)
            | NodeValue::ShortCode(_)
            | NodeValue::SoftBreak
            | NodeValue::SpoileredText
            | NodeValue::Strikethrough
            | NodeValue::Strong
            | NodeValue::Subscript
            | NodeValue::Subtext
            | NodeValue::Superscript
            | NodeValue::Text(_)
            | NodeValue::Underline
            | NodeValue::WikiLink(_) => unreachable!("not a container block"),

            // leaf_block
            NodeValue::CodeBlock(_)
            | NodeValue::DescriptionDetails
            | NodeValue::DescriptionTerm
            | NodeValue::Heading(_)
            | NodeValue::HtmlBlock(_)
            | NodeValue::Paragraph
            | NodeValue::TableCell
            | NodeValue::ThematicBreak => unreachable!("not a container block"),
        })
    }

    /// Columns one nesting level of `node` contributes — what
    /// shift-tab strips. `Item` uses its own `padding` (not the
    /// parent list's: comrak merges sibling-compatible lists across
    /// blank lines and the merged list keeps the first item's
    /// padding). `BlockQuote`/`Alert` return `None` — they nest via
    /// `>` markers, deindented through `line_own_prefix` instead.
    pub fn deindent_level_cols(&self, node: &'ast AstNode<'ast>) -> Option<usize> {
        match &node.data.borrow().value {
            NodeValue::Item(item) => Some(item.padding),
            NodeValue::TaskItem(_) => match &node.parent()?.data.borrow().value {
                NodeValue::List(list) => Some(list.padding),
                _ => None,
            },
            NodeValue::FootnoteDefinition(_) => Some(2),
            _ => None,
        }
    }

    /// Returns the prefix required to extend the given node and its ancestors
    /// to a new line i.e. the concatenation of [`extension_own_prefix`] of all
    /// container block ancestors.
    pub fn extension_prefix(&self, node: &'ast AstNode<'ast>) -> Option<String> {
        let mut result = if let Some(parent) = node.parent() {
            self.extension_prefix(parent)?
        } else {
            Default::default()
        };

        if let Some(own_prefix) = self.extension_own_prefix(node) {
            result += &own_prefix;
        }

        Some(result)
    }

    /// Returns the prefix required to insert a new node of the same type after
    /// the given prior node, extending all ancestors.
    pub fn insertion_prefix(&self, prior_node: &'ast AstNode<'ast>) -> Option<String> {
        let line = self.node_first_line(prior_node);

        let is_list_item =
            matches!(prior_node.data.borrow().value, NodeValue::Item(_) | NodeValue::TaskItem(_));
        let mut result = if is_list_item {
            // Preserve the prior item's actual source indentation rather than
            // reconstructing it from ancestor marker widths, which drops any
            // indentation deeper than the minimum (e.g. tab / 4-space nesting).
            self.buffer[self.line_ancestors_prefix(prior_node, line)].to_string()
        } else if let Some(parent) = prior_node.parent() {
            self.extension_prefix(parent)?
        } else {
            Default::default()
        };

        let own_prefix = self.line_own_prefix(prior_node, line);

        match &prior_node.data.borrow().value {
            NodeValue::FrontMatter(_) | NodeValue::Raw(_) => {
                unreachable!("not a container block")
            }

            // container_block
            NodeValue::Alert(_) => result += &self.buffer[own_prefix],
            NodeValue::BlockQuote => result += &self.buffer[own_prefix],
            NodeValue::DescriptionItem(_) => {
                unimplemented!("extension disabled")
            }
            NodeValue::DescriptionList => unimplemented!("extension disabled"),
            NodeValue::Document => {
                return None;
            }
            NodeValue::FootnoteDefinition(_) => result += &self.buffer[own_prefix], // unsure about this one

            NodeValue::Item(_) => result += &self.buffer[own_prefix],
            NodeValue::List(_) => {
                return None;
            }
            NodeValue::MultilineBlockQuote(_) => {
                unimplemented!("extension disabled")
            }
            NodeValue::Table(_) => {
                return None;
            }
            NodeValue::TableRow(_) => {
                return None;
            }
            NodeValue::TaskItem(node_task_item) => {
                let check = &node_task_item.symbol;
                if let Some(check) = check {
                    result += &self.buffer[own_prefix].replace(*check, " ")
                } else {
                    result += &self.buffer[own_prefix]
                }
            }

            // inline
            NodeValue::Image(_)
            | NodeValue::Code(_)
            | NodeValue::Emph
            | NodeValue::Escaped
            | NodeValue::EscapedTag(_)
            | NodeValue::FootnoteReference(_)
            | NodeValue::Highlight
            | NodeValue::HtmlInline(_)
            | NodeValue::LineBreak
            | NodeValue::Link(_)
            | NodeValue::Math(_)
            | NodeValue::ShortCode(_)
            | NodeValue::SoftBreak
            | NodeValue::SpoileredText
            | NodeValue::Strikethrough
            | NodeValue::Strong
            | NodeValue::Subscript
            | NodeValue::Subtext
            | NodeValue::Superscript
            | NodeValue::Text(_)
            | NodeValue::Underline
            | NodeValue::WikiLink(_) => unreachable!("not a container block"),

            // leaf_block
            NodeValue::CodeBlock(_)
            | NodeValue::DescriptionDetails
            | NodeValue::DescriptionTerm
            | NodeValue::Heading(_)
            | NodeValue::HtmlBlock(_)
            | NodeValue::Paragraph
            | NodeValue::TableCell
            | NodeValue::ThematicBreak => unreachable!("not a container block"),
        };

        Some(result)
    }

    /// Whether `node` contributes an indent-width gutter column —
    /// these are the containers a drag selects an indentation unit of.
    /// `List`/`Table`/`TableRow`/`Document` nest without shifting, so
    /// they own no column.
    pub fn is_gutter_level(&self, node: &'ast AstNode<'ast>) -> bool {
        matches!(
            node.data.borrow().value,
            NodeValue::Item(_)
                | NodeValue::TaskItem(_)
                | NodeValue::BlockQuote
                | NodeValue::Alert(_)
                | NodeValue::FootnoteDefinition(_)
        )
    }

    /// A gutter level whose marker lives only on its first line; later
    /// lines carry pure indentation. Such a level must reveal per-line
    /// (see [`reveal_line`]) so cursoring into a continuation line's
    /// indentation doesn't also reveal the first-line marker. Block quotes
    /// and alerts mark every line, so they reveal node-wide.
    pub fn marks_first_line_only(&self, node: &'ast AstNode<'ast>) -> bool {
        matches!(
            node.data.borrow().value,
            NodeValue::Item(_) | NodeValue::TaskItem(_) | NodeValue::FootnoteDefinition(_)
        )
    }

    /// Whether the gutter column `level` owns on `line` should render its
    /// raw source. First-line-marker levels scope to the line; block
    /// quotes/alerts reveal node-wide.
    pub fn reveal_gutter_column(
        &self, level: &'ast AstNode<'ast>, line: (Grapheme, Grapheme),
    ) -> bool {
        if self.marks_first_line_only(level) {
            self.reveal_line(level, line)
        } else {
            self.reveal(level)
        }
    }

    /// The nearest gutter-contributing ancestor of `node` (excluding
    /// `node` itself), i.e. the level whose column sits one to the left.
    fn gutter_parent(&self, node: &'ast AstNode<'ast>) -> Option<&'ast AstNode<'ast>> {
        node.ancestors().skip(1).find(|n| self.is_gutter_level(n))
    }

    /// The innermost gutter level on `line` (the deepest column), or
    /// `None` if the line has no gutter. This is the level whose
    /// `line_own_prefix` absorbs any indentation past the levels' own
    /// columns (a straddling tab's residual, over-indentation) so the
    /// columns tile the prefix.
    pub fn deepest_gutter_level(
        &self, node: &'ast AstNode<'ast>, line: (Grapheme, Grapheme),
    ) -> Option<&'ast AstNode<'ast>> {
        let root = node.ancestors().last()?;
        let deepest = self.deepest_container_block_at_offset(root, line.end());
        deepest.ancestors().find(|n| self.is_gutter_level(n))
    }

    /// Fills `map[line_idx]` with the deepest gutter container covering
    /// each source line, via a single pre-order DFS: a deeper container is
    /// visited after its ancestor, so it overwrites the ancestor on any
    /// line they share (compact nesting).
    fn assign_deepest_gutter(
        &self, node: &'ast AstNode<'ast>, map: &mut [Option<&'ast AstNode<'ast>>],
    ) {
        for child in node.children() {
            if !child.is_container_block() {
                continue;
            }
            if self.is_gutter_level(child) {
                for i in self.node_lines(child).iter() {
                    if let Some(slot) = map.get_mut(i) {
                        *slot = Some(child);
                    }
                }
            }
            self.assign_deepest_gutter(child, map);
        }
    }

    /// returns true if the syntax for a container block should be revealed
    ///
    /// Reveal `node` when a reveal-range endpoint lands strictly inside a
    /// gutter column `node` *owns the source for*. Columns render as
    /// atomic fragments, so a drag snaps to their edges and never reaches
    /// an interior — only keyboard navigation or edits do, so a
    /// click-drag never reveals syntax.
    ///
    /// An endpoint can only reveal the column it sits in, so the scan is
    /// over the (typically 0–2) reveal endpoints, one line each.
    pub fn reveal(&self, node: &'ast AstNode<'ast>) -> bool {
        for reveal_range in self.reveal_ranges() {
            for endpoint in [reveal_range.start(), reveal_range.end()] {
                let (line_idx, _) = self
                    .bounds
                    .source_lines
                    .find_containing(endpoint, true, true);
                let Some(&line) = self.bounds.source_lines.get(line_idx) else {
                    continue;
                };
                if !self.node_contains_line(node, line) {
                    continue;
                }
                if self.reveal_line(node, line) {
                    return true;
                }
            }
        }
        false
    }

    /// Like [`reveal`] but scoped to a single `line`: true when a reveal
    /// endpoint lands strictly inside the gutter column `node` owns on
    /// `line` — marker *or* leading indentation.
    ///
    /// An indentation-only own-prefix (a nested continuation line) renders
    /// collapsed to a single indent-wide column, so its interior cursor
    /// positions can't be distinguished until it reveals into per-grapheme
    /// source. Strict interiority keeps level-boundary edges (and drag,
    /// which snaps to edges) non-revealing.
    ///
    /// Line scoping matters for first-line-marker containers (items, task
    /// items, footnotes): the cursor inside a continuation line's
    /// indentation must reveal that indentation without also revealing the
    /// node's marker, which lives on a different line.
    pub fn reveal_line(&self, node: &'ast AstNode<'ast>, line: (Grapheme, Grapheme)) -> bool {
        let own = self.line_own_prefix(node, line);
        if own.is_empty() {
            return false;
        }
        self.reveal_ranges().any(|rr| {
            [rr.start(), rr.end()]
                .into_iter()
                .any(|ep| own.contains(ep, false, false))
        })
    }

    /// Registers an atomic, selectable [`Fragment`] for each gutter
    /// column (marker / per-level indentation) on `line`, laid out in
    /// the indent-width columns left of `content_top_left` (the content
    /// origin, after every prefix shift).
    ///
    /// Each column's [`line_own_prefix`] becomes one `Spacer`: a drag
    /// lands only at its edges (so a click-drag never reveals syntax —
    /// see [`reveal`]) and a selection highlights the whole column. The
    /// columns are non-atomic, so a single click places the cursor at the
    /// nearer edge rather than selecting the column; drag or double-click
    /// selects it. Marker glyphs are still painted by the per-container
    /// `show_*`; these fragments carry none and exist only for
    /// hit-testing and selection highlighting.
    ///
    /// Called from the block-line renderers, including the pre/post
    /// spacing renderers whose blank lines can still carry a container's
    /// indentation. A revealed level renders its own column as ordinary
    /// source syntax in place of the `Spacer` and its decoration.
    ///
    /// Resolves the line's columns from its [`deepest_gutter_level`], not
    /// `node`'s ancestors: a spacing line sits in a shallower node than
    /// the content it spaces, so walking up from `node` would miss the
    /// deeper columns. For content lines `node` is already on the line, so
    /// the two agree.
    pub fn show_block_line_prefixes(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, line: (Grapheme, Grapheme),
        content_top_left: Pos2, row_height: f32,
    ) {
        let Some(deepest) = self.deepest_gutter_level(node, line) else {
            return;
        };
        let indent = self.layout.indent;
        let mut levels: Vec<_> = deepest
            .ancestors()
            .filter(|n| self.is_gutter_level(n))
            .collect();
        levels.reverse();
        let base_x = content_top_left.x - indent * levels.len() as f32;

        let line_inflated = self.row_height_inflated_at_line(node, line);
        let line_row_height = line_inflated.unwrap_or(row_height);

        for (k, level) in levels.iter().enumerate() {
            let range = self.line_own_prefix(level, line);
            if range.is_empty() {
                continue;
            }
            let left = base_x + indent * k as f32;

            // Revealed: the cursor is in this level's syntax, so render its
            // prefix bytes as monospace source, sized and baseline-aligned to
            // match the gutter annotation (e.g. an ordered marker). Right-
            // aligned to the column's content edge so the marker sits about
            // where its decoration was; a marker wider than the column
            // overflows left (into the gutter) rather than over the content.
            if self.reveal_gutter_column(level, line) {
                let afs = self.layout.annotation_font_size;
                let result = self.compute_section_layout_new(
                    range,
                    self.width(level),
                    afs,
                    self.text_format_syntax(),
                );
                let baseline_shift = if line_inflated.is_some() {
                    (line_row_height - afs) / 2.0
                } else {
                    (row_height - afs) * 0.8
                };
                // `result.width` is the available width, not the laid-out
                // marker width; measure the rightmost glyph edge instead.
                let marker_width = result
                    .rows
                    .iter()
                    .flat_map(|row| &row.fragments)
                    .fold(0.0_f32, |w, frag| w.max(frag.rect.right()));
                let marker_left = left + indent - marker_width;
                let top = Pos2::new(marker_left, content_top_left.y + baseline_shift);
                let first = self.fragments.len();
                self.show_wrap_layout(ui, top, &result);
                // Cursor reads `rect`: give the small marker the row's full
                // height so an offset inside it shares the row's center y
                // (vertical nav round-trips).
                for frag in &mut self.fragments[first..] {
                    frag.rect.min.y = content_top_left.y;
                    frag.rect.max.y = content_top_left.y + line_row_height;
                }
                continue;
            }

            let rect = Rect::from_min_max(
                Pos2::new(left, content_top_left.y),
                Pos2::new(left + indent, content_top_left.y + line_row_height),
            );
            self.fragments.push(Fragment {
                rect,
                content_inset: FragmentInset::default(),
                source_range: range,
                style_stack: Vec::new(),
                content: FragmentContent::Spacer,
                // Non-atomic: a click places the cursor at the nearer
                // edge. A drag still selects the whole column (an empty-
                // glyph `Spacer` midpoint-snaps to an edge either way, so
                // a drag never lands interior — see [`reveal`]); a double-
                // click selects it via `bounds.words`.
                atomic: false,
                interaction: None,
            });
        }
    }

    /// Makes each gutter column (marker / per-level indentation) a word
    /// for word navigation and double-click. Replaces the default
    /// unicode-segmented words inside each line's prefix region — which
    /// would split a marker like `* ` into `*` + ` ` and drop
    /// indentation — with the per-level [`line_own_prefix`] columns. Call after
    /// `calc_words`, with the parsed `root`.
    pub fn calc_syntax_words(&mut self, root: &'ast AstNode<'ast>) {
        let n = self.bounds.source_lines.len();
        if n == 0 {
            return;
        }
        // One DFS resolves every line's deepest gutter container; a
        // per-line `deepest_container_block_at_offset` lookup would be
        // quadratic, and this runs every reparse.
        let mut deepest_by_line: Vec<Option<&'ast AstNode<'ast>>> = vec![None; n];
        self.assign_deepest_gutter(root, &mut deepest_by_line);

        let mut prefix_regions: Vec<(Grapheme, Grapheme)> = Vec::new();
        let mut columns: Vec<(Grapheme, Grapheme)> = Vec::new();
        for (line_idx, &deepest) in deepest_by_line.iter().enumerate() {
            let Some(deepest) = deepest else {
                continue;
            };
            let line = self.bounds.source_lines[line_idx];
            let (prefix_len, _) = self.line_prefix_len(deepest, line);
            if prefix_len.0 == 0 {
                continue;
            }
            prefix_regions.push((line.start(), line.start() + prefix_len));
            let mut level = Some(deepest);
            while let Some(node) = level {
                let range = self.line_own_prefix(node, line);
                if !range.is_empty() {
                    columns.push(range);
                }
                level = self.gutter_parent(node);
            }
        }
        if columns.is_empty() {
            return;
        }
        // Drop unicode words that fell in a prefix region; the columns
        // replace them. Both sets are non-overlapping and the regions are
        // disjoint from content, so the merged list stays sorted and
        // non-overlapping as `range_bound` requires.
        self.bounds
            .words
            .retain(|w| !prefix_regions.iter().any(|p| w.intersects(p, false)));
        self.bounds.words.extend(columns);
        self.bounds.words.sort();
    }

    /// Returns whether the given line is one of the source lines of the given node
    pub fn node_contains_line(
        &self, node: &'ast AstNode<'ast>, line: (Grapheme, Grapheme),
    ) -> bool {
        let first_line = self.node_first_line(node);
        let last_line = self.node_last_line(node);

        (first_line.start(), last_line.end()).contains_range(&line, true, true)
    }

    /// Returns the row height of a line in the given node, even if that node is
    /// a container block.
    pub fn node_line_row_height(
        &self, node: &'ast AstNode<'ast>, line: (Grapheme, Grapheme),
    ) -> f32 {
        // leaf blocks and inlines
        if node.data.borrow().value.contains_inlines() || !node.data.borrow().value.block() {
            return self.row_height(node);
        }

        // container blocks only
        for child in node.children() {
            if self.node_contains_line(child, line) {
                return self.node_line_row_height(child, line);
            }
        }

        self.row_height(node)
    }

    /// Visual height of `node`'s first content row when an inline image
    /// inflates it; `None` otherwise. Mirrors the wrap-layout row
    /// metric (`max(default_ascent, max_image_height) + default_descent`)
    /// so sibling markers can center on the actual row.
    pub fn first_content_row_height_inflated(&self, node: &'ast AstNode<'ast>) -> Option<f32> {
        let mut leaf = node;
        while !leaf.data.borrow().value.contains_inlines() {
            leaf = leaf.children().next()?;
        }
        self.row_height_inflated_at_line(leaf, self.node_first_line(leaf))
    }

    /// Visual height of `node`'s row at `line` when an inline image inflates
    /// it; `None` otherwise.
    pub fn row_height_inflated_at_line(
        &self, node: &'ast AstNode<'ast>, line: (Grapheme, Grapheme),
    ) -> Option<f32> {
        let mut leaf = node;
        while !leaf.data.borrow().value.contains_inlines() {
            leaf = leaf.children().next()?;
        }
        let leaf_node_line = self.node_line(leaf, line);
        if leaf_node_line.is_empty() || self.disable_images {
            return None;
        }
        let max_image_height = leaf
            .descendants()
            .filter(|d| matches!(d.data.borrow().value, NodeValue::Image(_)))
            .filter(|d| leaf_node_line.contains_range(&self.node_range(d), true, true))
            // Same collapse predicate as `layout_image`, else a selected image
            // stays tall but the row doesn't inflate, shifting the marker.
            .filter(|d| !self.range_revealed_interior(self.node_range(d)))
            .filter_map(|d| self.image_logical_size(d).map(|s| s.y))
            .fold(0.0_f32, f32::max);
        let leaf_row_height = self.row_height(leaf);
        let leaf_ascent = leaf_row_height * 0.8;
        let leaf_descent = leaf_row_height * 0.2;
        (max_image_height > leaf_ascent).then_some(max_image_height + leaf_descent)
    }

    // compute bounds for blocks stacked vertically
    pub fn compute_bounds_block_children(&mut self, node: &'ast AstNode<'ast>) {
        for child in node.children() {
            // add pre-spacing bounds
            self.compute_bounds_block_pre_spacing(child);

            // add block bounds
            self.compute_bounds(child);

            // add post-spacing bounds
            self.compute_bounds_block_post_spacing(child);
        }
    }
}
