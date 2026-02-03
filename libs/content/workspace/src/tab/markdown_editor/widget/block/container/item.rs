use comrak::nodes::{AstNode, ListType, NodeList, NodeValue};
use egui::text::LayoutJob;
use egui::{Pos2, Rect, Ui, Vec2};
use lb_rs::model::text::offset_types::{
    DocCharOffset, IntoRangeExt as _, RangeExt as _, RangeIterExt as _, RelCharOffset,
};

use crate::tab::markdown_editor::Editor;
use crate::tab::markdown_editor::widget::utils::wrap_layout::Wrap;
use crate::tab::markdown_editor::widget::{BULLET_RADIUS, INDENT, ROW_HEIGHT};

// https://github.github.com/gfm/#list-items
impl<'ast> Editor {
    pub fn height_item(&self, node: &'ast AstNode<'ast>) -> f32 {
        let any_children = node.children().next().is_some();
        if any_children {
            self.block_children_height(node)
        } else {
            let line = self.node_first_line(node);
            let line_content = self.line_content(node, line);

            self.height_section(
                &mut Wrap::new(self.width(node) - INDENT),
                line_content,
                self.text_format_syntax(node),
            )
        }
    }

    pub fn show_item(&mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2) {
        let first_line = self.node_first_line(node);
        let row_height = self.node_line_row_height(node, first_line);

        let parent = node.parent().unwrap();
        let NodeValue::List(node_list) = parent.data.borrow().value else {
            unreachable!("items always have list parents")
        };
        let NodeList { list_type, start, .. } = node_list;

        let annotation_size = Vec2 { x: INDENT, y: row_height };
        let annotation_space = Rect::from_min_size(top_left, annotation_size);

        let mut annotation_text_format = self.text_format_syntax(node);
        annotation_text_format.color = self.theme.fg().neutral_tertiary;
        match list_type {
            ListType::Bullet => {
                ui.painter().circle_filled(
                    annotation_space.center(),
                    BULLET_RADIUS * row_height / ROW_HEIGHT,
                    annotation_text_format.color,
                );
            }
            ListType::Ordered => {
                let siblings = self.sorted_siblings(node);
                let sibling_index = self.sibling_index(node, &siblings);
                let number = start + sibling_index;

                let text = format!("{number}.");
                let layout_job = LayoutJob::single_section(text, annotation_text_format);
                let galley = ui.fonts(|fonts| fonts.layout_job(layout_job));
                ui.painter()
                    .galley(annotation_space.left_top(), galley, Default::default());
            }
        }

        let any_children = node.children().next().is_some();
        let hovered = if any_children {
            self.show_block_children(ui, node, top_left + INDENT * Vec2::X);

            // todo: proper hit-testing (this ignores anything covering the space)
            let children_height = self.block_children_height(node);
            let children_space =
                Rect::from_min_size(top_left, Vec2::new(self.width(node), children_height));
            children_space.contains(ui.input(|i| i.pointer.latest_pos().unwrap_or_default()))
        } else {
            let line = self.node_first_line(node);
            let line_content = self.line_content(node, line);

            let mut wrap = Wrap::new(self.width(node) - INDENT);
            let resp = self.show_section(
                ui,
                top_left + INDENT * Vec2::X,
                &mut wrap,
                line_content,
                self.text_format_syntax(node),
                false,
            );
            self.bounds.wrap_lines.extend(wrap.row_ranges);

            resp.hovered
        };

        // fold button
        // todo: proper hit-testing (this ignores anything covering the space)
        let pointer = ui.input(|i| i.pointer.latest_pos().unwrap_or_default());

        let (fold_button_size, fold_button_icon_size, fold_button_space) =
            Self::fold_button_size_icon_size_space(top_left, row_height);
        let show_fold_button = self.touch_mode
            || hovered
            || fold_button_space.contains(pointer)
            || annotation_space.contains(pointer)
            || self.fold(node).is_some()
            || self.selected_fold_item(node);
        if !show_fold_button {
            return;
        }

        self.show_fold_button(
            ui,
            node,
            (fold_button_size, fold_button_icon_size, fold_button_space),
            self.item_contents(node),
            self.item_fold_reveal(node),
        );
    }

    pub fn own_prefix_len_item(
        &self, node: &'ast AstNode<'ast>, line: (DocCharOffset, DocCharOffset),
        node_list: &NodeList,
    ) -> Option<RelCharOffset> {
        let node_line = self.node_line(node, line);
        let mut result: RelCharOffset = 0.into();

        // "If a sequence of lines Ls constitutes a list item according to rule
        // #1, #2, or #3, then the result of indenting each line of Ls by 1-3
        // spaces (the same for each line) also constitutes a list item with the
        // same contents and attributes."
        let indentation = {
            let first_line = self.node_first_line(node);
            let first_node_line = self.node_line(node, first_line);

            let text = &self.buffer[first_node_line];
            if text.starts_with("   ") {
                "   ".len()
            } else if text.starts_with("  ") {
                "  ".len()
            } else if text.starts_with(" ") {
                " ".len()
            } else {
                0
            }
        };
        let NodeList { padding: marker_width_including_spaces, .. } = *node_list;
        if line == self.node_first_line(node) {
            result += indentation;

            // "If a sequence of lines Ls constitute a sequence of blocks Bs starting
            // with a non-whitespace character, and M is a list marker of width W
            // followed by 1 ≤ N ≤ 4 spaces, then the result of prepending M and the
            // following spaces to the first line of Ls, and indenting subsequent lines
            // of Ls by W + N spaces, is a list item with Bs as its contents."
            //
            // "If a sequence of lines Ls starting with a single blank line
            // constitute a (possibly empty) sequence of blocks Bs, not separated
            // from each other by more than one blank line, and M is a list marker
            // of width W, then the result of prepending M to the first line of Ls,
            // and indenting subsequent lines of Ls by W + 1 spaces, is a list item
            // with Bs as its contents."
            result += marker_width_including_spaces;
        } else {
            // "If a string of lines Ls constitute a list item with contents Bs, then
            // the result of deleting some or all of the indentation from one or
            // more lines in which the next non-whitespace character after the
            // indentation is paragraph continuation text is a list item with the
            // same contents and attributes."
            //
            // "If a line is empty, then it need not be indented."
            let text = &self.buffer[node_line];
            for i in 0..(indentation + marker_width_including_spaces) {
                if text.starts_with(&" ".repeat(indentation + marker_width_including_spaces - i)) {
                    result += indentation + marker_width_including_spaces - i;
                    break;
                }
            }
        }

        // marker_width_including_spaces reports the width _with_ spaces even
        // when they're not present
        Some(result.min(node_line.len()))
    }

    pub fn compute_bounds_item(&mut self, node: &'ast AstNode<'ast>) {
        // Push bounds for line prefix (bullet/number annotation)
        for line_idx in self.node_lines(node).iter() {
            let line = self.bounds.source_lines[line_idx];
            let line_own_prefix = self.line_own_prefix(node, line);

            self.bounds.paragraphs.push(line_own_prefix);
        }

        // Handle children or line content
        let any_children = node.children().next().is_some();
        if any_children {
            self.compute_bounds_block_children(node);
        } else {
            let line = self.node_first_line(node);
            let line_content = self.line_content(node, line);

            self.bounds.paragraphs.push(line_content);
            self.bounds.inline_paragraphs.push(line_content);
        }
    }

    pub fn item_contents(&self, node: &'ast AstNode<'ast>) -> (DocCharOffset, DocCharOffset) {
        // contents start at the end of the first child, which acts as a sort of section title
        // if no children, start at end of node first line
        let mut contents = if let Some(first_child) = node.children().next() {
            self.node_range(first_child).end().into_range()
        } else {
            self.node_first_line(node).end().into_range()
        };

        let sorted_siblings = self.sorted_siblings(node);
        let sibling_index = self.sibling_index(node, &sorted_siblings);

        if let Some(sibling) = sorted_siblings[sibling_index + 1..].first() {
            let sibling_first_line = self.node_first_line_idx(sibling);
            let last_line = sibling_first_line - 1;
            contents.1 = self.bounds.source_lines[last_line].end();
        } else {
            // absent a next sibling, we contain the remaining content of the
            // parent
            contents.1 = self.node_range(node.parent().unwrap()).end();
        }

        contents
    }

    /// Returns true if the item contents should be revealed whether the heading is folded or not
    pub fn item_fold_reveal(&self, node: &'ast AstNode<'ast>) -> bool {
        self.item_contents(node)
            .contains_range(&self.buffer.current.selection, false, true)
    }

    /// Returns true if the item is selected for folding; specialized adaptation of self.selected_block()
    pub fn selected_fold_item(&self, node: &'ast AstNode<'ast>) -> bool {
        // any items selected -> those items selected for fold
        let root = node.ancestors().last().unwrap();
        for descendent in root.descendants() {
            if self.selected_block(descendent)
                && matches!(descendent.data().value, NodeValue::Item(_) | NodeValue::TaskItem(_))
            {
                return self.selected_block(node);
            }
        }

        // else -> parent item of any selected items selected for fold
        node.first_child()
            .map(|c| self.selected_block(c))
            .unwrap_or_default()
    }
}
