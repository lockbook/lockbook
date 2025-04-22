use crate::tab::markdown_plusplus::{widget::Wrap, MarkdownPlusPlus};

impl MarkdownPlusPlus {
    pub fn span_line_break(&self, wrap: &Wrap) -> f32 {
        wrap.row_remaining()
    }

    pub fn show_line_break(&mut self, wrap: &mut Wrap) {
        wrap.offset = wrap.row_end();
    }
}
