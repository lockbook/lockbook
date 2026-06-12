use crate::tab::markdown_editor::bounds::{BoundExt as _, RangesExt as _};
use crate::tab::markdown_editor::input::{Event, Increment};
use crate::tab::markdown_editor::widget::utils::{
    NodeValueExt as _, leading_indent_cols, leading_indent_range,
};
use crate::tab::markdown_editor::{MdEdit, MdRender};
use comrak::nodes::{
    AstNode, LineColumn, ListType, NodeAlert, NodeHeading, NodeLink, NodeList, NodeShortCode,
    NodeTaskItem, NodeValue, Sourcepos,
};
use egui::{Pos2, Rangef};
use lb_rs::model::text::buffer::{self};
use lb_rs::model::text::offset_types::{
    Bytes, Grapheme, Graphemes, IntoRangeExt, RangeExt as _, RangeIterExt, ToRangeExt as _,
};
use lb_rs::model::text::operation_types::{Operation, Replace};

use super::{Advance, Bound, Location, Region};

/// tracks editor state necessary to support translating input events to buffer operations
#[derive(Default)]
pub struct EventState {
    pub internal_events: Vec<Event>,
}

impl<'ast> MdEdit {
    /// Translates editor events into buffer operations by interpreting them in the context of the current editor state.
    /// Dispatches events that aren't buffer operations. Returns a (text_updated, selection_updated) pair.
    pub fn calc_operations(
        &mut self, ctx: &egui::Context, root: &'ast AstNode<'ast>, event: Event,
        operations: &mut Vec<Operation>,
    ) -> buffer::Response {
        let current_selection = self.renderer.buffer.current.selection;
        let mut response = buffer::Response::default();
        match event {
            Event::Select { region } => {
                let range = self.region_to_range(region);
                let range = self.renderer.snap_selection_out_of_fold_tags(range);
                operations.push(Operation::Select(range));
            }
            Event::Replace { region, text, advance_cursor } => {
                let mut range = self.region_to_range(region);
                if matches!(region, Region::Selection) {
                    // selecting a fold chip is just selecting a fold tag, but
                    // replacing it should delete folded content
                    range = self.renderer.grow_range_over_fold_contents(range);
                }
                operations.push(Operation::Replace(Replace { range, text }));
                if advance_cursor {
                    operations.push(Operation::Select(range.start().to_range()));
                }
            }
            Event::ToggleStyle { region, style } => {
                // ToggleStyle inserts markdown syntax (e.g. `**…**`); the
                // syntax would render as literal text in plaintext mode.
                if !self.renderer.plaintext {
                    self.toggle_style(root, region, style, current_selection, operations);
                }
            }
            Event::Camera => {
                response.open_camera = true;
            }
            Event::Newline { shift } => {
                // Enter after a `···` fold chip creates the new heading / list
                // item after the hidden contents
                let handled = !shift && self.newline_at_fold(root, operations);
                if handled {
                    return response;
                }

                // insert/extend/terminate container blocks
                let mut handled = || {
                    // selection must be empty
                    let Some(offset) = self.renderer.selection_offset() else {
                        return false;
                    };

                    let container = self
                        .renderer
                        .deepest_container_block_at_offset(root, offset);
                    let line = self.renderer.line_at_offset(offset);
                    let line_content = self.renderer.line_content(container, line);
                    let own_prefix = self.renderer.line_own_prefix(container, line);

                    let in_code_block = matches!(
                        self.renderer
                            .leaf_block_at_offset(root, offset)
                            .data
                            .borrow()
                            .value,
                        NodeValue::CodeBlock(_)
                    );

                    if shift || in_code_block {
                        // shift -> extend
                        operations.push(Operation::Replace(Replace {
                            range: current_selection,
                            text: "\n".into(),
                        }));
                        if let Some(extension_prefix) = self.renderer.extension_prefix(container) {
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
                        if let Some(insertion_prefix) = self.renderer.insertion_prefix(container) {
                            operations.push(Operation::Replace(Replace {
                                range: current_selection,
                                text: insertion_prefix,
                            }));
                        };
                    }

                    // code block auto-indentation
                    if in_code_block {
                        let line_content_start = self.renderer.offset_to_byte(line_content.start());
                        let indentation_len = Bytes(
                            self.renderer.buffer[line_content].len()
                                - self.renderer.buffer[line_content].trim_start().len(),
                        );
                        let indentation =
                            (line_content_start, line_content_start + indentation_len);
                        let indentation = self.renderer.range_to_char(indentation);

                        operations.push(Operation::Replace(Replace {
                            range: current_selection,
                            text: self.renderer.buffer[indentation].to_string(),
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
                // Deleting against a folded region unfolds it rather than
                // editing hidden text; delete_at_fold pushes its own ops,
                // cursor placement included.
                let handled = self.delete_at_fold(region, operations);
                if handled {
                    return response;
                }

                // delete container block prefix
                let mut handled = || {
                    // must be mostly vanilla backspace
                    if !matches!(
                        region,
                        Region::SelectionOrAdvance {
                            advance: Advance::Next(Bound::Word) | Advance::By(Increment::Char),
                            backwards: true,
                        }
                    ) {
                        return false;
                    }

                    // selection must be empty
                    let Some(offset) = self.renderer.selection_offset() else {
                        return false;
                    };

                    let container = self
                        .renderer
                        .deepest_container_block_at_offset(root, offset);
                    let line = self.renderer.line_at_offset(offset);
                    let own_prefix = self.renderer.line_own_prefix(container, line);
                    let content = self.renderer.line_content(container, line);

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
                    let mut range = self.region_to_range(region);

                    // selecting a fold chip is just selecting a fold tag, but
                    // deleting it should delete folded content
                    if !current_selection.is_empty() {
                        range = self.renderer.grow_range_over_fold_contents(range);
                    }

                    let range = self.renderer.grow_delete_over_fold_tags(range);
                    operations.push(Operation::Replace(Replace { range, text: "".into() }));
                }

                // advance cursor
                operations.push(Operation::Select(current_selection.start().to_range()));
            }
            Event::Indent { deindent } => {
                let selected_lines = self
                    .renderer
                    .bounds
                    .source_lines
                    .find_intersecting(current_selection, true);
                let first_selected_line_idx = selected_lines.0;
                let first_selected_line =
                    self.renderer.bounds.source_lines[first_selected_line_idx];

                if !deindent {
                    // indent into extension of block on prior line
                    let mut handled = || {
                        // must not be first line
                        if first_selected_line_idx == 0 {
                            return false;
                        }

                        let prior_line_idx = first_selected_line_idx - 1;
                        let prior_line = self.renderer.bounds.source_lines[prior_line_idx];
                        let prior_line_deepest_container = self
                            .renderer
                            .deepest_container_block_at_offset(root, prior_line.end());
                        let first_selected_line_deepest = self
                            .renderer
                            .deepest_container_block_at_offset(root, first_selected_line.end());

                        // Pick the highest matching ancestor (last
                        // assignment wins). Use AST ancestry — list-
                        // item continuation prefixes are claimed
                        // greedily by the outermost item, so prefix
                        // presence isn't a reliable "already inside"
                        // signal. See `consume_indent_columns`.
                        let mut prior_line_container_extension_prefix = None;
                        for prior_line_container in prior_line_deepest_container.ancestors() {
                            let has_prefix_on_prior_line = !self
                                .renderer
                                .line_own_prefix(prior_line_container, prior_line)
                                .is_empty();
                            let already_inside_prior_container = first_selected_line_deepest
                                .ancestors()
                                .any(|a| a.same_node(prior_line_container));

                            if has_prefix_on_prior_line && !already_inside_prior_container {
                                if let Some(extension_prefix) =
                                    self.renderer.extension_own_prefix(prior_line_container)
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

                        // Indent only the selected line(s). Children
                        // keep their absolute indentation and are not
                        // dragged along; an item with children is
                        // aligned to its deepest direct child so that
                        // child's level is unchanged (it detaches to a
                        // sibling instead of being pushed a level
                        // deeper). Symmetric to deindent dropping to
                        // the parent's column.
                        let mut done = std::collections::BTreeSet::new();
                        for line_idx in selected_lines.iter() {
                            if !done.insert(line_idx) {
                                continue;
                            }
                            let line = self.renderer.bounds.source_lines[line_idx];
                            let container = self
                                .renderer
                                .deepest_container_block_at_offset(root, line.end());
                            let container_own_prefix =
                                self.renderer.line_own_prefix(container, line);

                            // Marker line: insert before the marker
                            // (whole block shifts). Continuation:
                            // insert after the prefix (content
                            // shifts).
                            let is_continuation =
                                Some(self.renderer.buffer[container_own_prefix].to_string())
                                    == self.renderer.extension_own_prefix(container);
                            let insertion_offset = if is_continuation {
                                container_own_prefix.end()
                            } else {
                                container_own_prefix.start()
                            };

                            // For a whitespace-nested item with
                            // children, indent it to its deepest
                            // direct child's column instead of one
                            // unit, so the child detaches as a sibling
                            // at its unchanged level.
                            let mut insert_text = prior_line_container_extension_prefix.clone();
                            if !is_continuation
                                && !insert_text.is_empty()
                                && insert_text.bytes().all(|b| b == b' ')
                            {
                                if let Some(max_child_col) =
                                    max_child_marker_col(&self.renderer, container)
                                {
                                    let cur_col = leading_indent_cols(&self.renderer.buffer[line]);
                                    let one_unit = insert_text.chars().count();
                                    let want = (cur_col + one_unit).max(max_child_col);
                                    insert_text = " ".repeat(want - cur_col);
                                }
                            }
                            // Only `1.` can interrupt the parent item's
                            // paragraph; a higher source number (common in
                            // imported lists) collapses the new sublist into
                            // plain text. Later items render off the list
                            // start, so forcing `1.` is harmless.
                            let renumber = (!is_continuation)
                                .then(|| ordered_marker_digits(&self.renderer, container, line))
                                .flatten();
                            match renumber {
                                Some(digits) => operations.push(Operation::Replace(Replace {
                                    range: digits,
                                    text: format!("{insert_text}1"),
                                })),
                                None => operations.push(Operation::Replace(Replace {
                                    range: insertion_offset.into_range(),
                                    text: insert_text,
                                })),
                            }
                        }

                        true
                    };
                    if !handled() {
                        // default -> do nothing
                    }
                } else {
                    // Cascade descendants too, so children stay
                    // nested under a deindented parent.
                    let mut handled = || {
                        for line_idx in selected_lines.iter() {
                            let line = self.renderer.bounds.source_lines[line_idx];
                            if find_deindent(&self.renderer, root, line).is_none() {
                                return false;
                            }
                        }
                        let mut done = std::collections::BTreeSet::new();
                        for line_idx in selected_lines.iter() {
                            if !done.insert(line_idx) {
                                continue;
                            }
                            let line = self.renderer.bounds.source_lines[line_idx];
                            let Some((range, text)) = find_deindent(&self.renderer, root, line)
                            else {
                                continue;
                            };
                            let cur_cols = leading_indent_cols(&self.renderer.buffer[range]);
                            let new_cols = text.chars().count();
                            let delta = -(cur_cols.saturating_sub(new_cols) as isize);
                            operations.push(Operation::Replace(Replace { range, text }));
                            let container = self
                                .renderer
                                .deepest_container_block_at_offset(root, line.end());
                            cascade_deindent_delta(
                                &self.renderer,
                                container,
                                delta,
                                &mut done,
                                operations,
                            );
                        }
                        true
                    };
                    if !handled() {
                        // default -> do nothing
                    }
                }

                operations.push(Operation::Select(current_selection));
            }
            Event::Undo => {
                response |= self.renderer.buffer.undo();
            }
            Event::Redo => {
                response |= self.renderer.buffer.redo();
            }
            Event::Cut => {
                let range = if !current_selection.is_empty() {
                    current_selection
                } else {
                    self.clipboard_current_line()
                };
                // selecting a fold chip is just selecting a fold tag, but
                // copying it should copy folded content
                let range = self.renderer.grow_range_over_fold_contents(range);

                ctx.copy_text(self.renderer.buffer[range].into());
                operations.push(Operation::Replace(Replace { range, text: "".into() }));
            }
            Event::Copy => {
                let range = if !current_selection.is_empty() {
                    current_selection
                } else {
                    self.clipboard_current_line()
                };
                let range = self.renderer.grow_range_over_fold_contents(range);

                ctx.copy_text(self.renderer.buffer[range].into());
            }
            Event::ToggleDebug => {
                self.renderer.debug = !self.renderer.debug;
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
            Event::ToggleFold => {
                let unapply = self.unapply_fold(root);
                for node in root.descendants() {
                    if matches!(node.data().value, NodeValue::Heading(_))
                        && self.renderer.selected_block(node)
                    {
                        self.renderer.apply_fold(
                            node,
                            self.renderer.heading_contents(node),
                            unapply,
                        );
                    }

                    if matches!(node.data().value, NodeValue::Item(_) | NodeValue::TaskItem(_))
                        && self.renderer.selected_fold_item(node)
                    {
                        self.renderer
                            .apply_fold(node, self.renderer.item_contents(node), unapply);
                    }
                }
            }
        }

        response
    }

    fn toggle_style(
        &mut self, root: &'ast AstNode<'ast>, region: Region, style: NodeValue,
        current_selection: (Grapheme, Grapheme), operations: &mut Vec<Operation>,
    ) {
        let range = self.region_to_range(region);

        match style {
            NodeValue::Document | NodeValue::Paragraph => {}
            _ if style.is_inline() => {
                let unapply = self.unapply_inline(root, range, &style);

                for inline_paragraph in &self.renderer.bounds.inline_paragraphs {
                    if inline_paragraph.intersects(&range, true) {
                        let paragraph_range = (
                            range.start().max(inline_paragraph.start()),
                            range.end().min(inline_paragraph.end()),
                        );

                        self.apply_inline_style(
                            root,
                            paragraph_range,
                            style.clone(),
                            unapply,
                            operations,
                        );
                    }
                }

                // todo: advance cursor
            }
            _ if style.is_leaf_block() || style.is_container_block() => {
                let unapply = self.unapply_block(root, &style);

                let mut handled = false;
                for node in root.descendants() {
                    if self.renderer.selected_block(node) {
                        handled = true;

                        // apply heading to ATX heading: replace existing heading
                        if let NodeValue::Heading(NodeHeading { level, .. }) = style.node_type() {
                            if let NodeValue::Heading(NodeHeading {
                                level: node_level,
                                setext: false,
                                ..
                            }) = node.data.borrow().value
                            {
                                for line_idx in self.renderer.node_lines(node).iter() {
                                    let line = self.renderer.bounds.source_lines[line_idx];
                                    let node_line = self.renderer.node_line(node, line);

                                    if level > node_level {
                                        let add_levels = level - node_level;
                                        operations.push(Operation::Replace(Replace {
                                            range: node_line.start().into_range(),
                                            text: "#".repeat(add_levels as _),
                                        }));
                                    } else if level == node_level {
                                        // remove heading
                                        let mut range = (
                                            node_line.start(),
                                            node_line.start() + Graphemes(node_level as _),
                                        );
                                        if self.renderer.buffer.current.segs.last_cursor_position()
                                            > range.end()
                                            && &self.renderer.buffer[(range.end(), range.end() + 1)]
                                                == " "
                                        {
                                            range.1 += 1;
                                        }

                                        operations.push(Operation::Replace(Replace {
                                            range,
                                            text: "".into(),
                                        }));
                                    } else {
                                        let remove_levels = node_level - level;
                                        operations.push(Operation::Replace(Replace {
                                            range: (
                                                node_line.start(),
                                                node_line.start() + Graphemes(remove_levels as _),
                                            ),
                                            text: "".into(),
                                        }));
                                    }
                                }
                            } else if NodeValue::Paragraph == node.data.borrow().value {
                                for line_idx in self.renderer.node_lines(node).iter() {
                                    let line = self.renderer.bounds.source_lines[line_idx];
                                    let node_line = self.renderer.node_line(node, line);

                                    // count paragraph soft breaks as node breaks
                                    if node.data.borrow().value == NodeValue::Paragraph
                                        && !line.intersects(
                                            &self.renderer.buffer.current.selection,
                                            true,
                                        )
                                    {
                                        continue;
                                    }

                                    operations.push(Operation::Replace(Replace {
                                        range: node_line.start().into_range(),
                                        text: "#".repeat(level as _) + " ",
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
                            for line_idx in self.renderer.node_lines(node).iter() {
                                let line = self.renderer.bounds.source_lines[line_idx];

                                let prefix = self.renderer.line_own_prefix(target_node, line);

                                operations.push(Operation::Replace(Replace {
                                    range: prefix,
                                    text: "".into(),
                                }));
                            }

                            if !unapply {
                                let mut first_line = true;
                                for line_idx in self.renderer.node_lines(node).iter() {
                                    let line = self.renderer.bounds.source_lines[line_idx];

                                    // count paragraph soft breaks as node breaks
                                    if node.data.borrow().value == NodeValue::Paragraph
                                        && !line.intersects(
                                            &self.renderer.buffer.current.selection,
                                            true,
                                        )
                                    {
                                        continue;
                                    }

                                    let range = self
                                        .renderer
                                        .line_ancestors_prefix(node, line)
                                        .end()
                                        .into_range();
                                    let text = match style {
                                        NodeValue::Heading(_) => unreachable!(),
                                        NodeValue::BlockQuote => "> ",
                                        NodeValue::Code(_) => unimplemented!(), // todo: support inserting lines
                                        NodeValue::List(NodeList {
                                            list_type: ListType::Bullet,
                                            is_task_list: false,
                                            ..
                                        }) => {
                                            if first_line {
                                                "* "
                                            } else {
                                                "  "
                                            }
                                        }
                                        NodeValue::List(NodeList {
                                            list_type: ListType::Ordered,
                                            ..
                                        }) => {
                                            if first_line {
                                                "1. "
                                            } else {
                                                "   "
                                            }
                                        }
                                        NodeValue::List(NodeList {
                                            list_type: ListType::Bullet,
                                            is_task_list: true,
                                            ..
                                        }) => {
                                            if first_line {
                                                "* [ ] "
                                            } else {
                                                "  "
                                            }
                                        }
                                        _ => unimplemented!(), // many such cases!
                                    }
                                    .into();

                                    operations.push(Operation::Replace(Replace { range, text }));

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
                        let text = match style {
                            NodeValue::Heading(NodeHeading { level, .. }) => {
                                // todo: technically this makes a bunch of separate headings
                                match level {
                                    1 => "# ",
                                    2 => "## ",
                                    3 => "### ",
                                    4 => "#### ",
                                    5 => "##### ",
                                    _ => "###### ",
                                }
                            }
                            NodeValue::BlockQuote => "> ",
                            NodeValue::Code(_) => unimplemented!(), // todo: support inserting lines
                            NodeValue::List(NodeList {
                                list_type: ListType::Bullet,
                                is_task_list: false,
                                ..
                            }) => "* ",
                            NodeValue::List(NodeList { list_type: ListType::Ordered, .. }) => "1. ",
                            NodeValue::List(NodeList {
                                list_type: ListType::Bullet,
                                is_task_list: true,
                                ..
                            }) => "* [ ] ",
                            _ => unimplemented!(), // many such cases!
                        }
                        .into();

                        operations.push(Operation::Replace(Replace { range, text }));
                    } else {
                        let target_node = self.renderer.deepest_container_block_at_offset(
                            root,
                            self.renderer.buffer.current.selection.start(),
                        );
                        if style == target_node.node_type() {
                            let line_idx = self
                                .renderer
                                .range_lines(current_selection.start().into_range())
                                .start();
                            let line = self.renderer.bounds.source_lines[line_idx];

                            let prefix = self.renderer.line_own_prefix(target_node, line);

                            operations.push(Operation::Replace(Replace {
                                range: prefix,
                                text: "".into(),
                            }));
                        }
                    }
                }

                // advance cursor (affects type change of empty list items)
                operations.push(Operation::Select(current_selection));
            }
            _ => {}
        }
    }

    /// Returns true if all text in the given range has style `style`
    pub fn inline_styled(
        &self, root: &'ast AstNode<'ast>, range: (Grapheme, Grapheme), style: &NodeValue,
    ) -> bool {
        for node in root.descendants() {
            if &node.node_type() == style
                && self
                    .renderer
                    .node_range(node)
                    .contains_range(&range, true, true)
            {
                return true;
            }
        }

        false
    }

    /// Returns true if an inline style would be unapplied instead of applied
    pub fn unapply_inline(
        &self, root: &'ast AstNode<'ast>, range: (Grapheme, Grapheme), style: &NodeValue,
    ) -> bool {
        let mut unapply = false;
        for inline_paragraph in &self.renderer.bounds.inline_paragraphs {
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

    /// Returns true if a block style would be unapplied instead of applied
    pub fn unapply_block(&self, root: &'ast AstNode<'ast>, style: &NodeValue) -> bool {
        let mut unapply = false;
        let mut any_selected_blocks = false;
        for node in root.descendants() {
            if self.renderer.selected_block(node) {
                any_selected_blocks = true;

                // Walk up the ancestor chain: the selected block (a leaf
                // like Paragraph, or its immediate container) may sit
                // several levels below the structural container that
                // bears the toggled style. `* foo` has `List > Item >
                // Paragraph` — the bullet style lives on `List`, two
                // levels above the selected Paragraph.
                let mut maybe = if node.is_container_block() { Some(node) } else { node.parent() };
                while let Some(ancestor) = maybe {
                    let ancestor_type = ancestor.node_type();
                    if std::mem::discriminant(&ancestor_type) == std::mem::discriminant(style) {
                        if &ancestor_type == style {
                            unapply = true;
                        }
                        break;
                    }
                    maybe = ancestor.parent();
                }
            }
        }

        if !any_selected_blocks {
            // selecting sequence of contiguous empty/whitespace-only lines:
            // check for matching container block
            return &self
                .renderer
                .deepest_container_block_at_offset(
                    root,
                    self.renderer.buffer.current.selection.start(),
                )
                .node_type()
                == style;
        }

        unapply
    }

    /// Returns true if a fold command should unfold instead of fold
    pub fn unapply_fold(&self, root: &'ast AstNode<'ast>) -> bool {
        let mut unapply = false;
        for node in root.descendants() {
            if matches!(node.data().value, NodeValue::Heading(_))
                && self.renderer.selected_block(node)
                && self.renderer.fold(node).is_some()
            {
                unapply = true;
            }

            if matches!(node.data().value, NodeValue::Item(_) | NodeValue::TaskItem(_))
                && self.renderer.selected_fold_item(node)
                && self.renderer.fold(node).is_some()
            {
                unapply = true;
            }
        }

        unapply
    }

    #[allow(clippy::collapsible_else_if)]
    pub fn apply_fold(
        &mut self, node: &'ast AstNode<'ast>, contents: (Grapheme, Grapheme), unapply: bool,
    ) {
        self.renderer.apply_fold(node, contents, unapply);
    }

    /// Applies or unapplies `style` to `cursor`, splitting or joining surrounding styles as necessary.
    fn apply_inline_style(
        &self, root: &'ast AstNode<'ast>, range: (Grapheme, Grapheme), style: NodeValue,
        unapply: bool, operations: &mut Vec<Operation>,
    ) {
        let selection = self.renderer.buffer.current.selection;
        if self.renderer.buffer.current.text.is_empty() {
            self.insert_head(range.start(), style.clone(), operations);
            operations.push(Operation::Select(selection));
            self.insert_tail(range.start(), style, operations);
            return;
        }

        // find nodes applying given style containing range start and end
        let mut start_node: Option<&'ast AstNode<'ast>> = None;
        for node in root.descendants() {
            if node.node_type() == style
                && self
                    .renderer
                    .node_range(node)
                    .contains(range.start(), true, true)
            {
                start_node = Some(node);
            }
        }
        let mut end_node: Option<&'ast AstNode<'ast>> = None;
        for node in root.descendants() {
            if node.node_type() == style
                && self
                    .renderer
                    .node_range(node)
                    .contains(range.end(), true, true)
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
                if self.renderer.head_range(start_node).unwrap().end() < range.start() {
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
                if self.renderer.tail_range(end_node).unwrap().start() > range.end() {
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

            let style_matches = node.node_type() == style;
            if style_matches && self.renderer.node_range(node).intersects(&range, true) {
                self.dehead_ast_node(node, operations);
                self.detail_ast_node(node, operations);
            }
        }
    }

    // todo: self by shared reference
    pub fn region_to_range(&mut self, region: Region) -> (Grapheme, Grapheme) {
        // Pointer click ends an up/down chain — the next arrow must
        // start from the click x, not the stale column.
        let has_pos = |loc: &Location| matches!(loc, Location::Pos(_));
        let clicked = match &region {
            Region::Location(loc) | Region::ToLocation(loc) => has_pos(loc),
            Region::BetweenLocations { start, end } => has_pos(start) || has_pos(end),
            Region::BoundAt { location, .. } => has_pos(location),
            _ => false,
        };
        if clicked {
            self.cursor.x_target = None;
        }
        let mut current_selection = self.renderer.buffer.current.selection;
        match region {
            Region::Location(location) => self.location_to_range(location),
            Region::ToLocation(location) => {
                (current_selection.0, self.location_to_char_offset(location))
            }
            Region::BetweenLocations { start, end } => {
                (self.location_to_char_offset(start), self.location_to_char_offset(end))
            }
            Region::Selection => current_selection,
            Region::SelectionOrAdvance { advance: offset, backwards } => {
                if current_selection.is_empty() {
                    current_selection.0 = self.advance(current_selection.0, offset, backwards);
                }
                current_selection
            }
            Region::ToAdvance { advance, backwards, extend_selection } => {
                if extend_selection
                    || current_selection.is_empty()
                    || matches!(advance, Advance::To(..))
                {
                    let mut selection = current_selection;
                    selection.1 = self.advance(selection.1, advance, backwards);
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
                    .range_bound(bound, backwards, false, &self.renderer.bounds)
                    .unwrap_or((offset, offset))
            }
            Region::BoundAt { bound, location, backwards } => {
                let offset = self.location_to_char_offset(location);
                offset
                    .range_bound(bound, backwards, true, &self.renderer.bounds)
                    .unwrap_or((offset, offset))
            }
        }
    }

    pub fn location_to_range(&self, location: Location) -> (Grapheme, Grapheme) {
        match location {
            Location::CurrentCursor => self.renderer.buffer.current.selection,
            Location::Grapheme(o) => o.into_range(),
            Location::Pos(pos) => self.pos_to_range(pos),
        }
    }

    pub fn location_to_char_offset(&self, location: Location) -> Grapheme {
        match location {
            // A pointer endpoint edge-snaps atomic fragments by x (see
            // `pos_to_char_offset`), rather than collapsing to the range
            // start — so a drag across an atomic span (e.g. a list marker
            // or one indentation column) selects the whole span instead
            // of an empty range.
            Location::Pos(pos) => self.pos_to_char_offset(pos),
            _ => self.location_to_range(location).0,
        }
    }

    fn clipboard_current_line(&self) -> (Grapheme, Grapheme) {
        let current_selection = self.renderer.buffer.current.selection;
        let paragraph_idx = self
            .renderer
            .bounds
            .source_lines
            .find_containing(current_selection.1, true, true)
            .0;

        let mut result = self.renderer.bounds.source_lines[paragraph_idx];

        // capture leading newline, if any
        if paragraph_idx != 0 {
            let line = self.renderer.bounds.source_lines[paragraph_idx];
            let prev_line = self.renderer.bounds.source_lines[paragraph_idx - 1];
            let range_between_lines = (prev_line.1, line.0);
            let rbl_text = &self.renderer.buffer[range_between_lines];
            if rbl_text.ends_with("\r\n") {
                result.0 -= 2;
            } else if rbl_text.ends_with('\n') || rbl_text.ends_with('\r') {
                result.0 -= 1;
            }
        }

        result
    }

    // todo: find a better home
    pub fn pos_to_range(&self, pos: Pos2) -> (Grapheme, Grapheme) {
        let Some(frag_idx) = self.renderer.closest_fragment_at_pos(pos) else {
            return Grapheme(0).into_range();
        };
        let frag = &self.renderer.fragments[frag_idx];

        // Past the last fragment's bottom: jump to doc end.
        let is_last = frag_idx + 1 == self.renderer.fragments.len();
        if is_last && pos.y > frag.rect.max.y {
            return self
                .renderer
                .buffer
                .current
                .segs
                .last_cursor_position()
                .into_range();
        }

        if frag.source_range.is_empty() {
            // Anchor / empty-range fragment: every position maps to
            // its source point.
            frag.source_range.start().into_range()
        } else if frag.atomic {
            // Override / atomic fragment: hit-test snaps to the
            // nearest edge of its source range, returning the full
            // range so callers that want "select the whole thing"
            // get that semantics.
            frag.source_range
        } else {
            self.renderer.fragment_offset(frag, pos.x).into_range()
        }
    }

    /// Resolves a pointer position to a single cursor offset. Unlike
    /// [`pos_to_range`] (which returns an atomic fragment's *whole*
    /// range so a click selects it), this edge-snaps an atomic fragment
    /// to the nearer edge by `pos.x` via [`fragment_offset`]. That lets
    /// a drag's anchor and moving end land on opposite edges of a marker
    /// / indentation column and select it, rather than both collapsing
    /// to its start.
    pub fn pos_to_char_offset(&self, pos: Pos2) -> Grapheme {
        let Some(frag_idx) = self.renderer.closest_fragment_at_pos(pos) else {
            return Grapheme(0);
        };
        let frag = &self.renderer.fragments[frag_idx];

        // Past the last fragment's bottom: jump to doc end.
        let is_last = frag_idx + 1 == self.renderer.fragments.len();
        if is_last && pos.y > frag.rect.max.y {
            return self.renderer.buffer.current.segs.last_cursor_position();
        }

        // The hit-test inverse: edge-snaps atomic fragments by the rect
        // midpoint, walks clusters otherwise, and collapses an empty
        // range to its start.
        self.renderer.fragment_offset(frag, pos.x)
    }
}

pub fn distance(coord: f32, range: Rangef) -> f32 {
    if range.contains(coord) {
        0.
    } else {
        (coord - range.min).abs().min((coord - range.max).abs())
    }
}

/// Buffer edit that deindents `line` past `ancestor`, or `None`.
/// Items/TaskItems/FootnoteDefinitions nest via whitespace: rewrite
/// the indent run minus one level's columns (tabs expand to spaces).
/// BlockQuotes/Alerts nest via `>` markers: drop them, but only when
/// nested inside another quote/alert (top-level deindent would
/// silently demote `> foo` to a paragraph).
fn deindent_replacement<'ast>(
    renderer: &MdRender, ancestor: &'ast AstNode<'ast>, line: (Grapheme, Grapheme),
) -> Option<((Grapheme, Grapheme), String)> {
    use NodeValue::*;
    match &ancestor.data.borrow().value {
        Item(_) | TaskItem(_) | FootnoteDefinition(_) => {
            let level_cols = renderer.deindent_level_cols(ancestor)?;
            if level_cols == 0 {
                return None;
            }
            let indent_range = leading_indent_range(&renderer.buffer, line);
            let cur_cols = leading_indent_cols(&renderer.buffer[indent_range]);
            // Land at `ancestor`'s own marker column: escaping a
            // container drops to its level. Using the marker column
            // (not `cur - padding`) pops out one full level even when
            // the source over-indents the nested item — otherwise an
            // item indented past the minimum only moves part-way and
            // stays nested.
            let target_cols =
                leading_indent_cols(&renderer.buffer[renderer.node_first_line(ancestor)]);
            if cur_cols <= target_cols {
                return None;
            }
            Some((indent_range, " ".repeat(target_cols)))
        }
        BlockQuote | Alert(_) => {
            // No-op at top level — only deindent when there's an
            // outer quote/alert to escape into.
            let nested = ancestor
                .ancestors()
                .skip(1)
                .any(|a| matches!(&a.data.borrow().value, BlockQuote | Alert(_)));
            if !nested {
                return None;
            }
            let own_prefix = renderer.line_own_prefix(ancestor, line);
            if own_prefix.is_empty() {
                return None;
            }
            Some((own_prefix, String::new()))
        }
        _ => None,
    }
}

/// First deindent edit produced walking the line's container
/// ancestors. `skip_container` walks past the deepest container
/// when the cursor's on its marker line, so shift-tab there
/// deindents the whole block instead of its content.
fn find_deindent<'ast>(
    renderer: &MdRender, root: &'ast AstNode<'ast>, line: (Grapheme, Grapheme),
) -> Option<((Grapheme, Grapheme), String)> {
    let container = renderer.deepest_container_block_at_offset(root, line.end());
    let container_own_prefix = renderer.line_own_prefix(container, line);
    let skip_container = Some(renderer.buffer[container_own_prefix].to_string())
        != renderer.extension_own_prefix(container);
    container.ancestors().find_map(|ancestor| {
        if container.same_node(ancestor) && skip_container {
            return None;
        }
        deindent_replacement(renderer, ancestor, line)
    })
}

/// Largest leading-indent column among `item`'s direct child list
/// items (the items one level nested under it), or `None` if it has
/// none. Used so indent can align the item to its child, leaving the
/// child's level unchanged.
/// Grapheme range of an ordered-list item's leading marker digits on `line`
/// (the `12` in `12. `), or `None` if `container` isn't an ordered list item.
fn ordered_marker_digits<'ast>(
    renderer: &MdRender, container: &'ast AstNode<'ast>, line: (Grapheme, Grapheme),
) -> Option<(Grapheme, Grapheme)> {
    let parent = container.parent()?;
    if !matches!(
        parent.data.borrow().value,
        NodeValue::List(NodeList { list_type: ListType::Ordered, .. })
    ) {
        return None;
    }
    let own_prefix = renderer.line_own_prefix(container, line);
    let digits = renderer.buffer[own_prefix]
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .count();
    (digits > 0).then(|| (own_prefix.start(), own_prefix.start() + Graphemes(digits)))
}

fn max_child_marker_col<'ast>(renderer: &MdRender, item: &'ast AstNode<'ast>) -> Option<usize> {
    item.children()
        .filter(|c| matches!(&c.data.borrow().value, NodeValue::List(_)))
        .flat_map(|list| list.children())
        .filter(|n| matches!(&n.data.borrow().value, NodeValue::Item(_) | NodeValue::TaskItem(_)))
        .map(|child| {
            let line = renderer.bounds.source_lines[renderer.node_first_line_idx(child)];
            leading_indent_cols(&renderer.buffer[line])
        })
        .max()
}

/// Drags `container`'s descendants left with a deindented parent so
/// children stay nested instead of being stranded — a child left four
/// or more columns deep with no list-item ancestor reparses as an
/// indented code block. `delta_cols` is negative (the columns removed
/// from the parent). Only fires for whitespace-nested containers (Item,
/// TaskItem, FootnoteDefinition); quote/alert nesting is per-line
/// markers with no column delta. Tabs in the indent expand to spaces.
///
/// There is deliberately no indent counterpart: indenting only the
/// targeted line(s) can't strand a child (it always retains a list
/// ancestor), so a tight child simply re-parents to a sibling.
fn cascade_deindent_delta<'ast>(
    renderer: &MdRender, container: &'ast AstNode<'ast>, delta_cols: isize,
    done: &mut std::collections::BTreeSet<usize>, operations: &mut Vec<Operation>,
) {
    if !matches!(
        &container.data.borrow().value,
        NodeValue::Item(_) | NodeValue::TaskItem(_) | NodeValue::FootnoteDefinition(_)
    ) {
        return;
    }

    let first = renderer.node_first_line_idx(container);
    let last = renderer.node_last_line_idx(container);
    for i in first..=last {
        if !done.insert(i) {
            continue;
        }
        let line = renderer.bounds.source_lines[i];
        let range = leading_indent_range(&renderer.buffer, line);
        let cur = leading_indent_cols(&renderer.buffer[range]) as isize;
        let new = (cur + delta_cols).max(0) as usize;
        operations.push(Operation::Replace(Replace { range, text: " ".repeat(new) }));
    }
}

impl<'ast> MdEdit {
    fn dehead_ast_node(&self, node: &'ast AstNode<'ast>, operations: &mut Vec<Operation>) {
        if let Some(range) = self.renderer.head_range(node) {
            operations.push(Operation::Replace(Replace { range, text: "".into() }));
        }
    }

    fn detail_ast_node(&self, node: &'ast AstNode<'ast>, operations: &mut Vec<Operation>) {
        if let Some(range) = self.renderer.tail_range(node) {
            operations.push(Operation::Replace(Replace { range, text: "".into() }));
        }
    }

    fn adjust_for_whitespace(&self, mut offset: Grapheme, tail: bool) -> Grapheme {
        loop {
            let c = if tail {
                if offset == 0 {
                    break;
                }
                &(&self.renderer.buffer)[(offset - 1, offset)]
            } else {
                if offset == self.renderer.buffer.current.segs.last_cursor_position() {
                    break;
                }
                &(&self.renderer.buffer)[(offset, offset + 1)]
            };
            if c == " " {
                if tail { offset -= 1 } else { offset += 1 }
            } else {
                break;
            }
        }
        offset
    }

    fn insert_head(&self, offset: Grapheme, style: NodeValue, operations: &mut Vec<Operation>) {
        let text = style.node_type().head().to_string();
        operations.push(Operation::Replace(Replace { range: offset.to_range(), text }));
    }

    fn insert_tail(&self, offset: Grapheme, style: NodeValue, operations: &mut Vec<Operation>) {
        let text = style.node_type().tail().to_string();
        if let NodeValue::Link(link) = style {
            let NodeLink { url, .. } = *link;

            operations.push(Operation::Replace(Replace {
                range: offset.to_range(),
                text: text[..2].into(),
            }));
            let url_empty = url.is_empty();
            if url_empty {
                operations.push(Operation::Select(offset.to_range()));
            } else {
                operations
                    .push(Operation::Replace(Replace { range: offset.to_range(), text: url }));
            }
            operations.push(Operation::Replace(Replace {
                range: offset.to_range(),
                text: text[2..].into(),
            }));
            if !url_empty {
                operations.push(Operation::Select(offset.to_range()));
            }
        } else {
            operations.push(Operation::Replace(Replace { range: offset.to_range(), text }));
        }
    }
}

trait NodeType {
    fn node_type(&self) -> NodeValue;
}

impl NodeType for AstNode<'_> {
    fn node_type(&self) -> NodeValue {
        self.data.borrow().value.node_type()
    }
}

impl NodeType for NodeValue {
    fn node_type(&self) -> NodeValue {
        match self {
            NodeValue::Document => NodeValue::Document,
            NodeValue::FrontMatter(_) => NodeValue::FrontMatter(Default::default()),
            NodeValue::BlockQuote => NodeValue::BlockQuote,
            // Preserve the discriminating fields so toggle-style
            // comparisons can tell bullet / ordered / task lists apart.
            // Other NodeList fields (padding, start, etc.) are layout
            // and shouldn't affect type equality.
            NodeValue::List(l) => NodeValue::List(NodeList {
                list_type: l.list_type,
                is_task_list: l.is_task_list,
                ..Default::default()
            }),
            NodeValue::Item(l) => NodeValue::Item(NodeList {
                list_type: l.list_type,
                is_task_list: l.is_task_list,
                ..Default::default()
            }),
            NodeValue::DescriptionList => NodeValue::DescriptionList,
            NodeValue::DescriptionItem(_) => NodeValue::DescriptionItem(Default::default()),
            NodeValue::DescriptionTerm => NodeValue::DescriptionTerm,
            NodeValue::DescriptionDetails => NodeValue::DescriptionDetails,
            NodeValue::CodeBlock(_) => NodeValue::CodeBlock(Default::default()),
            NodeValue::HtmlBlock(_) => NodeValue::HtmlBlock(Default::default()),
            NodeValue::Paragraph => NodeValue::Paragraph,
            // headings are the only thing with any data preserved
            NodeValue::Heading(heading) => {
                NodeValue::Heading(NodeHeading { level: heading.level, ..Default::default() })
            }
            NodeValue::ThematicBreak => NodeValue::ThematicBreak,
            NodeValue::FootnoteDefinition(_) => NodeValue::FootnoteDefinition(Default::default()),
            NodeValue::Table(_) => NodeValue::Table(Default::default()),
            NodeValue::TableRow(_) => NodeValue::TableRow(Default::default()),
            NodeValue::TableCell => NodeValue::TableCell,
            NodeValue::Text(_) => NodeValue::Text(Default::default()),
            // wish this had a Default impl
            NodeValue::TaskItem(_) => NodeValue::TaskItem(NodeTaskItem {
                symbol: Default::default(),
                symbol_sourcepos: Sourcepos {
                    start: LineColumn { line: Default::default(), column: Default::default() },
                    end: LineColumn { line: Default::default(), column: Default::default() },
                },
            }),
            NodeValue::SoftBreak => NodeValue::SoftBreak,
            NodeValue::LineBreak => NodeValue::LineBreak,
            NodeValue::Code(_) => NodeValue::Code(Default::default()),
            NodeValue::HtmlInline(_) => NodeValue::HtmlInline(Default::default()),
            NodeValue::Raw(_) => NodeValue::Raw(Default::default()),
            NodeValue::Emph => NodeValue::Emph,
            NodeValue::Strong => NodeValue::Strong,
            NodeValue::Strikethrough => NodeValue::Strikethrough,
            NodeValue::Highlight => NodeValue::Highlight,
            NodeValue::Superscript => NodeValue::Superscript,
            NodeValue::Link(_) => NodeValue::Link(Default::default()),
            NodeValue::Image(_) => NodeValue::Image(Default::default()),
            NodeValue::FootnoteReference(_) => NodeValue::FootnoteReference(Default::default()),
            // wish this had a Default impl
            NodeValue::ShortCode(_) => {
                NodeValue::ShortCode(NodeShortCode { code: "".into(), emoji: "".into() }.into())
            }
            NodeValue::Math(_) => NodeValue::Math(Default::default()),
            NodeValue::MultilineBlockQuote(_) => NodeValue::MultilineBlockQuote(Default::default()),
            NodeValue::Escaped => NodeValue::Escaped,
            NodeValue::WikiLink(_) => NodeValue::WikiLink(Default::default()),
            NodeValue::Underline => NodeValue::Underline,
            NodeValue::Subscript => NodeValue::Subscript,
            NodeValue::SpoileredText => NodeValue::SpoileredText,
            NodeValue::EscapedTag(_) => NodeValue::EscapedTag(Default::default()),
            // wish this had a Default impl
            NodeValue::Alert(_) => NodeValue::Alert(
                NodeAlert {
                    alert_type: Default::default(),
                    title: Default::default(),
                    multiline: Default::default(),
                    fence_length: Default::default(),
                    fence_offset: Default::default(),
                }
                .into(),
            ),
            NodeValue::Subtext => NodeValue::Subtext,
        }
    }
}

trait HeadTail {
    fn head(&self) -> &'static str;
    fn tail(&self) -> &'static str;
}

impl HeadTail for NodeValue {
    fn head(&self) -> &'static str {
        match self {
            NodeValue::Code(_) => "`",
            NodeValue::Emph => "*",
            NodeValue::Strong => "**",
            NodeValue::Strikethrough => "~~",
            NodeValue::Link(_) => "[",
            NodeValue::Image(_) => "![",
            NodeValue::Highlight => "==",
            NodeValue::Underline => "__",
            NodeValue::SpoileredText => "||",
            NodeValue::Subscript => "~",
            NodeValue::Superscript => "^",
            _ => unimplemented!(), // many such cases!
        }
    }

    fn tail(&self) -> &'static str {
        match self {
            NodeValue::Code(_) => "`",
            NodeValue::Emph => "*",
            NodeValue::Strong => "**",
            NodeValue::Strikethrough => "~~",
            NodeValue::Link(_) => "]()",
            NodeValue::Image(_) => "]()",
            NodeValue::Highlight => "==",
            NodeValue::Underline => "__",
            NodeValue::SpoileredText => "||",
            NodeValue::Subscript => "~",
            NodeValue::Superscript => "^",
            _ => unimplemented!(), // many such cases!
        }
    }
}
