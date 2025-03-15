use comrak::nodes::AstNode;
use egui::{FontFamily, FontId, Pos2, Rect, Sense, Stroke, TextFormat, Ui, Vec2};

use crate::tab::markdown_plusplus::{
    widget::{WrapContext, BLOCK_SPACING, INLINE_PADDING, ROW_HEIGHT},
    MarkdownPlusPlus,
};

impl<'ast> MarkdownPlusPlus {
    pub fn text_format_code_block(&self, parent: &AstNode<'_>) -> TextFormat {
        let parent_text_format = self.text_format(parent);
        let parent_row_height = self
            .ctx
            .fonts(|fonts| fonts.row_height(&parent_text_format.font_id));
        let monospace_row_height = self.ctx.fonts(|fonts| {
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

    pub(crate) fn show_code_block(
        &self, ui: &mut Ui, node: &'ast AstNode<'ast>, mut top_left: Pos2, width: f32, code: &str,
        info: &str,
    ) {
        let wrap = WrapContext::new(width);

        let text_width = wrap.width - 2. * INLINE_PADDING;

        let info_height = self.height_code_block_info(node, wrap.width, info);
        let height = self.height_code_block(node, wrap.width, code, info);
        let rect = Rect::from_min_size(top_left, Vec2::new(wrap.width, height));

        ui.painter().rect(
            rect.expand2(Vec2::new(INLINE_PADDING, 1.)),
            2.,
            self.theme.bg().neutral_primary,
            Stroke::new(1., self.theme.bg().neutral_tertiary),
        );

        top_left.y += BLOCK_SPACING;

        let info_rect = Rect::from_min_size(top_left, Vec2::new(wrap.width, info_height));
        ui.painter().rect(
            info_rect.expand2(Vec2::new(INLINE_PADDING, BLOCK_SPACING + 1.)),
            2.,
            self.theme.bg().neutral_secondary,
            Stroke::new(1., self.theme.bg().neutral_tertiary),
        );

        let copy_button_size = self.row_height(node);
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
            ui.output_mut(|o| o.copied_text = code.into());
        }

        self.show_text(
            ui,
            node,
            top_left + Vec2::new(INLINE_PADDING, 0.),
            &mut WrapContext::new(text_width),
            info,
        );

        top_left.y += info_height;
        top_left.y += BLOCK_SPACING;
        top_left.y += self.row_height(node);

        self.show_text(
            ui,
            node,
            top_left + Vec2::new(INLINE_PADDING, 0.),
            &mut WrapContext::new(text_width),
            code,
        );
    }

    pub fn height_code_block(&self, node: &AstNode<'_>, width: f32, code: &str, info: &str) -> f32 {
        let code_height =
            self.inline_text_height(node, &WrapContext::new(width - 2. * INLINE_PADDING), code);
        let info_height = self.height_code_block_info(node, width, info);
        BLOCK_SPACING + info_height + BLOCK_SPACING + ROW_HEIGHT + code_height + ROW_HEIGHT
    }

    pub fn height_code_block_info(&self, node: &AstNode<'_>, width: f32, info: &str) -> f32 {
        if info.is_empty() {
            ROW_HEIGHT
        } else {
            self.inline_text_height(node, &WrapContext::new(width - 2. * INLINE_PADDING), info)
        }
    }
}
