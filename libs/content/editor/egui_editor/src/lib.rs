pub use crate::editor::{Editor, EditorResponse};
use egui::{FontData, FontDefinitions, FontFamily, Pos2, Rect};

use std::sync::Arc;

pub mod appearance;
pub mod ast;
pub mod bounds;
pub mod buffer;
pub mod debug;
pub mod draw;
pub mod editor;
pub mod galleys;
pub mod images;
pub mod input;
pub mod layouts;
pub mod offset_types;
pub mod style;
pub mod test_input;
pub mod unicode_segs;

pub fn register_fonts(fonts: &mut FontDefinitions) {
    fonts
        .font_data
        .insert("pt_sans".to_string(), FontData::from_static(lb_fonts::PT_SANS_REGULAR));
    fonts
        .font_data
        .insert("pt_mono".to_string(), FontData::from_static(lb_fonts::PT_MONO_REGULAR));
    fonts
        .font_data
        .insert("pt_bold".to_string(), FontData::from_static(lb_fonts::PT_SANS_BOLD));
    fonts.font_data.insert("material_icons".to_owned(), {
        let mut font = egui::FontData::from_static(lb_fonts::MATERIAL_ICONS_OUTLINED_REGULAR);
        font.tweak.y_offset_factor = -0.1;
        font
    });

    fonts
        .families
        .insert(FontFamily::Name(Arc::from("Bold")), vec!["pt_bold".to_string()]);

    fonts
        .families
        .get_mut(&FontFamily::Proportional)
        .unwrap()
        .insert(0, "pt_sans".to_string());

    fonts
        .families
        .get_mut(&FontFamily::Monospace)
        .unwrap()
        .insert(0, "pt_mono".to_string());

    fonts
        .families
        .get_mut(&egui::FontFamily::Monospace)
        .unwrap()
        .push("material_icons".to_owned());
}
