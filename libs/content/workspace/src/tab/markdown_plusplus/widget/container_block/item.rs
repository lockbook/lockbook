use comrak::nodes::{AstNode, ListType, NodeList, NodeValue};
use egui::text::LayoutJob;
use egui::{Pos2, Rect, Ui, Vec2};
use lb_rs::model::text::offset_types::{DocCharOffset, RangeExt as _, RelCharOffset};

use crate::tab::markdown_plusplus::widget::{BULLET_RADIUS, INDENT, ROW_HEIGHT};
use crate::tab::markdown_plusplus::MarkdownPlusPlus;

// https://github.github.com/gfm/#list-items
impl<'ast> MarkdownPlusPlus {
    pub fn height_item(&self, node: &'ast AstNode<'ast>) -> f32 {
        self.block_children_height(node)
    }

    pub fn show_item(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, mut top_left: Pos2, node_list: &NodeList,
    ) {
        let NodeList { list_type, start, .. } = *node_list;

        // todo: better bullet position for nested blocks -
        let annotation_size = Vec2 { x: INDENT, y: ROW_HEIGHT };
        let annotation_space = Rect::from_min_size(top_left, annotation_size);

        let text_format = self.text_format_syntax(node);

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

        // debug
        // ui.painter()
        //     .rect_stroke(annotation_space, 2., egui::Stroke::new(1., self.theme.fg().blue));

        top_left.x += annotation_space.width();
        self.show_block_children(ui, node, top_left);
    }

    // This routine is simple because indentation for list items must be
    // consistent within a list and comrak gives us the information we need
    pub fn line_prefix_len_item(
        &self, node: &'ast AstNode<'ast>, line: (DocCharOffset, DocCharOffset),
        node_list: &NodeList,
    ) -> RelCharOffset {
        let NodeList { padding, .. } = *node_list;
        let parent = node.parent().unwrap();
        let mut result = self.line_prefix_len(parent, line);

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
        result += padding;

        // "If a sequence of lines Ls constitutes a list item according to rule
        // #1, #2, or #3, then the result of indenting each line of Ls by 1-3
        // spaces (the same for each line) also constitutes a list item with the
        // same contents and attributes."
        let NodeValue::List(NodeList { marker_offset: indentation, .. }) =
            parent.data.borrow().value
        else {
            unreachable!("items always have list parents")
        };
        result += indentation;

        // "If a string of lines Ls constitute a list item with contents Bs, then
        // the result of deleting some or all of the indentation from one or
        // more lines in which the next non-whitespace character after the
        // indentation is paragraph continuation text is a list item with the
        // same contents and attributes."
        //
        // "If a line is empty, then it need not be indented."
        result.min(line.len())
    }
}
