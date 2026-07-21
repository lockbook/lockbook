pub mod affine_scroll;
pub mod button;
pub mod button_group;
pub mod floating;
pub mod glyphon_cache;
pub mod glyphon_label;
pub mod glyphon_render;
pub mod glyphon_text_edit;
pub mod icon_button;
pub mod image_cache;
pub mod phosphor_icon_button;
pub mod progress_bar;
pub mod separator;
pub mod subscription;
pub mod switch;
pub mod tab_cache;

pub use button::Button;
pub use button_group::ButtonGroup;
pub use floating::{
    FloatingChrome, MenuEntries, TipPlacement, is_menu_open, show_menu, show_text_menu, tip_lines,
    tip_text, tip_ui, tip_ui_rich,
};
pub use glyphon_label::{GlyphonLabel, ShapedLabel, TextOverflow};
pub use glyphon_text_edit::GlyphonTextEdit;
pub use icon_button::IconButton;
pub use phosphor_icon_button::PhosphorIconButton;
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
