use comrak::nodes::{AstNode, NodeLink, NodeValue};
use egui::{Pos2, Ui};
use lb_rs::model::text::offset_types::{Grapheme, RangeExt, RangeIterExt as _};

use crate::tab::markdown_editor::MdRender;
use crate::tab::markdown_editor::widget::utils::wrap_layout::Layout;

impl<'ast> MdRender {
    pub fn height_paragraph(&self, node: &'ast AstNode<'ast>) -> f32 {
        let mut result = 0.;
        if !self.disable_images {
            for descendant in node.descendants() {
                if let NodeValue::Image(node_link) = &descendant.data.borrow().value {
                    let NodeLink { url, .. } = &**node_link;
                    result += self.height_image(node, url);
                    result += self.layout.block_spacing;
                }
            }
        }

        let last_line_idx = self.node_last_line_idx(node);
        for line_idx in self.node_lines(node).iter() {
            let line = self.bounds.source_lines[line_idx];
            let node_line = self.node_line(node, line);

            result += self.height_paragraph_line(node, node_line);

            if line_idx != last_line_idx {
                result += self.layout.block_spacing;
            }
        }

        result
    }

    /// Build a `Layout` for one paragraph line. Shared by height +
    /// show passes — both consume the resulting `WrapUnitLayout`.
    /// Walks the paragraph's pre/infix/post slices (handles leading/
    /// trailing whitespace per CommonMark "raw content" rule), and
    /// recurses into inline children for the inline range.
    fn layout_paragraph_line(
        &self, node: &'ast AstNode<'ast>, node_line: (Grapheme, Grapheme),
    ) -> Layout {
        let mut layout = Layout::new(node_line);
        if let Some((pre_node, pre_children, _, post_children, post_node)) =
            self.split_range(node, node_line)
        {
            let fmt = self.text_format(node);
            if !pre_node.is_empty() {
                layout.push_source(pre_node, &self.buffer[pre_node], fmt.clone());
            }
            if !pre_children.is_empty() {
                layout.push_source(pre_children, &self.buffer[pre_children], fmt.clone());
            }
            self.layout_inline_children(&mut layout, node, node_line);
            if !post_children.is_empty() {
                layout.push_source(post_children, &self.buffer[post_children], fmt.clone());
            }
            if !post_node.is_empty() {
                layout.push_source(post_node, &self.buffer[post_node], fmt);
            }
        } else {
            // Empty paragraph line such as in `- [ ] \n  x`.
            let fmt = self.text_format(node);
            layout.push_source(node_line, &self.buffer[node_line], fmt);
        }
        layout
    }

    pub fn height_paragraph_line(
        &self, node: &'ast AstNode<'ast>, node_line: (Grapheme, Grapheme),
    ) -> f32 {
        let width = self.width(node);
        let layout = self.layout_paragraph_line(node, node_line);
        self.compute_layout_from(layout, width, self.layout.row_height)
            .height
    }

    pub fn show_paragraph(&mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, mut top_left: Pos2) {
        for line in self.node_lines(node).iter() {
            let line = self.bounds.source_lines[line];
            let node_line = self.node_line(node, line);

            if !self.disable_images {
                for descendant in node.descendants() {
                    if let NodeValue::Image(node_link) = &descendant.data.borrow().value {
                        let NodeLink { url, .. } = &**node_link;
                        if node_line.contains_inclusive(self.node_range(descendant).start()) {
                            self.show_image_block(ui, node, top_left, url);
                            top_left.y += self.height_image(node, url);
                            top_left.y += self.layout.block_spacing;
                        }
                    }
                }
            }

            let line_height = self.height_paragraph_line(node, node_line);

            self.show_paragraph_line(ui, node, top_left, node_line);
            top_left.y += line_height;

            top_left.y += self.layout.block_spacing;
        }
    }

    pub fn show_paragraph_line(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2,
        node_line: (Grapheme, Grapheme),
    ) {
        let width = self.width(node);
        let layout = self.layout_paragraph_line(node, node_line);
        let result = self.compute_layout_from(layout, width, self.layout.row_height);
        self.show_wrap_layout(ui, top_left, &result);
    }

    pub fn compute_bounds_paragraph(&mut self, node: &'ast AstNode<'ast>) {
        for line in self.node_lines(node).iter() {
            let line = self.bounds.source_lines[line];
            let node_line = self.node_line(node, line);

            self.bounds.inline_paragraphs.push(node_line);
        }
    }
}
