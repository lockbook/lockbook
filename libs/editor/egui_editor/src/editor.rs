use crate::appearance::{Appearance, Theme};
use crate::ast::Ast;
use crate::buffer::Buffer;
use crate::cursor::Cursor;
use crate::debug::DebugInfo;
use crate::galleys::Galleys;
use crate::layouts::Layouts;
use crate::styles::StyleInfo;
use crate::test_input::TEST_MARKDOWN;
use crate::unicode_segs::UnicodeSegs;
use crate::{ast, events, galleys, layouts, styles, unicode_segs};
use egui::{Context, FontData, FontDefinitions, FontFamily, Ui, Vec2};
use std::sync::Arc;

pub struct Editor {
    pub initialized: bool,

    // config
    pub appearance: Appearance,

    // state
    pub buffer: Buffer,
    pub cursor: Cursor,
    pub debug: DebugInfo,

    // cached intermediate state
    pub segs: UnicodeSegs,
    pub ast: Ast,
    pub styles: Vec<StyleInfo>,
    pub layouts: Layouts,
    pub galleys: Galleys,
}

impl Default for Editor {
    fn default() -> Self {
        Self {
            initialized: Default::default(),

            appearance: Default::default(),

            buffer: TEST_MARKDOWN.into(),
            cursor: Default::default(),
            debug: Default::default(),

            segs: Default::default(),
            ast: Default::default(),
            styles: Default::default(),
            layouts: Default::default(),
            galleys: Default::default(),
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

        // update theme
        self.appearance.current_theme =
            if ui.visuals().dark_mode { Theme::Dark } else { Theme::Light };

        // process events
        let (text_updated, selection_updated) = if self.initialized {
            events::process(
                &ui.ctx().input().events,
                &self.layouts,
                &self.galleys,
                &mut self.buffer,
                &mut self.segs,
                &mut self.cursor,
                &mut self.debug,
            )
        } else {
            self.segs = unicode_segs::calc(&self.buffer);
            (true, true)
        };

        // recalculate dependent state
        if text_updated {
            self.ast = ast::calc(&self.buffer);
        }
        if text_updated || selection_updated {
            self.styles = styles::calc(&self.ast, &self.cursor.selection_bytes(&self.segs));
            self.layouts = layouts::calc(&self.buffer, &self.styles, &self.appearance);
            self.print_layouts();
        }
        self.galleys = galleys::calc(&self.layouts, &self.appearance, ui);

        self.initialized = true;

        // draw
        self.draw_text(ui);
        self.draw_cursor(ui);
        if self.debug.draw_enabled {
            self.draw_debug(ui);
        }
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
