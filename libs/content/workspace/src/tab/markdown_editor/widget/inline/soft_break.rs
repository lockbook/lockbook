use crate::tab::markdown_editor::widget::inline::Response;
use crate::tab::markdown_editor::widget::utils::text_layout::Wrap;
use crate::tab::markdown_editor::Editor;

impl Editor {
    pub fn span_soft_break(&self, wrap: &Wrap) -> f32 {
        self.span_line_break(wrap)
    }

    pub fn show_soft_break(&mut self, wrap: &mut Wrap) -> Response {
        self.show_line_break(wrap)
    }
}
