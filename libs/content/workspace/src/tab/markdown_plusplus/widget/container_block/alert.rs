use comrak::nodes::{AlertType, AstNode, NodeAlert};
use egui::{Pos2, Rect, Stroke, TextFormat, Ui, Vec2};
use lb_rs::model::text::offset_types::{DocCharOffset, RelCharOffset};

use crate::tab::markdown_plusplus::{
    widget::{Wrap, INDENT},
    MarkdownPlusPlus,
};

impl<'ast> MarkdownPlusPlus {
    pub fn text_format_alert(&self, parent: &AstNode<'_>, node_alert: &NodeAlert) -> TextFormat {
        let parent_text_format = self.text_format(parent);
        TextFormat {
            color: match node_alert.alert_type {
                AlertType::Note => self.theme.fg().blue,
                AlertType::Tip => self.theme.fg().green,
                AlertType::Important => self.theme.fg().magenta,
                AlertType::Warning => self.theme.fg().yellow,
                AlertType::Caution => self.theme.fg().red,
            },
            ..parent_text_format
        }
    }

    pub fn height_alert(&self, node: &'ast AstNode<'ast>) -> f32 {
        self.height_item(node)
    }

    pub fn show_alert(&mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, mut top_left: Pos2) {
        let height = self.height_alert(node);
        let annotation_size = Vec2 { x: INDENT, y: height };
        let annotation_space = Rect::from_min_size(top_left, annotation_size);

        ui.painter().vline(
            annotation_space.center().x,
            annotation_space.y_range(),
            Stroke::new(3., self.text_format(node).color),
        );

        // debug
        // ui.painter()
        //     .rect_stroke(annotation_space, 2., egui::Stroke::new(1., self.theme.fg().blue));

        top_left.x += annotation_space.width();
        self.show_block_children(ui, node, top_left);
    }

    // todo: review handling of [!NOTE] line
    pub fn line_prefix_len_alert(
        &self, node: &'ast AstNode<'ast>, line: (DocCharOffset, DocCharOffset),
        node_alert: &NodeAlert,
    ) -> RelCharOffset {
        let NodeAlert { multiline, .. } = node_alert;
        if *multiline {
            self.line_prefix_len_multiline_block_quote(node, line)
        } else {
            self.line_prefix_len_block_quote(node, line)
        }
    }

    // todo: review handling of [!NOTE] line
    pub fn show_line_prefix_alert(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, line: (DocCharOffset, DocCharOffset),
        top_left: Pos2, height: f32, row_height: f32, node_alert: &NodeAlert,
    ) {
        let NodeAlert { multiline, .. } = node_alert;
        if *multiline {
            self.show_line_prefix_multiline_block_quote(
                ui, node, line, top_left, height, row_height,
            );
        } else {
            self.show_line_prefix_block_quote(ui, node, line, top_left, height, row_height);
        }
    }
}
