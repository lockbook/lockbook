pub use crate::editor::{Editor, EditorResponse};
use egui::{FontData, FontDefinitions, FontFamily};
use std::sync::Arc;

pub mod appearance;
pub mod apple_model;
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
    fonts.font_data.insert(
        "pt_sans".to_string(),
        FontData::from_static(include_bytes!("../fonts/PTSans-Regular.ttf")),
    );
    fonts.font_data.insert(
        "pt_mono".to_string(),
        FontData::from_static(include_bytes!("../fonts/PTMono-Regular.ttf")),
    );
    fonts.font_data.insert(
        "pt_bold".to_string(),
        FontData::from_static(include_bytes!("../fonts/PTSans-Bold.ttf")),
    );

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
}
