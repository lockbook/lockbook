use comrak::nodes::{AlertType, AstNode, NodeAlert};
use egui::{Pos2, Rect, Stroke, TextFormat, Ui, Vec2};

use crate::tab::markdown_plusplus::{widget::INDENT, MarkdownPlusPlus};

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

    pub fn height_alert(&self, node: &'ast AstNode<'ast>, width: f32) -> f32 {
        self.height_item(node, width)
    }

    pub fn show_alert(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, mut top_left: Pos2, mut width: f32,
    ) {
        let height = self.height_alert(node, width);
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
        width -= annotation_space.width();
        self.show_block_children(ui, node, top_left, width);
    }
}
