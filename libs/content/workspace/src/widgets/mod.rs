pub mod button;
pub mod button_group;
pub mod glyphon_cache;
pub mod glyphon_label;
pub mod glyphon_render;
pub mod glyphon_text_edit;
pub mod icon_button;
pub mod image_cache;
pub mod progress_bar;
pub mod separator;
pub mod subscription;
pub mod switch;
pub mod tab_cache;

pub use button::Button;
pub use button_group::ButtonGroup;
pub use glyphon_label::{GlyphonLabel, ShapedLabel};
pub use glyphon_text_edit::GlyphonTextEdit;
pub use icon_button::IconButton;
pub use progress_bar::ProgressBar;
pub use separator::separator;
pub use subscription::subscription;
pub use switch::switch;

pub trait UiExt {
    fn glyphon_text_edit(&mut self, text: &mut String) -> egui::Response;
}

impl UiExt for egui::Ui {
    fn glyphon_text_edit(&mut self, text: &mut String) -> egui::Response {
        self.add(GlyphonTextEdit::new(text))
    }
}
