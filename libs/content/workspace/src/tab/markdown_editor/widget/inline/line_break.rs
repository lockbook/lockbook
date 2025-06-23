use crate::tab::markdown_editor::widget::inline::Response;
use crate::tab::markdown_editor::widget::utils::text_layout::Wrap;
use crate::tab::markdown_editor::Editor;

impl Editor {
    pub fn span_line_break(&self, wrap: &Wrap) -> f32 {
        wrap.row_remaining()
    }

    pub fn show_line_break(&mut self, wrap: &mut Wrap) -> Response {
        wrap.offset = wrap.row_end();
        Response { clicked: false, hovered: false }
    }
}
