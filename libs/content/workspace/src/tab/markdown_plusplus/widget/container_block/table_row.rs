use std::sync::Arc;

use comrak::nodes::AstNode;
use egui::{Context, FontFamily, FontId, Pos2, Rangef, Rect, Stroke, TextFormat, Ui, Vec2};

use crate::tab::markdown_plusplus::{
    widget::{Ast, Block},
    MarkdownPlusPlus,
};

pub struct TableRow<'a, 't, 'w> {
    ast: &'w Ast<'a, 't>,
    is_header_row: bool,
}

impl MarkdownPlusPlus {
    pub fn text_format_table_row(&self, parent: &AstNode<'_>, is_header_row: bool) -> TextFormat {
        let parent_text_format = self.text_format(parent);
        TextFormat {
            font_id: FontId {
                family: if is_header_row {
                    FontFamily::Name(Arc::from("Bold"))
                } else {
                    FontFamily::Proportional
                },
                ..parent_text_format.font_id
            },
            ..parent_text_format
        }
    }
}

impl<'a, 't, 'w> TableRow<'a, 't, 'w> {
    pub fn new(ast: &'w Ast<'a, 't>, is_header_row: bool) -> Self {
        Self { ast, is_header_row }
    }

    pub fn text_format(parent_text_format: TextFormat, is_header_row: bool) -> TextFormat {
        TextFormat {
            font_id: FontId {
                family: if is_header_row {
                    FontFamily::Name(Arc::from("Bold"))
                } else {
                    FontFamily::Proportional
                },
                ..parent_text_format.font_id
            },
            ..parent_text_format
        }
    }
}

impl Block for TableRow<'_, '_, '_> {
    fn show(&self, width: f32, top_left: Pos2, ui: &mut Ui) {
        // draw row backgrounds
        let row_rect =
            Rect::from_min_size(top_left, Vec2::new(width, self.height(width, ui.ctx())));
        if self.is_header_row {
            ui.painter()
                .rect_filled(row_rect, 0., self.ast.theme.bg().neutral_secondary);
        }

        // draw cell contents
        let mut child_top_left = top_left;
        let child_width = width / self.ast.children.len() as f32;
        for child in &self.ast.children {
            Block::show(child, child_width, child_top_left, ui);
            child_top_left.x += child_width;
        }

        // draw interior decorations
        let stroke = Stroke { width: 1., color: self.ast.theme.bg().neutral_tertiary };
        if !self.is_header_row {
            ui.painter()
                .hline(Rangef::new(top_left.x, top_left.x + width), top_left.y, stroke);
        }
        for child_idx in 1..self.ast.children.len() {
            ui.painter().vline(
                top_left.x + child_idx as f32 * child_width,
                Rangef::new(top_left.y, top_left.y + self.height(width, ui.ctx())),
                stroke,
            );
        }
    }

    fn height(&self, width: f32, ctx: &Context) -> f32 {
        // the height of the row is the height of the tallest cell
        let child_width = width / self.ast.children.len() as f32;
        let mut cell_height_max = 0.0f32;
        for cell in &self.ast.children {
            cell_height_max = cell_height_max.max(cell.height(child_width, ctx));
        }

        cell_height_max
    }
}
