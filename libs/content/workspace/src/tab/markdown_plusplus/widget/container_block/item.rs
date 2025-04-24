use comrak::nodes::{AstNode, ListType, NodeList, NodeValue};
use egui::text::LayoutJob;
use egui::{Pos2, Rect, Ui, Vec2};
use lb_rs::model::text::offset_types::{DocCharOffset, RangeExt as _, RelCharOffset};

use crate::tab::markdown_plusplus::widget::{Wrap, BULLET_RADIUS, INDENT, ROW_HEIGHT};
use crate::tab::markdown_plusplus::MarkdownPlusPlus;

// https://github.github.com/gfm/#list-items
impl<'ast> MarkdownPlusPlus {
    pub fn height_item(&self, node: &'ast AstNode<'ast>) -> f32 {
        self.block_children_height(node)
    }

    pub fn show_item(&mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, mut top_left: Pos2) {
        // todo: better bullet position for nested blocks -
        let annotation_size = Vec2 { x: INDENT, y: ROW_HEIGHT };
        let annotation_space = Rect::from_min_size(top_left, annotation_size);

        // debug
        // ui.painter()
        //     .rect_stroke(annotation_space, 2., egui::Stroke::new(1., self.theme.fg().blue));

        top_left.x += annotation_space.width();
        self.show_block_children(ui, node, top_left);
    }

    pub fn line_prefix_len_item(
        &self, node: &'ast AstNode<'ast>, line: (DocCharOffset, DocCharOffset),
        node_list: &NodeList,
    ) -> RelCharOffset {
        let NodeList { padding: marker_width_including_spaces, .. } = *node_list;
        let parent = node.parent().unwrap();
        let parent_prefix_len = self.line_prefix_len(parent, line);
        let mut result = parent_prefix_len;

        // "If a sequence of lines Ls constitutes a list item according to rule
        // #1, #2, or #3, then the result of indenting each line of Ls by 1-3
        // spaces (the same for each line) also constitutes a list item with the
        // same contents and attributes."
        let NodeValue::List(NodeList { marker_offset: indentation, .. }) =
            parent.data.borrow().value
        else {
            unreachable!("items always have list parents")
        };

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
            let text = &self.buffer[(line.start() + parent_prefix_len, line.end())];
            for i in 0..(marker_width_including_spaces + indentation) {
                if text.starts_with(&" ".repeat(marker_width_including_spaces + indentation - i)) {
                    result += marker_width_including_spaces + indentation - i;
                    break;
                }
            }
        }

        result.min(line.len())
    }

    pub fn show_line_prefix_item(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, line: (DocCharOffset, DocCharOffset),
        top_left: Pos2, _height: f32, row_height: f32, node_list: &NodeList,
    ) {
        let NodeList { list_type, start, .. } = *node_list;

        let annotation_size = Vec2 { x: INDENT, y: row_height };
        let annotation_space = Rect::from_min_size(top_left, annotation_size);
        let text_format = self.text_format_syntax(node);

        let line_prefix_len = self.line_prefix_len(node, line);
        let parent_line_prefix_len = self.line_prefix_len(node.parent().unwrap(), line);
        let prefix_range = (line.start() + parent_line_prefix_len, line.start() + line_prefix_len);

        if prefix_range.intersects(&self.buffer.current.selection, false)
            || self
                .buffer
                .current
                .selection
                .contains(prefix_range.start(), true, true)
        {
            let mut wrap = Wrap::new(INDENT);
            self.show_text_line(ui, top_left, &mut wrap, prefix_range, text_format, false);
        } else if line == self.node_first_line(node) {
            match list_type {
                ListType::Bullet => {
                    ui.painter().circle_filled(
                        annotation_space.center(),
                        BULLET_RADIUS,
                        text_format.color,
                    );
                }
                ListType::Ordered => {
                    let text = format!("{}.", start);
                    let layout_job = LayoutJob::single_section(text, text_format);
                    let galley = ui.fonts(|fonts| fonts.layout_job(layout_job));
                    ui.painter()
                        .galley(annotation_space.left_top(), galley, Default::default());
                }
            }
        }

        if !prefix_range.is_empty() {
            self.bounds.paragraphs.push(prefix_range);
        }
    }
}
