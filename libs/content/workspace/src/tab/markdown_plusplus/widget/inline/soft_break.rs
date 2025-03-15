use crate::tab::markdown_plusplus::{widget::WrapContext, MarkdownPlusPlus};

impl MarkdownPlusPlus {
    pub fn inline_span_soft_break(&self, wrap: &WrapContext) -> f32 {
        self.inline_span_line_break(wrap)
    }

    pub fn show_soft_break(&self, wrap: &mut WrapContext) {
        self.show_line_break(wrap);
    }
}
