use crate::widgets::{toolbar::MOBILE_TOOL_BAR_SIZE, ToolBar, ToolBarVisibility};
pub use editor::{Editor, EditorResponse};
use egui::{FontData, FontDefinitions, FontFamily};
use lb_rs::Uuid;
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

pub struct Markdown {
    pub editor: Editor,
    pub toolbar: ToolBar,
}

impl Markdown {
    // todo: you eleminated the idea of an auto rename signal here, evaluate what to do with it
    pub fn new(
        core: lb_rs::Core, bytes: &[u8], toolbar_visibility: &ToolBarVisibility, needs_name: bool,
        file_id: Uuid, plaintext_mode: bool,
    ) -> Self {
        let content = String::from_utf8_lossy(bytes);
        let editor = Editor::new(core, file_id, &content, &file_id, needs_name, plaintext_mode);
        let toolbar = ToolBar::new(toolbar_visibility);

        Self { editor, toolbar }
    }

    pub fn past_first_frame(&self) -> bool {
        self.editor.debug.frame_count > 1
    }

    pub fn show(&mut self, ui: &mut egui::Ui) -> EditorResponse {
        ui.vertical(|ui| {
            let mut res = if cfg!(target_os = "ios") || cfg!(target_os = "android") {
                ui.allocate_ui(
                    egui::vec2(ui.available_width(), ui.available_height() - MOBILE_TOOL_BAR_SIZE),
                    |ui| self.editor.scroll_ui(ui),
                )
                .inner
            } else {
                self.editor.scroll_ui(ui)
            };
            self.toolbar.show(ui, &mut self.editor, &mut res);
            res
        })
        .inner
    }
}
