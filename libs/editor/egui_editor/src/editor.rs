use crate::appearance::Appearance;
use crate::ast::Ast;
use crate::buffer::Buffer;
use crate::debug::DebugInfo;
use crate::galleys::Galleys;
use crate::layouts::Layouts;
use crate::styles::StyleInfo;
use crate::test_input::TEST_MARKDOWN;
use crate::{ast, events, galleys, layouts, register_fonts, styles};
use egui::{Context, FontDefinitions, Ui, Vec2};

pub struct Editor {
    pub initialized: bool,

    // config
    pub appearance: Appearance,

    // state
    pub buffer: Buffer,
    pub debug: DebugInfo,

    // cached intermediate state
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
            debug: Default::default(),

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
        let ui_size = ui.max_rect().size();
        self.debug.frame_start();

        // update theme
        let theme_updated = self.appearance.set_theme(ui.visuals());

        // process events
        let (text_updated, cursor_pos_updated, selection_updated) = if self.initialized {
            let prior_cursor_pos = self.buffer.current.cursor.pos;
            let prior_selection = self.buffer.current.cursor.selection();
            let (text_updated, maybe_to_clipboard) = events::process(
                &ui.ctx().input().events,
                &self.layouts,
                &self.galleys,
                &self.appearance,
                ui_size,
                &mut self.buffer,
                &mut self.debug,
            );
            let cursor_pos_updated = self.buffer.current.cursor.pos != prior_cursor_pos;
            let selection_updated = self.buffer.current.cursor.selection() != prior_selection;

            // put cut or copied text in clipboard
            if let Some(to_clipboard) = maybe_to_clipboard {
                ui.output().copied_text = to_clipboard;
            }
            (text_updated, cursor_pos_updated, selection_updated)
        } else {
            (true, true, true)
        };

        // recalculate dependent state
        if text_updated {
            self.ast = ast::calc(&self.buffer.current);
        }
        if text_updated || selection_updated || theme_updated {
            self.styles = styles::calc(
                &self.ast,
                &self
                    .buffer
                    .current
                    .cursor
                    .selection_bytes(&self.buffer.current.segs),
            );
            self.layouts = layouts::calc(&self.buffer.current, &self.styles, &self.appearance);
        }
        self.galleys = galleys::calc(&self.layouts, &self.appearance, ui);

        self.initialized = true;

        // draw
        self.draw_text(ui_size, ui);
        self.draw_cursor(ui);
        if self.debug.draw_enabled {
            self.draw_debug(ui);
        }

        // scroll
        if cursor_pos_updated {
            ui.scroll_to_rect(
                self.buffer
                    .current
                    .cursor
                    .rect(&self.buffer.current.segs, &self.galleys),
                None,
            );
        }
    }

    pub fn set_text(&mut self, new_text: String) {
        self.buffer = new_text.as_str().into();
        self.initialized = false;
    }

    pub fn set_font(&self, ctx: &Context) {
        let mut fonts = FontDefinitions::default();
        register_fonts(&mut fonts);
        ctx.set_fonts(fonts);
    }
}
