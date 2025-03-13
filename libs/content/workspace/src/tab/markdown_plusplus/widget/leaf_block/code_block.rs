use comrak::nodes::NodeCodeBlock;
use egui::{Context, FontFamily, FontId, Pos2, Rect, Sense, Stroke, TextFormat, Ui, Vec2};

use crate::tab::markdown_plusplus::{
    theme::Theme,
    widget::{Ast, Block, WrapContext, BLOCK_SPACING, INLINE_PADDING},
};

pub struct CodeBlock<'a, 't, 'w> {
    ast: &'w Ast<'a, 't>,
    node: &'w NodeCodeBlock,
}

impl<'a, 't, 'w> CodeBlock<'a, 't, 'w> {
    pub fn new(ast: &'w Ast<'a, 't>, node: &'w NodeCodeBlock) -> Self {
        Self { ast, node }
    }

    pub fn text_format(theme: &Theme, parent_text_format: TextFormat, ctx: &Context) -> TextFormat {
        let parent_row_height = ctx.fonts(|fonts| fonts.row_height(&parent_text_format.font_id));
        let monospace_row_height = ctx.fonts(|fonts| {
            fonts
                .row_height(&FontId { family: FontFamily::Monospace, ..parent_text_format.font_id })
        });
        let monospace_row_height_preserving_size =
            parent_text_format.font_id.size * parent_row_height / monospace_row_height;
        TextFormat {
            font_id: FontId {
                size: monospace_row_height_preserving_size,
                family: FontFamily::Monospace,
            },
            line_height: Some(parent_row_height),
            ..parent_text_format
        }
    }
}

impl Block for CodeBlock<'_, '_, '_> {
    fn show(&self, width: f32, top_left: Pos2, ui: &mut Ui) {
        self.ast.show_code_block(
            width,
            top_left,
            ui,
            self.node.literal.clone(),
            self.node.info.clone(),
        );
    }

    fn height(&self, width: f32, ctx: &Context) -> f32 {
        self.ast
            .code_block_height(width, ctx, self.node.literal.clone(), self.node.info.clone())
    }
}

impl Ast<'_, '_> {
    pub(crate) fn show_code_block(
        &self, width: f32, mut top_left: Pos2, ui: &mut Ui, code: String, info: String,
    ) {
        let text_width = width - 2. * INLINE_PADDING;

        let info_height = self.code_block_info_height(width, ui.ctx(), info.clone());
        let height = self.code_block_height(width, ui.ctx(), code.clone(), info.clone());
        let rect = Rect::from_min_size(top_left, Vec2::new(width, height));

        ui.painter().rect(
            rect.expand2(Vec2::new(INLINE_PADDING, 1.)),
            2.,
            self.theme.bg().neutral_primary,
            Stroke::new(1., self.theme.bg().neutral_tertiary),
        );

        top_left.y += BLOCK_SPACING;

        let info_rect = Rect::from_min_size(top_left, Vec2::new(width, info_height));
        ui.painter().rect(
            info_rect.expand2(Vec2::new(INLINE_PADDING, BLOCK_SPACING + 1.)),
            2.,
            self.theme.bg().neutral_secondary,
            Stroke::new(1., self.theme.bg().neutral_tertiary),
        );

        let copy_button_size = self.row_height(ui.ctx());
        let copy_button_rect = Rect::from_min_size(
            top_left + Vec2::new(text_width - copy_button_size, 0.),
            Vec2::new(copy_button_size, copy_button_size),
        );
        ui.painter().rect_stroke(
            copy_button_rect,
            2.,
            Stroke::new(1., self.theme.bg().neutral_tertiary),
        );
        if ui.allocate_rect(copy_button_rect, Sense::click()).clicked() {
            ui.output_mut(|o| o.copied_text = code.clone());
        }

        self.show_text(
            &mut WrapContext::new(text_width),
            top_left + Vec2::new(INLINE_PADDING, 0.),
            ui,
            info.clone(),
        );

        top_left.y += info_height;
        top_left.y += BLOCK_SPACING;
        top_left.y += self.row_height(ui.ctx());

        // let code = if code.ends_with("\r\n") {
        //     code[..code.len() - 2].into()
        // } else if code.ends_with('\n') {
        //     code[..code.len() - 1].into()
        // } else {
        //     code
        // };

        self.show_text(
            &mut WrapContext::new(text_width),
            top_left + Vec2::new(INLINE_PADDING, 0.),
            ui,
            code.clone(),
        );
    }

    pub(crate) fn code_block_height(
        &self, width: f32, ctx: &Context, code: String, info: String,
    ) -> f32 {
        let row_height = self.row_height(ctx);
        let code_height =
            self.inline_text_height(&WrapContext::new(width - 2. * INLINE_PADDING), ctx, code);
        let info_height = self.code_block_info_height(width, ctx, info);
        BLOCK_SPACING + info_height + BLOCK_SPACING + row_height + code_height + row_height
    }

    pub(crate) fn code_block_info_height(&self, width: f32, ctx: &Context, info: String) -> f32 {
        if info.is_empty() {
            self.row_height(ctx)
        } else {
            self.inline_text_height(&WrapContext::new(width - 2. * INLINE_PADDING), ctx, info)
        }
    }
}
