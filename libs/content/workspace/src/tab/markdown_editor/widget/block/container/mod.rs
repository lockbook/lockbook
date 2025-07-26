use comrak::nodes::{AstNode, NodeValue};
use egui::{Pos2, Rect, Ui, Vec2};
use lb_rs::model::text::offset_types::{
    DocCharOffset, IntoRangeExt, RangeExt as _, RangeIterExt as _, RelCharOffset,
};

use crate::tab::markdown_editor::Editor;
use crate::tab::markdown_editor::widget::INDENT;

pub(crate) mod alert;
pub(crate) mod block_quote;
pub(crate) mod document;
pub(crate) mod footnote_definition;
pub(crate) mod item;
pub(crate) mod list;
pub(crate) mod table;
pub(crate) mod table_row;
pub(crate) mod task_item;

impl<'ast> Editor {
    pub fn indent(&self, node: &'ast AstNode<'ast>) -> f32 {
        let value = &node.data.borrow().value;
        let sp = &node.data.borrow().sourcepos;
        match value {
            NodeValue::FrontMatter(_) => 0.,
            NodeValue::Raw(_) => unreachable!("can only be created programmatically"),

            // container_block
            NodeValue::Alert(_) => INDENT,
            NodeValue::BlockQuote => INDENT,
            NodeValue::DescriptionItem(_) => unimplemented!("extension disabled"),
            NodeValue::DescriptionList => unimplemented!("extension disabled"),
            NodeValue::Document => 0.,
            NodeValue::FootnoteDefinition(_) => INDENT,
            NodeValue::Item(_) => INDENT,
            NodeValue::List(_) => 0., // indentation handled by items
            NodeValue::MultilineBlockQuote(_) => unimplemented!("extension disabled"),
            NodeValue::Table(_) => 0.,
            NodeValue::TableRow(_) => 0.,
            NodeValue::TaskItem(_) => INDENT,

            // inline
            NodeValue::Image(_) => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::Code(_) => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::Emph => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::Escaped => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::EscapedTag(_) => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::FootnoteReference(_) => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::HtmlInline(_) => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::LineBreak => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::Link(_) => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::Math(_) => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::SoftBreak => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::SpoileredText => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::Strikethrough => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::Strong => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::Subscript => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::Superscript => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::Text(_) => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::Underline => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::WikiLink(_) => unimplemented!("not a block: {} {:?}", sp, value),

            // leaf_block
            NodeValue::CodeBlock(_) => unimplemented!("not a container block: {} {:?}", sp, value),
            NodeValue::DescriptionDetails => unimplemented!("extension disabled"),
            NodeValue::DescriptionTerm => unimplemented!("extension disabled"),
            NodeValue::Heading(_) => unimplemented!("not a container block: {} {:?}", sp, value),
            NodeValue::HtmlBlock(_) => unimplemented!("not a container block: {} {:?}", sp, value),
            NodeValue::Paragraph => unimplemented!("not a container block: {} {:?}", sp, value),
            NodeValue::TableCell => unimplemented!("not a container block: {} {:?}", sp, value),
            NodeValue::ThematicBreak => unimplemented!("not a container block: {} {:?}", sp, value),
        }
    }

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

    // blocks are stacked vertically
    pub fn show_block_children(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, mut top_left: Pos2,
    ) {
        let mut children: Vec<_> = node.children().collect();
        children.sort_by_key(|child| child.data.borrow().sourcepos);
        for child in children {
            // add pre-spacing
            let pre_spacing = self.block_pre_spacing_height(child);
            self.show_block_pre_spacing(ui, child, top_left);
            top_left.y += pre_spacing;

            // add block
            let child_height = self.height(child);

            if self.debug {
                let child_width = self.width(child);
                let child_rect =
                    Rect::from_min_size(top_left, Vec2 { x: child_width, y: child_height });

                if self.selected_block(child) {
                    ui.painter().rect(
                        child_rect,
                        2.,
                        self.theme.bg().neutral_secondary,
                        egui::Stroke { width: 1., color: self.theme.bg().neutral_tertiary },
                    );
                }
            }

            self.show_block(ui, child, top_left);
            top_left.y += child_height;

            // add post-spacing
            let post_spacing = self.block_post_spacing_height(child);
            self.show_block_post_spacing(ui, child, top_left);
            top_left.y += post_spacing;
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
        &self, node: &'ast AstNode<'ast>, line: (DocCharOffset, DocCharOffset),
    ) -> (RelCharOffset, bool) {
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
            NodeValue::Image(_) => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::Code(_) => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::Emph => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::Escaped => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::EscapedTag(_) => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::FootnoteReference(_) => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::HtmlInline(_) => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::LineBreak => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::Link(_) => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::Math(_) => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::SoftBreak => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::SpoileredText => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::Strikethrough => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::Strong => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::Subscript => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::Superscript => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::Text(_) => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::Underline => unimplemented!("not a block: {} {:?}", sp, value),
            NodeValue::WikiLink(_) => unimplemented!("not a block: {} {:?}", sp, value),

            // leaf_block
            NodeValue::CodeBlock(_) => unimplemented!("not a container block: {} {:?}", sp, value),
            NodeValue::DescriptionDetails => unimplemented!("extension disabled"),
            NodeValue::DescriptionTerm => unimplemented!("extension disabled"),
            NodeValue::Heading(_) => unimplemented!("not a container block: {} {:?}", sp, value),
            NodeValue::HtmlBlock(_) => unimplemented!("not a container block: {} {:?}", sp, value),
            NodeValue::Paragraph => unimplemented!("not a container block: {} {:?}", sp, value),
            NodeValue::TableCell => unimplemented!("not a container block: {} {:?}", sp, value),
            NodeValue::ThematicBreak => unimplemented!("not a container block: {} {:?}", sp, value),
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
        &self, node: &'ast AstNode<'ast>, line: (DocCharOffset, DocCharOffset),
    ) -> (DocCharOffset, DocCharOffset) {
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
        &self, node: &'ast AstNode<'ast>, line: (DocCharOffset, DocCharOffset),
    ) -> (DocCharOffset, DocCharOffset) {
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
        &self, node: &'ast AstNode<'ast>, line: (DocCharOffset, DocCharOffset),
    ) -> (DocCharOffset, DocCharOffset) {
        let (prefix_len, _) = self.line_prefix_len(node, line);
        (line.start() + prefix_len, line.end())
    }

    /// Returns the range representing the portion of the line before the
    /// [`node_content`]. Equivalent to [`line_ancestors_prefix`] +
    /// [`line_own_prefix`]. Always has length == [`line_prefix_len`].
    pub fn line_prefix(
        &self, node: &'ast AstNode<'ast>, line: (DocCharOffset, DocCharOffset),
    ) -> (DocCharOffset, DocCharOffset) {
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
            NodeValue::TaskItem(_) => " ".repeat(own_prefix.len().0),

            // inline
            NodeValue::Image(_)
            | NodeValue::Code(_)
            | NodeValue::Emph
            | NodeValue::Escaped
            | NodeValue::EscapedTag(_)
            | NodeValue::FootnoteReference(_)
            | NodeValue::HtmlInline(_)
            | NodeValue::LineBreak
            | NodeValue::Link(_)
            | NodeValue::Math(_)
            | NodeValue::SoftBreak
            | NodeValue::SpoileredText
            | NodeValue::Strikethrough
            | NodeValue::Strong
            | NodeValue::Subscript
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
        let mut result = if let Some(parent) = prior_node.parent() {
            self.extension_prefix(parent)?
        } else {
            Default::default()
        };

        let line = self.node_first_line(prior_node);
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
            NodeValue::TaskItem(_) => result += &self.buffer[own_prefix],

            // inline
            NodeValue::Image(_)
            | NodeValue::Code(_)
            | NodeValue::Emph
            | NodeValue::Escaped
            | NodeValue::EscapedTag(_)
            | NodeValue::FootnoteReference(_)
            | NodeValue::HtmlInline(_)
            | NodeValue::LineBreak
            | NodeValue::Link(_)
            | NodeValue::Math(_)
            | NodeValue::SoftBreak
            | NodeValue::SpoileredText
            | NodeValue::Strikethrough
            | NodeValue::Strong
            | NodeValue::Subscript
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

    /// returns true if the syntax for a container block should be revealed
    pub fn reveal(&self, node: &'ast AstNode<'ast>) -> bool {
        for line in self.node_lines(node).iter() {
            let line = self.bounds.source_lines[line];

            let line_prefix = self.line_prefix(node, line);
            if line_prefix.is_empty() {
                continue;
            }

            let selection = self.buffer.current.selection;
            if line_prefix.contains(selection.start(), true, false) {
                return true;
            }
            if line_prefix.contains(selection.end(), true, false) {
                return true;
            }

            if self.buffer[self.node_line(node, line)]
                .chars()
                .all(|c| c.is_whitespace())
                && selection.start() == line_prefix.end()
            {
                // line prefix and contents contain only whitespace: end of line
                // prefix is inclusive
                // this improves the editing experience for list items
                return true;
            }
        }
        false
    }

    /// Returns whether the given line is one of the source lines of the given node
    pub fn node_contains_line(
        &self, node: &'ast AstNode<'ast>, line: (DocCharOffset, DocCharOffset),
    ) -> bool {
        let first_line = self.node_first_line(node);
        let last_line = self.node_last_line(node);

        (first_line.start(), last_line.end()).contains_range(&line, true, true)
    }

    /// Returns the row height of a line in the given node, even if that node is
    /// a container block.
    pub fn node_line_row_height(
        &self, node: &'ast AstNode<'ast>, line: (DocCharOffset, DocCharOffset),
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

    pub fn compute_bounds_list(&mut self, node: &'ast AstNode<'ast>) {
        self.compute_bounds_block_children(node);
    }

    // compute bounds for blocks stacked vertically
    pub fn compute_bounds_block_children(&mut self, node: &'ast AstNode<'ast>) {
        let mut children: Vec<_> = node.children().collect();
        children.sort_by_key(|child| child.data.borrow().sourcepos);
        for child in children {
            // add pre-spacing bounds
            self.compute_bounds_block_pre_spacing(child);

            // add block bounds
            self.compute_bounds(child);

            // add post-spacing bounds
            self.compute_bounds_block_post_spacing(child);
        }
    }
}
