use crate::ast::Ast;
use crate::cursor::Cursor;
use crate::cursor_types::DocByteOffset;
use crate::debug_layer::DebugLayer;
use crate::galley::GalleyInfo;
use crate::layout_job::LayoutJobInfo;
use crate::styled_chunk::StyledChunk;
use crate::test_input::TEST_MARKDOWN;
use crate::theme::VisualAppearance;
use egui::{Context, FontData, FontDefinitions, FontFamily, Ui, Vec2};
use std::sync::Arc;

pub struct Editor {
    pub raw: String,

    pub visual_appearance: VisualAppearance,

    pub debug: DebugLayer,
    pub text_unprocessed: bool,
    pub cursor_unprocessed: bool,
    pub gr_ind: Vec<DocByteOffset>,
    pub ast: Ast,
    pub styled: Vec<StyledChunk>,
    pub layout: Vec<LayoutJobInfo>,
    pub galleys: Vec<GalleyInfo>,
    pub cursor: Cursor,
}

impl Default for Editor {
    fn default() -> Self {
        Self {
            debug: DebugLayer::default(),

            raw: TEST_MARKDOWN.to_string(),
            text_unprocessed: true,
            cursor_unprocessed: true,
            gr_ind: vec![],
            ast: Ast::default(),
            styled: vec![],
            layout: vec![],
            galleys: vec![],
            cursor: Cursor::default(),
            visual_appearance: Default::default(),
        }
    }
}

impl Editor {
    pub fn draw(&mut self, ctx: &Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.spacing_mut().item_spacing = Vec2::ZERO;
                self.ui(ui);
            });
        });
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        self.debug.frame_start();

        self.visual_appearance.update(ui);
        self.key_events(ui);
        self.mouse_events(ui);

        if self.text_unprocessed {
            self.ast = Ast::parse(&self.raw);
            self.calc_unicode_segs();
        }
        if self.text_unprocessed || self.cursor_unprocessed {
            self.populate_styled();
            self.populate_layouts();
        }
        self.text_unprocessed = false;
        self.cursor_unprocessed = false;

        self.present_text(ui);
        self.draw_cursor(ui);
        self.debug_layer(ui);
    }

    pub fn set_font(&self, ctx: &Context) {
        let mut fonts = FontDefinitions::default();

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

        ctx.set_fonts(fonts);
    }
}
