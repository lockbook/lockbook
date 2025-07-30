use crate::tab::markdown_editor::Editor;
use crate::tab::markdown_editor::bounds::{BoundExt as _, RangesExt as _};
use crate::tab::markdown_editor::galleys::Galleys;
use crate::tab::markdown_editor::input::Event;
use crate::tab::markdown_editor::style::{
    BlockNode, BlockNodeType, InlineNode, InlineNodeType, ListItem, MarkdownNode,
};
use crate::tab::markdown_editor::widget::ROW_SPACING;
use crate::tab::markdown_editor::widget::utils::NodeValueExt as _;
use comrak::nodes::{AstNode, NodeHeading, NodeValue};
use egui::{Pos2, Rangef, Vec2};
use lb_rs::model::text::buffer::{self};
use lb_rs::model::text::offset_types::{
    DocCharOffset, IntoRangeExt, RangeExt as _, RangeIterExt, RelByteOffset, RelCharOffset,
    ToRangeExt as _,
};
use lb_rs::model::text::operation_types::{Operation, Replace};
use pulldown_cmark::HeadingLevel;

use super::advance::AdvanceExt as _;
use super::{Bound, Location, Offset, Region};

/// tracks editor state necessary to support translating input events to buffer operations
#[derive(Default)]
pub struct EventState {
    pub internal_events: Vec<Event>,
}

