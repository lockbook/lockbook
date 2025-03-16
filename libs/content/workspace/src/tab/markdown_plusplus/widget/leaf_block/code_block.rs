use comrak::nodes::AstNode;
use egui::{FontFamily, FontId, Pos2, Rect, Sense, Stroke, TextFormat, Ui, Vec2};

use crate::tab::markdown_plusplus::{
    widget::{WrapContext, BLOCK_PADDING, ROW_HEIGHT},
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

    pub fn height_code_block(&self, node: &'ast AstNode<'ast>, width: f32, code: &str) -> f32 {
        let code = trim_one_trailing_newline(code);
        let text_width = width - 2. * BLOCK_PADDING;

        let info_height = ROW_HEIGHT;
        let code_height = self.inline_text_height(node, &WrapContext::new(text_width), code);
        BLOCK_PADDING + info_height + BLOCK_PADDING + BLOCK_PADDING + code_height + BLOCK_PADDING
    }

    pub fn show_code_block(
        &self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2, width: f32, code: &str,
        info: &str,
    ) {
        let code = trim_one_trailing_newline(code);
        let text_width = width - 2. * BLOCK_PADDING;

        let info_height = ROW_HEIGHT;
        let code_height =
            self.inline_text_height(node, &WrapContext::new(width - 2. * BLOCK_PADDING), code);
        let height = BLOCK_PADDING
            + info_height
            + BLOCK_PADDING
            + BLOCK_PADDING
            + code_height
            + BLOCK_PADDING;

        // full border
        let rect = Rect::from_min_size(top_left, Vec2::new(width, height));
        ui.painter()
            .rect_stroke(rect, 2., Stroke::new(1., self.theme.bg().neutral_tertiary));

        // info rect
        let info_rect = Rect::from_min_size(
            top_left,
            Vec2::new(width, BLOCK_PADDING + info_height + BLOCK_PADDING),
        );
        ui.painter().rect(
            info_rect,
            2.,
            self.theme.bg().neutral_secondary,
            Stroke::new(1., self.theme.bg().neutral_tertiary),
        );

        // copy button
        let copy_button_size = ROW_HEIGHT;
        let copy_button_rect = Rect::from_min_size(
            top_left + Vec2::new(text_width - copy_button_size, BLOCK_PADDING),
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

        // info text
        let info_top_left = top_left + Vec2::splat(BLOCK_PADDING);
        self.show_text(ui, node, info_top_left, &mut WrapContext::new(text_width), info);

        // code text
        let code_top_left = top_left
            + Vec2::new(BLOCK_PADDING, BLOCK_PADDING + info_height + BLOCK_PADDING + BLOCK_PADDING);
        self.show_text(ui, node, code_top_left, &mut WrapContext::new(text_width), code);
    }
}

fn trim_one_trailing_newline(code: &str) -> &str {
    code.strip_suffix("\r\n")
        .or_else(|| code.strip_suffix('\n'))
        .unwrap_or(code)
}
