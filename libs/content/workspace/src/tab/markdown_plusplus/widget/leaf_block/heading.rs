use comrak::nodes::{AstNode, NodeHeading};
use egui::{Context, FontId, Pos2, Rect, Stroke, TextFormat, Ui, Vec2};

use crate::tab::markdown_plusplus::{
    widget::{Ast, Block, WrapContext, ROW_HEIGHT},
    MarkdownPlusPlus,
};

pub struct Heading<'a, 't, 'w> {
    ast: &'w Ast<'a, 't>,
    node: &'w NodeHeading,
}

impl MarkdownPlusPlus {
    pub fn text_format_heading(&self, parent: &AstNode<'_>, node: &NodeHeading) -> TextFormat {
        let parent_text_format = self.text_format(parent);
        TextFormat {
            font_id: FontId {
                size: match node.level {
                    6 => 16.,
                    5 => 19.,
                    4 => 22.,
                    3 => 25.,
                    2 => 28.,
                    _ => 32.,
                },
                ..parent_text_format.font_id
            },
            ..parent_text_format
        }
    }
}

impl<'a, 't, 'w> Heading<'a, 't, 'w> {
    pub fn new(ast: &'w Ast<'a, 't>, node: &'w NodeHeading) -> Self {
        Self { ast, node }
    }

    pub fn text_format(node_heading: &NodeHeading, parent_text_format: TextFormat) -> TextFormat {
        TextFormat {
            font_id: FontId {
                size: match node_heading.level {
                    6 => 16.,
                    5 => 19.,
                    4 => 22.,
                    3 => 25.,
                    2 => 28.,
                    _ => 32.,
                },
                ..parent_text_format.font_id
            },
            ..parent_text_format
        }
    }
}

impl Block for Heading<'_, '_, '_> {
    fn show(&self, width: f32, mut top_left: Pos2, ui: &mut Ui) {
        self.ast
            .show_inline_children(&mut WrapContext::new(width), &mut top_left, ui);

        if self.node.level == 1 {
            let line_break_rect = Rect::from_min_size(top_left, Vec2::new(width, ROW_HEIGHT));

            ui.painter().hline(
                line_break_rect.x_range(),
                line_break_rect.center().y,
                Stroke { width: 1.0, color: self.ast.theme.bg().neutral_tertiary },
            );
        }
    }

    fn height(&self, width: f32, ctx: &Context) -> f32 {
        self.ast.inline_children_height(width, ctx)
            + if self.node.level == 1 { ROW_HEIGHT } else { 0. }
    }
}
