use egui::{Context, FontFamily, FontId, Pos2, TextFormat, Ui};

use crate::tab::markdown_plusplus::{
    theme::Theme,
    widget::{Ast, Inline, WrapContext},
};

pub struct HtmlInline<'a, 't, 'w> {
    ast: &'w Ast<'a, 't>,
    html: &'w String,
}

impl<'a, 't, 'w> HtmlInline<'a, 't, 'w> {
    pub fn new(ast: &'w Ast<'a, 't>, html: &'w String) -> Self {
        Self { ast, html }
    }

    // rendered as code
    pub fn text_format(theme: &Theme, parent_text_format: TextFormat, ctx: &Context) -> TextFormat {
        TextFormat {
            color: theme.fg().accent_primary,
            font_id: FontId {
                size: parent_text_format.font_id.size * 0.9,
                family: FontFamily::Monospace,
            },
            background: theme.bg().neutral_secondary,
            ..parent_text_format
        }
    }
}

impl Inline for HtmlInline<'_, '_, '_> {
    fn show(&self, wrap: &mut WrapContext, top_left: Pos2, ui: &mut Ui) {
        self.ast.show_text(wrap, top_left, ui, self.html.clone());
    }

    fn span(&self, wrap: &WrapContext, ctx: &Context) -> f32 {
        self.ast.text_span(wrap, ctx, self.html.clone())
    }
}
