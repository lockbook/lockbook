use crate::tab::markdown_plusplus::{widget::WrapContext, MarkdownPlusPlus};

impl MarkdownPlusPlus {
    pub fn span_line_break(&self, wrap: &WrapContext) -> f32 {
        wrap.line_remaining()
    }

    pub fn show_line_break(&self, wrap: &mut WrapContext) {
        wrap.offset = wrap.line_end();
    }
}