impl<'ast> Editor {
    /// Translates editor events into buffer operations by interpreting them in the context of the current editor state.
    /// Dispatches events that aren't buffer operations. Returns a (text_updated, selection_updated) pair.
    pub fn calc_operations(
        &mut self, ctx: &egui::Context, root: &'ast AstNode<'ast>, event: Event,
        operations: &mut Vec<Operation>,
    ) -> buffer::Response {
        let current_selection = self.buffer.current.selection;
        let mut response = buffer::Response::default();
        match event {
            Event::Select { region } => {
                operations.push(Operation::Select(self.region_to_range(region)));
            }
            Event::Replace { region, text } => {
                let range = self.region_to_range(region);
                operations.push(Operation::Replace(Replace { range, text }));
                operations.push(Operation::Select(range.start().to_range()));
            }
            Event::ToggleStyle { region, style } => {
                let range = self.region_to_range(region);

                match style {
                    MarkdownNode::Document | MarkdownNode::Paragraph => {}
                    MarkdownNode::Inline(inline_style) => {
                        let unapply = self.unapply_inline(root, range, &inline_style);

                        for inline_paragraph in &self.bounds.inline_paragraphs {
                            if inline_paragraph.intersects(&range, true) {
                                let paragraph_range = (
                                    range.start().max(inline_paragraph.start()),
                                    range.end().min(inline_paragraph.end()),
                                );

                                self.apply_inline_style(
                                    root,
                                    paragraph_range,
                                    inline_style.clone(),
                                    unapply,
                                    operations,
                                );
                            }
                        }

                        // todo: advance cursor
                    }
                    MarkdownNode::Block(block_style) => {
                        let unapply = self.unapply_block(root, &block_style);

                        let mut handled = false;
                        for node in root.descendants() {
                            if self.selected_block(node) {
                                handled = true;

                                // apply heading to ATX heading: replace existing heading
                                if let BlockNodeType::Heading(style_level) = block_style.node_type()
                                {
                                    if let NodeValue::Heading(NodeHeading {
                                        level: node_level,
                                        setext: false,
                                    }) = node.data.borrow().value
                                    {
                                        for line_idx in self.node_lines(node).iter() {
                                            let line = self.bounds.source_lines[line_idx];
                                            let node_line = self.node_line(node, line);

                                            let style_level = match style_level {
                                                HeadingLevel::H1 => 1,
                                                HeadingLevel::H2 => 2,
                                                HeadingLevel::H3 => 3,
                                                HeadingLevel::H4 => 4,
                                                HeadingLevel::H5 => 5,
                                                HeadingLevel::H6 => 6,
                                            };
                                            if style_level > node_level {
                                                let add_levels = style_level - node_level;
                                                operations.push(Operation::Replace(Replace {
                                                    range: node_line.start().into_range(),
                                                    text: "#".repeat(add_levels as _),
                                                }));
                                            } else if style_level == node_level {
                                                // remove heading
                                                let mut range = (
                                                    node_line.start(),
                                                    node_line.start()
                                                        + RelCharOffset(node_level as _),
                                                );
                                                if self.buffer.current.segs.last_cursor_position()
                                                    > range.end()
                                                    && &self.buffer[(range.end(), range.end() + 1)]
                                                        == " "
                                                {
                                                    range.1 += 1;
                                                }

                                                operations.push(Operation::Replace(Replace {
                                                    range,
                                                    text: "".into(),
                                                }));
                                            } else {
                                                let remove_levels = node_level - style_level;
                                                operations.push(Operation::Replace(Replace {
                                                    range: (
                                                        node_line.start(),
                                                        node_line.start()
                                                            + RelCharOffset(remove_levels as _),
                                                    ),
                                                    text: "".into(),
                                                }));
                                            }
                                        }
                                    } else if NodeValue::Paragraph == node.data.borrow().value {
                                        for line_idx in self.node_lines(node).iter() {
                                            let line = self.bounds.source_lines[line_idx];
                                            let node_line = self.node_line(node, line);

                                            // count paragraph soft breaks as node breaks
                                            if node.data.borrow().value == NodeValue::Paragraph
                                                && !line.intersects(
                                                    &self.buffer.current.selection,
                                                    true,
                                                )
                                            {
                                                continue;
                                            }

                                            operations.push(Operation::Replace(Replace {
                                                range: node_line.start().into_range(),
                                                text: "#".repeat(style_level as _) + " ",
                                            }));
                                        }
                                    }
                                } else {
                                    // remove target prefix regardless (will often be empty / supports replacements)
                                    // todo: space between selected nodes?
                                    let target_node = if node.is_container_block() {
                                        node
                                    } else {
                                        node.parent().unwrap()
                                    };
                                    for line_idx in self.node_lines(node).iter() {
                                        let line = self.bounds.source_lines[line_idx];

                                        let prefix = self.line_own_prefix(target_node, line);

                                        operations.push(Operation::Replace(Replace {
                                            range: prefix,
                                            text: "".into(),
                                        }));
                                    }

                                    if !unapply {
                                        let mut first_line = true;
                                        for line_idx in self.node_lines(node).iter() {
                                            let line = self.bounds.source_lines[line_idx];

                                            // count paragraph soft breaks as node breaks
                                            if node.data.borrow().value == NodeValue::Paragraph
                                                && !line.intersects(
                                                    &self.buffer.current.selection,
                                                    true,
                                                )
                                            {
                                                continue;
                                            }

                                            let range = self
                                                .line_ancestors_prefix(node, line)
                                                .end()
                                                .into_range();
                                            let text = match block_style {
                                                BlockNode::Heading(_) => unreachable!(),
                                                BlockNode::Quote => "> ",
                                                BlockNode::Code(_) => unimplemented!(), // todo: support inserting lines
                                                BlockNode::ListItem(ListItem::Bulleted, _) => {
                                                    if first_line {
                                                        "* "
                                                    } else {
                                                        "  "
                                                    }
                                                }
                                                BlockNode::ListItem(ListItem::Numbered(_), _) => {
                                                    if first_line {
                                                        "1. "
                                                    } else {
                                                        "   "
                                                    }
                                                }
                                                BlockNode::ListItem(ListItem::Todo(true), _) => {
                                                    if first_line {
                                                        "* [x] "
                                                    } else {
                                                        "  "
                                                    }
                                                }
                                                BlockNode::ListItem(ListItem::Todo(false), _) => {
                                                    if first_line {
                                                        "* [ ] "
                                                    } else {
                                                        "  "
                                                    }
                                                }
                                                BlockNode::Rule => unimplemented!(), // todo: kind of just not a priority rn
                                            }
                                            .into();

                                            operations
                                                .push(Operation::Replace(Replace { range, text }));

                                            // count paragraph soft breaks as node breaks
                                            if node.data.borrow().value != NodeValue::Paragraph {
                                                first_line = false;
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        if !handled {
                            // selecting sequence of contiguous empty/whitespace-only lines:
                            // insert or remove matching prefix
                            if !unapply {
                                let range = current_selection.start().into_range();
                                let text = match block_style {
                                    BlockNode::Heading(heading_level) => {
                                        // todo: technically this makes a bunch of separate headings
                                        match heading_level {
                                            HeadingLevel::H1 => "# ",
                                            HeadingLevel::H2 => "## ",
                                            HeadingLevel::H3 => "### ",
                                            HeadingLevel::H4 => "#### ",
                                            HeadingLevel::H5 => "##### ",
                                            HeadingLevel::H6 => "###### ",
                                        }
                                    }
                                    BlockNode::Quote => "> ",
                                    BlockNode::Code(_) => unimplemented!(), // todo: support inserting lines
                                    BlockNode::ListItem(ListItem::Bulleted, _) => "* ",
                                    BlockNode::ListItem(ListItem::Numbered(_), _) => "1. ",
                                    BlockNode::ListItem(ListItem::Todo(true), _) => "* [x] ",
                                    BlockNode::ListItem(ListItem::Todo(false), _) => "* [ ] ",
                                    BlockNode::Rule => unimplemented!(), // todo: kind of just not a priority rn
                                }
                                .into();

                                operations.push(Operation::Replace(Replace { range, text }));
                                operations.push(Operation::Select(current_selection));
                            } else {
                                let target_node = self.deepest_container_block_at_offset(
                                    root,
                                    self.buffer.current.selection.start(),
                                );
                                if block_style
                                    .node_type()
                                    .matches(&target_node.data.borrow().value)
                                {
                                    let line_idx = self
                                        .range_lines(current_selection.start().into_range())
                                        .start();
                                    let line = self.bounds.source_lines[line_idx];

                                    let prefix = self.line_own_prefix(target_node, line);

                                    operations.push(Operation::Replace(Replace {
                                        range: prefix,
                                        text: "".into(),
                                    }));
                                }
                            }
                        }
                    }
                }
            }
            Event::Newline { shift } => {
                // insert/extend/terminate container blocks
                let mut handled = || {
                    // selection must be empty
                    let Some(offset) = self.selection_offset() else {
                        return false;
                    };

                    let container = self.deepest_container_block_at_offset(root, offset);
                    let line = self.line_at_offset(offset);
                    let line_content = self.line_content(container, line);
                    let own_prefix = self.line_own_prefix(container, line);

                    let in_code_block = matches!(
                        self.leaf_block_at_offset(root, offset).data.borrow().value,
                        NodeValue::CodeBlock(_)
                    );

                    if shift || in_code_block {
                        // shift -> extend
                        operations.push(Operation::Replace(Replace {
                            range: current_selection,
                            text: "\n".into(),
                        }));
                        if let Some(extension_prefix) = self.extension_prefix(container) {
                            operations.push(Operation::Replace(Replace {
                                range: current_selection,
                                text: extension_prefix,
                            }));
                        };
                    } else if line_content.is_empty() {
                        // empty container block -> terminate

                        // operation must do something
                        if own_prefix.is_empty() {
                            return false;
                        }

                        operations.push(Operation::Replace(Replace {
                            range: own_prefix,
                            text: "".into(),
                        }));
                    } else {
                        // nonempty container block -> insert
                        operations.push(Operation::Replace(Replace {
                            range: current_selection,
                            text: "\n".into(),
                        }));
                        if let Some(insertion_prefix) = self.insertion_prefix(container) {
                            operations.push(Operation::Replace(Replace {
                                range: current_selection,
                                text: insertion_prefix,
                            }));
                        };
                    }

                    // code block auto-indentation
                    if in_code_block {
                        let line_content_start = self.offset_to_byte(line_content.start());
                        let indentation_len = RelByteOffset(
                            self.buffer[line_content].len()
                                - self.buffer[line_content].trim_start().len(),
                        );
                        let indentation =
                            (line_content_start, line_content_start + indentation_len);
                        let indentation = self.range_to_char(indentation);

                        operations.push(Operation::Replace(Replace {
                            range: current_selection,
                            text: self.buffer[indentation].to_string(),
                        }));
                    }

                    true
                };
                if !handled() {
                    // default -> insert newline
                    operations.push(Operation::Replace(Replace {
                        range: current_selection,
                        text: "\n".into(),
                    }));
                }

                // advance cursor
                operations.push(Operation::Select(current_selection.start().to_range()));
            }
            Event::Delete { region } => {
                // delete container block prefix
                let mut handled = || {
                    // must be mostly vanilla backspace
                    if !matches!(
                        region,
                        Region::SelectionOrOffset {
                            offset: Offset::Next(Bound::Char | Bound::Word),
                            backwards: true,
                        }
                    ) {
                        return false;
                    }

                    // selection must be empty
                    let Some(offset) = self.selection_offset() else {
                        return false;
                    };

                    let container = self.deepest_container_block_at_offset(root, offset);
                    let line = self.line_at_offset(offset);
                    let own_prefix = self.line_own_prefix(container, line);
                    let content = self.line_content(container, line);

                    // selection must be at content start
                    if offset != content.start() {
                        return false;
                    }

                    // operation must do something
                    if own_prefix.is_empty() {
                        return false;
                    }

                    operations
                        .push(Operation::Replace(Replace { range: own_prefix, text: "".into() }));

                    true
                };
                if !handled() {
                    // default -> delete region
                    let range = self.region_to_range(region);
                    operations.push(Operation::Replace(Replace { range, text: "".into() }));
                    operations.push(Operation::Select(range.start().to_range()));
                }

                // advance cursor
                operations.push(Operation::Select(current_selection.start().to_range()));
            }
            Event::Indent { deindent } => {
                let selected_lines = self
                    .bounds
                    .source_lines
                    .find_intersecting(current_selection, true);
                let first_selected_line_idx = selected_lines.0;
                let first_selected_line = self.bounds.source_lines[first_selected_line_idx];

                if !deindent {
                    // indent into extension of block on prior line
                    let mut handled = || {
                        // must not be first line
                        if first_selected_line_idx == 0 {
                            return false;
                        }

                        let prior_line_idx = first_selected_line_idx - 1;
                        let prior_line = self.bounds.source_lines[prior_line_idx];
                        let prior_line_deepest_container =
                            self.deepest_container_block_at_offset(root, prior_line.end());

                        // among blocks on prior line, find the least deep that
                        // has a prefix on the prior line but not on the first
                        // selected line. this is the container that the
                        // selected lines will be tab-indented into. this rule
                        // accounts for empty-prefix nodes like lists and
                        // prefix-less situations like paragraph continuation
                        // text.
                        let mut prior_line_container_extension_prefix = None;
                        for prior_line_container in prior_line_deepest_container.ancestors() {
                            let has_prefix_on_prior_line = !self
                                .line_own_prefix(prior_line_container, prior_line)
                                .is_empty();
                            let has_prefix_on_first_selected_line = if self
                                .node_last_line_idx(prior_line_container)
                                < first_selected_line_idx
                            {
                                false
                            } else {
                                !self
                                    .line_own_prefix(prior_line_container, first_selected_line)
                                    .is_empty()
                            };

                            if has_prefix_on_prior_line && !has_prefix_on_first_selected_line {
                                if let Some(extension_prefix) =
                                    self.extension_own_prefix(prior_line_container)
                                {
                                    prior_line_container_extension_prefix = Some(extension_prefix);
                                }
                            }
                        }
                        let Some(prior_line_container_extension_prefix) =
                            prior_line_container_extension_prefix
                        else {
                            return false;
                        };

                        // prepend container prefix to each line
                        // todo: only prepend to lines which do not already have
                        // the prefix; this would improve behavior when lazy
                        // continuation lines are mixed with
                        // non-lazy-continuation lines
                        // todo: more attention to multi-line indentation
                        for line_idx in selected_lines.iter() {
                            let line = self.bounds.source_lines[line_idx];
                            let container =
                                self.deepest_container_block_at_offset(root, line.end());
                            let container_own_prefix = self.line_own_prefix(container, line);

                            let insertion_offset =
                                if Some(self.buffer[container_own_prefix].to_string())
                                    == self.extension_own_prefix(container)
                                {
                                    // on what could be a subsequent line of a
                                    // container block, tab to indent the line
                                    // contents; this is the experience when
                                    // e.g. tab-indenting the cursor into a
                                    // preceding container block
                                    container_own_prefix.end()
                                } else {
                                    // on what can only be the first line of a
                                    // container block, tab to indent the block;
                                    // this is the experience when e.g.
                                    // tab-indenting a list item into the list
                                    // item above
                                    container_own_prefix.start()
                                };

                            operations.push(Operation::Replace(Replace {
                                range: insertion_offset.into_range(),
                                text: prior_line_container_extension_prefix.clone(),
                            }));
                        }

                        true
                    };
                    if !handled() {
                        // default -> do nothing
                    }
                } else {
                    // de-indent out of current container block
                    let mut handled = || {
                        // all lines must have container ancestor prefix
                        for line_idx in selected_lines.iter() {
                            let line = self.bounds.source_lines[line_idx];
                            let container =
                                self.deepest_container_block_at_offset(root, line.end());
                            let container_own_prefix = self.line_own_prefix(container, line);

                            // on what can only be the first line of a container
                            // block, shift-tab to de-indent the block rather
                            // than its contents
                            let skip_container =
                                Some(self.buffer[container_own_prefix].to_string())
                                    != self.extension_own_prefix(container);

                            let mut found_container_ancestor = false;
                            for ancestor in container.ancestors() {
                                if container.same_node(ancestor) && skip_container {
                                    continue;
                                }

                                let ancestor_own_prefix = self.line_own_prefix(ancestor, line);
                                if !ancestor_own_prefix.is_empty() {
                                    found_container_ancestor = true;
                                }
                            }
                            if !found_container_ancestor {
                                return false;
                            }
                        }

                        // remove container ancestor prefix from each line
                        for line_idx in selected_lines.iter() {
                            let line = self.bounds.source_lines[line_idx];
                            let container =
                                self.deepest_container_block_at_offset(root, line.end());
                            let container_own_prefix = self.line_own_prefix(container, line);

                            // on what can only be the first line of a container
                            // block, shift-tab to de-indent the block rather
                            // than its contents
                            let skip_container =
                                Some(self.buffer[container_own_prefix].to_string())
                                    != self.extension_own_prefix(container);

                            for ancestor in container.ancestors() {
                                if container.same_node(ancestor) && skip_container {
                                    continue;
                                }

                                let ancestor_own_prefix = self.line_own_prefix(ancestor, line);
                                if !ancestor_own_prefix.is_empty() {
                                    operations.push(Operation::Replace(Replace {
                                        range: ancestor_own_prefix,
                                        text: "".into(),
                                    }));
                                    break;
                                }
                            }
                        }

                        true
                    };
                    if !handled() {
                        // default -> do nothing
                    }
                }

                // advance cursor
                operations.push(Operation::Select(current_selection));
            }
            Event::Find { term, backwards } => {
                if let Some(result) = self.find(term, backwards) {
                    operations.push(Operation::Select(result));
                }
            }
            Event::Undo => {
                response |= self.buffer.undo();
            }
            Event::Redo => {
                response |= self.buffer.redo();
            }
            Event::Cut => {
                let range = if !current_selection.is_empty() {
                    current_selection
                } else {
                    self.clipboard_current_paragraph()
                };

                ctx.output_mut(|o| o.copied_text = self.buffer[range].into());
                operations.push(Operation::Replace(Replace { range, text: "".into() }));
            }
            Event::Copy => {
                let range = if !current_selection.is_empty() {
                    current_selection
                } else {
                    self.clipboard_current_paragraph()
                };

                ctx.output_mut(|o| o.copied_text = self.buffer[range].into());
            }
            Event::ToggleDebug => {
                self.debug = !self.debug;
            }
            Event::IncrementBaseFontSize => {
                // self.appearance.base_font_size =
                //     self.appearance.base_font_size.map(|size| size + 1.)
            }
            Event::DecrementBaseFontSize => {
                // if self.appearance.font_size() > 2. {
                //     self.appearance.base_font_size =
                //         self.appearance.base_font_size.map(|size| size - 1.)
                // }
            }
        }

        response
    }

    /// Returns true if all text in the given range has style `style`
    pub fn inline_styled(
        &self, root: &'ast AstNode<'ast>, range: (DocCharOffset, DocCharOffset), style: &InlineNode,
    ) -> bool {
        for node in root.descendants() {
            if style.node_type().matches(&node.data.borrow().value)
                && self.node_range(node).contains_range(&range, true, true)
            {
                return true;
            }
        }

        false
    }

    /// Returns true if an inline style would be unapplied instead of applied
    pub fn unapply_inline(
        &self, root: &'ast AstNode<'ast>, range: (DocCharOffset, DocCharOffset), style: &InlineNode,
    ) -> bool {
        let mut unapply = false;
        for inline_paragraph in &self.bounds.inline_paragraphs {
            if inline_paragraph.intersects(&range, true) {
                let paragraph_range = (
                    range.start().max(inline_paragraph.start()),
                    range.end().min(inline_paragraph.end()),
                );

                unapply |= self.inline_styled(root, paragraph_range, style);
            }
        }
        unapply
    }

    /// Returns true if the provided node has style `style`
    pub fn block_styled(&self, node: &'ast AstNode<'ast>, style: &BlockNode) -> bool {
        style.node_type().matches(&node.data.borrow().value)
    }

    /// Returns true if a block style would be unapplied instead of applied
    pub fn unapply_block(&self, root: &'ast AstNode<'ast>, style: &BlockNode) -> bool {
        let mut unapply = false;
        let mut any_selected_blocks = false;
        for node in root.descendants() {
            if self.selected_block(node) {
                any_selected_blocks = true;

                let target_node =
                    if node.is_container_block() { node } else { node.parent().unwrap() };
                unapply |= style.node_type().matches(&target_node.data.borrow().value);
            }
        }

        if !any_selected_blocks {
            // selecting sequence of contiguous empty/whitespace-only lines:
            // check for matching container block
            return style.node_type().matches(
                &self
                    .deepest_container_block_at_offset(root, self.buffer.current.selection.start())
                    .data
                    .borrow()
                    .value,
            );
        }

        unapply
    }

    /// Applies or unapplies `style` to `cursor`, splitting or joining surrounding styles as necessary.
    fn apply_inline_style(
        &self, root: &'ast AstNode<'ast>, range: (DocCharOffset, DocCharOffset), style: InlineNode,
        unapply: bool, operations: &mut Vec<Operation>,
    ) {
        let selection = self.buffer.current.selection;
        if self.buffer.current.text.is_empty() {
            self.insert_head(range.start(), style.clone(), operations);
            operations.push(Operation::Select(selection));
            self.insert_tail(range.start(), style, operations);
            return;
        }

        // find nodes applying given style containing range start and end
        let mut start_node: Option<&'ast AstNode<'ast>> = None;
        for node in root.descendants() {
            if style.node_type().matches(&node.data.borrow().value)
                && self.node_range(node).contains(range.start(), true, true)
            {
                start_node = Some(node);
            }
        }
        let mut end_node: Option<&'ast AstNode<'ast>> = None;
        for node in root.descendants() {
            if style.node_type().matches(&node.data.borrow().value)
                && self.node_range(node).contains(range.end(), true, true)
            {
                end_node = Some(node);
            }
        }

        // if start and end are in different nodes, detail start and dehead end (remove syntax characters inside selection)
        let nodes_same = match (start_node, end_node) {
            (None, None) => true,
            (Some(start), Some(end)) if start.same_node(end) => true,
            _ => false,
        };
        if !nodes_same {
            if let Some(start_node) = start_node {
                self.detail_ast_node(start_node, operations);
            }
            if let Some(end_node) = end_node {
                self.dehead_ast_node(end_node, operations);
            }
        }

        if unapply {
            // if unapplying, tail or dehead node containing start to crop styled region to selection
            if let Some(start_node) = start_node {
                if self.prefix_range(start_node).unwrap().end() < range.start() {
                    let offset = self.adjust_for_whitespace(range.start(), true);
                    self.insert_tail(offset, style.clone(), operations);
                } else {
                    self.dehead_ast_node(start_node, operations);
                }
            }

            // selection must be updated after between changes to start and end to avoid selecting new head/tail
            operations.push(Operation::Select(selection));

            // if unapplying, head or detail node containing end to crop styled region to selection
            if let Some(end_node) = end_node {
                if self.postfix_range(end_node).unwrap().start() > range.end() {
                    let offset = self.adjust_for_whitespace(range.end(), false);
                    self.insert_head(offset, style.clone(), operations);
                } else {
                    self.detail_ast_node(end_node, operations);
                }
            }
        } else {
            // if applying, head start and/or tail end to extend styled region to selection
            if start_node.is_none() {
                let offset = self
                    .adjust_for_whitespace(range.start(), false)
                    .min(range.end());
                self.insert_head(offset, style.clone(), operations)
            }

            // selection must be updated after between changes to start and end to avoid selecting new head/tail
            operations.push(Operation::Select(selection));

            if end_node.is_none() {
                let offset = self
                    .adjust_for_whitespace(range.end(), true)
                    .max(range.start());
                self.insert_tail(offset, style.clone(), operations)
            }
        }

        // remove head and tail for nodes between nodes containing start and end
        for node in root.descendants() {
            // skip the start and end nodes (handled already)
            if let Some(start_node) = start_node {
                if start_node.same_node(node) {
                    continue;
                }
            }
            if let Some(end_node) = end_node {
                if end_node.same_node(node) {
                    continue;
                }
            }

            let style_matches = style.node_type().matches(&node.data.borrow().value);
            if style_matches && self.node_range(node).intersects(&range, true) {
                self.dehead_ast_node(node, operations);
                self.detail_ast_node(node, operations);
            }
        }
    }

    // todo: self by shared reference
    pub fn region_to_range(&mut self, region: Region) -> (DocCharOffset, DocCharOffset) {
        let mut current_selection = self.buffer.current.selection;
        match region {
            Region::Location(location) => self.location_to_char_offset(location).to_range(),
            Region::ToLocation(location) => {
                (current_selection.0, self.location_to_char_offset(location))
            }
            Region::BetweenLocations { start, end } => {
                (self.location_to_char_offset(start), self.location_to_char_offset(end))
            }
            Region::Selection => current_selection,
            Region::SelectionOrOffset { offset, backwards } => {
                if current_selection.is_empty() {
                    current_selection.0 = current_selection.0.advance(
                        &mut self.cursor.x_target,
                        offset,
                        backwards,
                        &self.buffer.current.segs,
                        &self.galleys,
                        &self.bounds,
                    );
                }
                current_selection
            }
            Region::ToOffset { offset, backwards, extend_selection } => {
                if extend_selection
                    || current_selection.is_empty()
                    || matches!(offset, Offset::To(..))
                {
                    let mut selection = current_selection;
                    selection.1 = selection.1.advance(
                        &mut self.cursor.x_target,
                        offset,
                        backwards,
                        &self.buffer.current.segs,
                        &self.galleys,
                        &self.bounds,
                    );
                    if extend_selection {
                        selection.0 = current_selection.0;
                    } else {
                        selection.0 = selection.1;
                    }
                    selection
                } else if backwards {
                    current_selection.start().to_range()
                } else {
                    current_selection.end().to_range()
                }
            }
            Region::Bound { bound, backwards } => {
                let offset = current_selection.1;
                offset
                    .range_bound(bound, backwards, false, &self.bounds)
                    .unwrap_or((offset, offset))
            }
            Region::BoundAt { bound, location, backwards } => {
                let offset = self.location_to_char_offset(location);
                offset
                    .range_bound(bound, backwards, true, &self.bounds)
                    .unwrap_or((offset, offset))
            }
        }
    }

    pub fn location_to_char_offset(&self, location: Location) -> DocCharOffset {
        match location {
            Location::CurrentCursor => self.buffer.current.selection.1,
            Location::DocCharOffset(o) => o,
            Location::Pos(pos) => pos_to_char_offset(pos, &self.galleys),
        }
    }

    fn clipboard_current_paragraph(&self) -> (DocCharOffset, DocCharOffset) {
        let current_selection = self.buffer.current.selection;
        let paragraph_idx = self
            .bounds
            .paragraphs
            .find_containing(current_selection.1, true, true)
            .0;

        let mut result = self.bounds.paragraphs[paragraph_idx];

        // capture leading newline, if any
        if paragraph_idx != 0 {
            let paragraph = self.bounds.paragraphs[paragraph_idx];
            let prev_paragraph = self.bounds.paragraphs[paragraph_idx - 1];
            let range_between_paragraphs = (prev_paragraph.1, paragraph.0);
            let rbp_text = &self.buffer[range_between_paragraphs];
            if rbp_text.ends_with("\r\n") {
                result.0 -= 2;
            } else if rbp_text.ends_with('\n') || rbp_text.ends_with('\r') {
                result.0 -= 1;
            }
        }

        result
    }
}

// todo: find a better home
pub fn pos_to_char_offset(pos: Pos2, galleys: &Galleys) -> DocCharOffset {
    let galley_idx = pos_to_galley(pos, galleys);
    let galley = &galleys[galley_idx];

    let expanded_rect = galley.rect.expand(ROW_SPACING / 2.);

    if pos.y < expanded_rect.min.y {
        // click position is above galley
        galley.range.start()
    } else if pos.y > expanded_rect.max.y {
        // click position is below galley
        galley.range.end()
    } else {
        let relative_pos = pos - galley.rect.min;

        // clamp y coordinate for forgiving cursor placement clicks
        let relative_pos =
            Vec2::new(relative_pos.x, relative_pos.y.clamp(0.0, galley.rect.height()));

        if galley.range.is_empty() {
            // hack: empty galley range means every position in the galley maps to
            // that location
            galley.range.start()
        } else {
            let new_cursor = galley.galley.cursor_from_pos(relative_pos);
            galleys.offset_by_galley_and_cursor(galley, new_cursor)
        }
    }
}

pub fn pos_to_galley(pos: Pos2, galleys: &Galleys) -> usize {
    let mut closest_galley = None;
    let mut closest_distance = (f32::INFINITY, f32::INFINITY);
    for (galley_idx, galley) in galleys.galleys.iter().enumerate() {
        if galley.rect.contains(pos) {
            return galley_idx; // galleys do not overlap
        }

        // this ain't yo mama's distance metric
        let x_distance = distance(pos.x, galley.rect.x_range());
        let y_distance = distance(pos.y, galley.rect.y_range());

        // prefer empty galleys which are placed deliberately to affect such behavior
        if ((y_distance, x_distance) < closest_distance)
            || (((y_distance, x_distance) == closest_distance) && galley.range.is_empty())
        {
            closest_galley = Some(galley_idx);
            closest_distance = (y_distance, x_distance);
        }
    }
    closest_galley.expect("there must always be a galley")
}

pub fn distance(coord: f32, range: Rangef) -> f32 {
    if range.contains(coord) {
        0.
    } else {
        (coord - range.min).abs().min((coord - range.max).abs())
    }
}

impl<'ast> Editor {
    fn dehead_ast_node(&self, node: &'ast AstNode<'ast>, operations: &mut Vec<Operation>) {
        if let Some(prefix_range) = self.prefix_range(node) {
            operations.push(Operation::Replace(Replace { range: prefix_range, text: "".into() }));
        }
    }

    fn detail_ast_node(&self, node: &'ast AstNode<'ast>, operations: &mut Vec<Operation>) {
        if let Some(postfix_range) = self.postfix_range(node) {
            operations.push(Operation::Replace(Replace { range: postfix_range, text: "".into() }));
        }
    }

    fn adjust_for_whitespace(&self, mut offset: DocCharOffset, tail: bool) -> DocCharOffset {
        loop {
            let c = if tail {
                if offset == 0 {
                    break;
                }
                &(&self.buffer)[(offset - 1, offset)]
            } else {
                if offset == self.buffer.current.segs.last_cursor_position() {
                    break;
                }
                &(&self.buffer)[(offset, offset + 1)]
            };
            if c == " " {
                if tail { offset -= 1 } else { offset += 1 }
            } else {
                break;
            }
        }
        offset
    }

    fn insert_head(
        &self, offset: DocCharOffset, style: InlineNode, operations: &mut Vec<Operation>,
    ) {
        let text = style.node_type().head().to_string();
        operations.push(Operation::Replace(Replace { range: offset.to_range(), text }));
    }

    fn insert_tail(
        &self, offset: DocCharOffset, style: InlineNode, operations: &mut Vec<Operation>,
    ) {
        let text = style.node_type().tail().to_string();
        if style.node_type() == InlineNodeType::Link {
            operations.push(Operation::Replace(Replace {
                range: offset.to_range(),
                text: text[..2].into(),
            }));
            operations.push(Operation::Select(offset.to_range()));
            operations.push(Operation::Replace(Replace {
                range: offset.to_range(),
                text: text[2..].into(),
            }));
        } else {
            operations.push(Operation::Replace(Replace { range: offset.to_range(), text }));
        }
    }
}
