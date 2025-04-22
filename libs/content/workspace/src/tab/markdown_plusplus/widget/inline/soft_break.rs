use crate::tab::markdown_plusplus::{widget::Wrap, MarkdownPlusPlus};

impl MarkdownPlusPlus {
    pub fn span_soft_break(&self, wrap: &Wrap) -> f32 {
        self.span_line_break(wrap)
    }

    pub fn show_soft_break(&mut self, wrap: &mut Wrap) {
        self.show_line_break(wrap);
    }
}
