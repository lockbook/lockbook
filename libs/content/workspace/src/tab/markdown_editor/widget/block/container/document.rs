use comrak::nodes::AstNode;
use egui::{FontId, Pos2, TextFormat};
use lb_rs::model::text::offset_types::RangeIterExt as _;

use crate::tab::markdown_editor::Editor;
use crate::tab::markdown_editor::widget::utils::wrap_layout::Wrap;
use crate::tab::markdown_editor::widget::{ROW_HEIGHT, ROW_SPACING};

impl<'ast> Editor {
    pub fn text_format_document(&self) -> TextFormat {
        let parent_text_format = TextFormat::default();
        TextFormat {
            color: self.theme.fg().neutral_secondary,
            font_id: FontId {
                size: parent_text_format.font_id.size * ROW_HEIGHT
                    / self
                        .ctx
                        .fonts(|fonts| fonts.row_height(&parent_text_format.font_id)),
                ..parent_text_format.font_id
            },
            ..parent_text_format
        }
    }

    pub fn height_document(&self, node: &'ast AstNode<'ast>) -> f32 {
        let width = self.width(node);

        let any_children = node.children().next().is_some();
        if any_children && !self.plaintext_mode {
            self.block_children_height(node)
        } else {
            let mut result = 0.;
            for line_idx in self.node_lines(node).iter() {
                let line = self.bounds.source_lines[line_idx];
                result +=
                    self.height_section(&mut Wrap::new(width), line, self.text_format_syntax(node));
                result += ROW_SPACING;
            }
            result
        }
    }

    pub fn show_document(
        &mut self, ui: &mut egui::Ui, node: &'ast AstNode<'ast>, mut top_left: Pos2,
    ) {
        let width = self.width(node);

        let any_children = node.children().next().is_some();
        if any_children && !self.plaintext_mode {
            self.show_block_children(ui, node, top_left);
        } else {
            for line_idx in self.node_lines(node).iter() {
                let line = self.bounds.source_lines[line_idx];

                let mut wrap = Wrap::new(width);
                self.show_section(
                    ui,
                    top_left,
                    &mut wrap,
                    line,
                    self.text_format_syntax(node),
                    false,
                );
                top_left.y += wrap.height();
                top_left.y += ROW_SPACING;
                self.bounds.wrap_lines.extend(wrap.row_ranges);
            }
        }
    }

    pub fn compute_bounds_document(&mut self, node: &'ast AstNode<'ast>) {
        let any_children = node.children().next().is_some();
        if any_children {
            self.compute_bounds_block_children(node);
        } else {
            for line_idx in self.node_lines(node).iter() {
                let line = self.bounds.source_lines[line_idx];
                self.bounds.inline_paragraphs.push(line);
            }
        }
    }
}
