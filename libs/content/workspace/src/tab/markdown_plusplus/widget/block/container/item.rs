use comrak::nodes::{AstNode, ListType, NodeList, NodeValue};
use egui::text::LayoutJob;
use egui::{Pos2, Rect, Ui, Vec2};
use lb_rs::model::text::offset_types::{
    DocCharOffset, RangeExt as _, RangeIterExt as _, RelCharOffset,
};

use crate::tab::markdown_plusplus::widget::utils::text_layout::Wrap;
use crate::tab::markdown_plusplus::widget::{BULLET_RADIUS, INDENT, ROW_HEIGHT};
use crate::tab::markdown_plusplus::MarkdownPlusPlus;

// https://github.github.com/gfm/#list-items
impl<'ast> MarkdownPlusPlus {
    pub fn height_item(&self, node: &'ast AstNode<'ast>) -> f32 {
        let any_children = node.children().next().is_some();
        if any_children {
            self.block_children_height(node)
        } else {
            let line = self.node_first_line(node);
            let line_content = self.line_content(node, line);

            self.height_text_line(
                &mut Wrap::new(self.width(node) - INDENT),
                line_content,
                self.text_format_syntax(node),
            )
        }
    }

    pub fn show_item(&mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, mut top_left: Pos2) {
        let first_line = self.node_first_line(node);
        let row_height = self.node_line_row_height(node, first_line);

        let parent = node.parent().unwrap();
        let NodeValue::List(node_list) = parent.data.borrow().value else {
            unreachable!("items always have list parents")
        };
        let NodeList { list_type, start, .. } = node_list;

        let annotation_size = Vec2 { x: INDENT, y: row_height };
        let annotation_space = Rect::from_min_size(top_left, annotation_size);
        let text_format = self.text_format_syntax(node);

        match list_type {
            ListType::Bullet => {
                ui.painter().circle_filled(
                    annotation_space.center(),
                    BULLET_RADIUS * row_height / ROW_HEIGHT,
                    text_format.color,
                );
            }
            ListType::Ordered => {
                let siblings = self.sorted_siblings(node);
                let sibling_index = self.sibling_index(node, &siblings);
                let number = start + sibling_index;

                let text = format!("{}.", number);
                let layout_job = LayoutJob::single_section(text, text_format);
                let galley = ui.fonts(|fonts| fonts.layout_job(layout_job));
                ui.painter()
                    .galley(annotation_space.left_top(), galley, Default::default());
            }
        }
        for line_idx in self.node_lines(node).iter() {
            let line = self.bounds.source_lines[line_idx];

            self.bounds
                .paragraphs
                .push(self.line_own_prefix(node, line));
        }

        top_left.x += INDENT;

        let any_children = node.children().next().is_some();
        if any_children {
            self.show_block_children(ui, node, top_left);
        } else {
            let line = self.node_first_line(node);
            let line_content = self.line_content(node, line);

            self.bounds.paragraphs.push(line_content);
            self.show_text_line(
                ui,
                top_left,
                &mut Wrap::new(self.width(node) - INDENT),
                line_content,
                self.text_format_syntax(node),
                false,
            );
        }
    }

    pub fn line_prefix_len_item(
        &self, node: &'ast AstNode<'ast>, line: (DocCharOffset, DocCharOffset),
        node_list: &NodeList,
    ) -> Option<RelCharOffset> {
        let node_line = self.node_line(node, line);
        let mut result = 0.into();

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

        Some(result)
    }
}
