use egui::{Context, FontDefinitions, Ui, Vec2};

use crate::appearance::Appearance;
use crate::ast::Ast;
use crate::buffer::Buffer;
use crate::debug::DebugInfo;
use crate::events;
use crate::galleys::Galleys;
use crate::images::ImageCache;
use crate::layouts::Layouts;
use crate::styles::StyleInfo;
use crate::test_input::TEST_MARKDOWN;
use crate::{ast, galleys, images, layouts, register_fonts, styles};

pub struct Editor {
    pub initialized: bool,

    // config
    pub appearance: Appearance,
    pub client: reqwest::blocking::Client, // todo: don't download images on the UI thread

    // state
    pub buffer: Buffer,
    pub debug: DebugInfo,
    pub images: ImageCache,

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
            client: Default::default(),

            buffer: TEST_MARKDOWN.into(),
            debug: Default::default(),
            images: Default::default(),

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
            self.scroll_ui(ui);
        });
    }

    pub fn scroll_ui(&mut self, ui: &mut Ui) {
        let id = ui.auto_id_with("lbeditor");
        if ui.memory().has_focus(id) {
            ui.memory().lock_focus(id, true);
        }

        let sao = egui::ScrollArea::vertical().show(ui, |ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            self.ui(ui, id);
        });
        let resp = ui.interact(sao.inner_rect, id, egui::Sense::click_and_drag());
        if let Some(pos) = resp.interact_pointer_pos() {
            if !ui.memory().has_focus(id) {
                events::process(
                    &[egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: Default::default(),
                    }],
                    &self.layouts,
                    &self.galleys,
                    &self.appearance,
                    sao.inner_rect.size(),
                    &mut self.buffer,
                    &mut self.debug,
                );
                ui.memory().request_focus(id);
                ui.memory().lock_focus(id, true);
            }
        } else if resp.clicked_elsewhere() {
            ui.memory().surrender_focus(id);
        }
    }

    pub fn ui(&mut self, ui: &mut Ui, id: egui::Id) {
        let ui_size = ui.available_rect_before_wrap().size();

        self.debug.frame_start();

        // update theme
        let theme_updated = self.appearance.set_theme(ui.visuals());

        // process events
        let (text_updated, cursor_pos_updated, selection_updated) = if self.initialized {
            let prior_cursor_pos = self.buffer.current.cursor.pos;
            let prior_selection = self.buffer.current.cursor.selection();
            let (text_updated, maybe_to_clipboard) = if ui.memory().has_focus(id) {
                events::process(
                    &ui.ctx().input().events,
                    &self.layouts,
                    &self.galleys,
                    &self.appearance,
                    ui_size,
                    &mut self.buffer,
                    &mut self.debug,
                )
            } else {
                (false, None)
            };
            let cursor_pos_updated = self.buffer.current.cursor.pos != prior_cursor_pos;
            let selection_updated = self.buffer.current.cursor.selection() != prior_selection;

            // put cut or copied text in clipboard
            if let Some(to_clipboard) = maybe_to_clipboard {
                ui.output().copied_text = to_clipboard;
            }
            (text_updated, cursor_pos_updated, selection_updated)
        } else {
            ui.memory().request_focus(id);
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
            self.images = images::calc(&self.layouts, &self.images, &self.client, ui);
        }
        self.galleys = galleys::calc(&self.layouts, &self.images, &self.appearance, ui);

        self.initialized = true;

        // draw
        // let rect = ui.available_rect_before_wrap();
        self.draw_text(ui_size, ui);

        if ui.memory().has_focus(id) {
            self.draw_cursor(ui);
        }

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
