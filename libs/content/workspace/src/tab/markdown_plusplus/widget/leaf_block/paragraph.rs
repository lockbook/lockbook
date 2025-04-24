use comrak::nodes::{AstNode, NodeLink, NodeValue};
use egui::{Pos2, Ui, Vec2};
use lb_rs::model::text::offset_types::{DocCharOffset, IntoRangeExt, RangeExt, RangeIterExt as _};

use crate::tab::markdown_plusplus::{
    widget::{Wrap, BLOCK_SPACING, ROW_HEIGHT},
    MarkdownPlusPlus,
};

impl<'ast> MarkdownPlusPlus {
    pub fn height_paragraph(&self, node: &'ast AstNode<'ast>) -> f32 {
        let mut result = 0.;
        for descendant in node.descendants() {
            if let NodeValue::Image(NodeLink { url, .. }) = &descendant.data.borrow().value {
                result += self.height_image(node, url);
                result += BLOCK_SPACING;
            }
        }

        let last_line_idx = self.node_lines(node).iter().count() - 1;
        for (line_idx, line) in self.node_lines(node).iter().enumerate() {
            let line = self.bounds.source_lines[line];
            let mut shown_as_postfix = false;

            // see the corresponding show fn for an explanation
            for ancestor in node
                .ancestors()
                .skip(1)
                .collect::<Vec<_>>()
                .iter()
                .rev()
                .skip(1)
            {
                let parent = ancestor.parent().unwrap();
                if self.reveal(ancestor) {
                    let height = self.height_line_postfix(parent, line, ROW_HEIGHT);

                    result += height;
                    shown_as_postfix = true;
                    break;
                }
            }
            if !shown_as_postfix {
                result += self.height_paragraph_line(node, line);
            }

            if line_idx != last_line_idx {
                result += BLOCK_SPACING;
            }
        }

        result
    }

    pub fn height_paragraph_line(
        &self, node: &'ast AstNode<'ast>, line: (DocCharOffset, DocCharOffset),
    ) -> f32 {
        let width = self.width(node);
        let mut wrap = Wrap::new(width);

        let parent = node.parent().unwrap();
        let parent_prefix_len = self.line_prefix_len(parent, line);
        let node_line = (line.start() + parent_prefix_len, line.end());

        let line_children = self.children_in_line(node, line);

        if let Some(first_child) = line_children.first() {
            if node_line.intersects(&self.buffer.current.selection, true) {
                let prefix_range = (node_line.start(), self.node_range(first_child).start());
                wrap.offset +=
                    self.span_text_line(&wrap, prefix_range, self.text_format_syntax(node));
            }
        }
        for child in &line_children {
            wrap.offset += self.span(child, &wrap);
        }
        if let Some(last_child) = line_children.last() {
            if node_line.intersects(&self.buffer.current.selection, true) {
                let postfix_range = (self.node_range(last_child).end(), node_line.end());
                wrap.offset +=
                    self.span_text_line(&wrap, postfix_range, self.text_format_syntax(node));
            }
        }

        wrap.height()
    }

    pub fn show_paragraph(&mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, mut top_left: Pos2) {
        for descendant in node.descendants() {
            if let NodeValue::Image(NodeLink { url, .. }) = &descendant.data.borrow().value {
                self.show_image_block(ui, node, top_left, url);
                top_left.y += self.height_image(node, url);
                top_left.y += BLOCK_SPACING;
            }
        }

        for line in self.node_lines(node).iter() {
            let line = self.bounds.source_lines[line];
            let mut shown_as_postfix = false;

            // this code is best understood in terms of an example:
            // line:           "·*·_·>·_·p·a·r·a·g·r·a·p·h·"
            // item prefix:     |<->|
            // item postfix:        |<------------------->|
            // quote prefix:        |<->|
            // quote postfix:           |<--------------->|
            // ...and let's suppose we're revealing the quote but not the item

            // iterate ancestors outermost-to-innermost, skipping the
            // (parent-less) document and this block
            for ancestor in node
                .ancestors()
                .skip(1)
                .collect::<Vec<_>>()
                .iter()
                .rev()
                .skip(1)
            {
                let ancestor_parent = ancestor.parent().unwrap();

                let node_width = self.width(node);
                let ancestor_width = self.width(ancestor);
                let relative_indent = ancestor_width - node_width;
                let parent_postfix_top_left = top_left - Vec2::X * relative_indent;

                if self.reveal(ancestor) {
                    // we found the outermost ancestor that's revealed (in our
                    // example, the quote) so we want to show its ancestors
                    // prefixes plus one text line for its prefix and postfix
                    let node_parent_prefix_len = self.line_prefix_len(node.parent().unwrap(), line);
                    let ancestor_parent_prefix_len = self.line_prefix_len(ancestor_parent, line);

                    let ancestor_parent_prefix =
                        (line.start(), line.start() + ancestor_parent_prefix_len);
                    let shown_prefix =
                        (ancestor_parent_prefix.end(), line.start() + node_parent_prefix_len);

                    // we need to compute the height of the shown line for the
                    // rendering of the ancestors' prefixes. in our example
                    // that's an item, but if it was a quote, it should show
                    // with the height of the shown line (accounting for line
                    // wrap) so there isn't a weird gap in the quote marker. we
                    // pass the parent because we're including our own prefix
                    // i.e. we're showing the parent's postfix.
                    let height = self.height_line_postfix(ancestor_parent, line, ROW_HEIGHT);

                    // show the parent's prefix with the computed height. this
                    // must be done before showing its postfix because text must
                    // be shown in source order. this constraint contributes
                    // materially to the complexity of this implementation e.g.
                    // it's the reason we can't just return the height from the
                    // fn that shows the postfix and instead need to compute the
                    // height ahead of time.

                    self.show_line_prefix(
                        ui,
                        ancestor_parent,
                        line,
                        parent_postfix_top_left,
                        height,
                        ROW_HEIGHT,
                    );

                    let width = self.width(ancestor_parent);
                    let mut wrap = Wrap::new(width);
                    self.show_text_line(
                        ui,
                        parent_postfix_top_left,
                        &mut wrap,
                        shown_prefix,
                        self.text_format_syntax(ancestor_parent),
                        false,
                    );
                    self.bounds.paragraphs.push(shown_prefix);
                    self.show_paragraph_line(ui, node, parent_postfix_top_left, line, &mut wrap);

                    // if the paragraph line is shown in this way:
                    // * it's height is determined by the height of the shown line
                    top_left.y += height;

                    // * it need not be shown the normal way
                    shown_as_postfix = true;

                    // * it need not be shown by any inner ancestor
                    break;
                }
            }
            if !shown_as_postfix {
                let width = self.width(node);
                let mut wrap = Wrap::new(width);

                let parent = node.parent().unwrap();
                let line_height = self.height_paragraph_line(node, line);
                self.show_line_prefix(ui, parent, line, top_left, line_height, wrap.row_height);

                self.show_paragraph_line(ui, node, top_left, line, &mut wrap);
                top_left.y += line_height;
            }
            top_left.y += BLOCK_SPACING;
        }
    }

    pub fn show_paragraph_line(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2,
        line: (DocCharOffset, DocCharOffset), wrap: &mut Wrap,
    ) {
        let parent = node.parent().unwrap();
        let parent_prefix_len = self.line_prefix_len(parent, line);
        let node_line = (line.start() + parent_prefix_len, line.end());

        // "The paragraph's raw content is formed by concatenating the lines
        // and removing initial and final whitespace"
        if let Some((leading_whitespace, _, children, postfix_whitespace, _)) =
            self.line_ranges(node, line)
        {
            if !leading_whitespace.is_empty() {
                self.bounds.paragraphs.push(leading_whitespace);
            }
            self.bounds.paragraphs.push(children);
            if !postfix_whitespace.is_empty() {
                self.bounds.paragraphs.push(postfix_whitespace);
            }

            if node_line.intersects(&self.buffer.current.selection, true) {
                self.show_text_line(
                    ui,
                    top_left,
                    wrap,
                    leading_whitespace,
                    self.text_format_syntax(node),
                    false,
                );
            }
            for child in &self.children_in_line(node, line) {
                self.show_inline(ui, child, top_left, wrap);
            }
            if node_line.intersects(&self.buffer.current.selection, true) {
                self.show_text_line(
                    ui,
                    top_left,
                    wrap,
                    postfix_whitespace,
                    self.text_format_syntax(node),
                    false,
                );
            }
        } else {
            self.bounds.paragraphs.push(node_line.start().into_range());
        }
    }
}
