use comrak::nodes::AstNode;
use egui::{Pos2, Rect, Stroke, TextFormat, Ui, Vec2};

use crate::tab::markdown_plusplus::{widget::INDENT, MarkdownPlusPlus};

impl<'ast> MarkdownPlusPlus {
    pub fn text_format_block_quote(&self, parent: &AstNode<'_>) -> TextFormat {
        let parent_text_format = self.text_format(parent);
        TextFormat { color: self.theme.fg().neutral_tertiary, ..parent_text_format }
    }

    pub fn height_block_quote(&self, node: &'ast AstNode<'ast>, width: f32) -> f32 {
        self.height_item(node, width)
    }

    pub fn show_block_quote(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, mut top_left: Pos2, mut width: f32,
    ) {
        let height = self.height_block_quote(node, width);
        let annotation_size = Vec2 { x: INDENT, y: height };
        let annotation_space = Rect::from_min_size(top_left, annotation_size);

        ui.painter().vline(
            annotation_space.center().x,
            annotation_space.y_range(),
            Stroke::new(3., self.theme.bg().neutral_tertiary),
        );

        // debug
        // ui.painter()
        //     .rect_stroke(annotation_space, 2., egui::Stroke::new(1., self.theme.fg().blue));

        top_left.x += annotation_space.width();
        width -= annotation_space.width();
        self.show_block_children(ui, node, top_left, width);
    }
}
