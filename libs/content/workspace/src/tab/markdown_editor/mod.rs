use egui::{FontData, FontDefinitions, FontFamily};
use std::sync::Arc;

pub mod appearance;
pub mod ast;
pub mod bounds;
pub mod debug;
pub mod draw;
pub mod editor;
pub mod galleys;
pub mod grammar;
pub mod images;
pub mod input;
pub mod layouts;
pub mod output;
pub mod style;
pub mod test_input;
pub mod widgets;

pub use editor::{Editor, Response};
pub use input::Event;

pub fn register_fonts(fonts: &mut FontDefinitions) {
    let (pt_sans, pt_mono, pt_bold) = if cfg!(target_vendor = "apple") {
        (lb_fonts::SF_PRO_REGULAR, lb_fonts::SF_MONO_REGULAR, lb_fonts::SF_PRO_TEXT_BOLD)
    } else if cfg!(target_os = "android") {
        (lb_fonts::ROBOTO_REGULAR, lb_fonts::ROBOTO_MONO_REGULAR, lb_fonts::ROBOTO_BOLD)
    } else {
        (lb_fonts::PT_SANS_REGULAR, lb_fonts::PT_MONO_REGULAR, lb_fonts::PT_SANS_BOLD)
    };

    fonts
        .font_data
        .insert("pt_sans".to_string(), FontData::from_static(pt_sans));
    fonts
        .font_data
        .insert("pt_mono".to_string(), FontData::from_static(pt_mono));
    fonts
        .font_data
        .insert("pt_bold".to_string(), FontData::from_static(pt_bold));
    fonts.font_data.insert("material_icons".to_owned(), {
        let mut font = egui::FontData::from_static(lb_fonts::MATERIAL_SYMBOLS_OUTLINED);
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
