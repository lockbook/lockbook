use comrak::nodes::NodeWikiLink;
use egui::{Context, Pos2, Stroke, TextFormat, Ui};

use crate::tab::markdown_plusplus::{
    theme::Theme,
    widget::{Ast, Inline, WrapContext},
};

pub struct WikiLink<'a, 't, 'w> {
    ast: &'w Ast<'a, 't>,
    node: &'w NodeWikiLink,
}

impl<'a, 't, 'w> WikiLink<'a, 't, 'w> {
    pub fn new(ast: &'w Ast<'a, 't>, node: &'w NodeWikiLink) -> Self {
        Self { ast, node }
    }

    pub fn text_format(theme: &Theme, parent_text_format: TextFormat, ctx: &Context) -> TextFormat {
        TextFormat {
            color: theme.fg().blue,
            underline: Stroke { width: 1., color: theme.fg().blue },
            ..parent_text_format
        }
    }
}

impl Inline for WikiLink<'_, '_, '_> {
    fn show(&self, wrap: &mut WrapContext, mut top_left: Pos2, ui: &mut Ui) {
        self.ast.show_inline_children(wrap, &mut top_left, ui)
    }

    fn span(&self, wrap: &WrapContext, ctx: &Context) -> f32 {
        self.ast.inline_children_span(wrap, ctx)
    }
}
