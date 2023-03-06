use std::time::Instant;

use egui::{Context, FontDefinitions, Response, Ui, Vec2};

use crate::appearance::Appearance;
use crate::ast::Ast;
use crate::buffer::{Buffer, SubModification};
use crate::cursor::Cursor;
use crate::debug::DebugInfo;
use crate::element::ItemType;
use crate::events::{self, pos_to_char_offset};
use crate::galleys::Galleys;
use crate::images::ImageCache;
use crate::layouts::{Annotation, Layouts};
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
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.spacing_mut().item_spacing = Vec2::ZERO;
                self.ui(ui);
            });
        });
    }

    pub fn ui(&mut self, ui: &mut Ui) -> Response {
        let id = ui.auto_id_with("lbeditor");
        let rect = ui.available_rect_before_wrap();
        let ui_size = rect.size();

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
        self.draw_text(ui_size, ui);
        let resp = ui.interact(rect, id, egui::Sense::click_and_drag());
        if let Some(pos) = resp.interact_pointer_pos() {
            ui.memory().request_focus(id);
            ui.memory().lock_focus(id, true);
            self.process_pointer_click(ui_size, &pos, ui);

            // self.layouts = layouts::calc(&self.buffer.current, &self.styles, &self.appearance);
            // self.galleys = galleys::calc(&self.layouts, &self.images, &self.appearance, ui);
        } else if resp.clicked_elsewhere() {
            ui.memory().surrender_focus(id);
        }

        if ui.memory().has_focus(id) {
            self.draw_cursor(ui);
        }

        if self.debug.draw_enabled {
            self.draw_debug(ui);
        }

        // scroll
        /*if cursor_pos_updated {
            ui.scroll_to_rect(
                self.buffer
                    .current
                    .cursor
                    .rect(&self.buffer.current.segs, &self.galleys),
                None,
            );
        }*/

        resp
    }

    fn process_pointer_click(&mut self, ui_size: egui::Vec2, pos: &egui::Pos2, ui: &mut egui::Ui) {
        let modifiers = ui.ctx().input().modifiers;
        let galleys = &self.galleys;
        let appearance = &self.appearance;
        let buffer = &mut self.buffer;
        let mut modifications = Vec::new();
        let mut cursor = buffer.current.cursor;
        let mut previous_cursor = buffer.current.cursor;

        /*cursor.fix(false, &buffer.current.segs, galleys);
        if cursor != previous_cursor {
            modifications.push(SubModification::Cursor { cursor });
            previous_cursor = cursor;
        }*/

        // do not process scrollbar clicks
        if pos.x <= ui_size.x {
            // process checkbox clicks
            let checkbox_click = {
                let mut checkbox_click = false;
                for galley in &galleys.galleys {
                    if let Some(Annotation::Item(ItemType::Todo(checked), ..)) = galley.annotation {
                        if galley.checkbox_bounds(appearance).contains(*pos) {
                            modifications.push(SubModification::Cursor {
                                cursor: Cursor {
                                    pos: buffer
                                        .current
                                        .segs
                                        .byte_offset_to_char(galley.range.start + galley.head_size),
                                    selection_origin: Some(
                                        buffer.current.segs.byte_offset_to_char(
                                            galley.range.start + galley.head_size - 6,
                                        ),
                                    ),
                                    ..Default::default()
                                },
                            });
                            modifications.push(SubModification::Insert {
                                text: if checked { "- [ ] " } else { "- [x] " }.to_string(),
                            });
                            modifications.push(SubModification::Cursor { cursor });

                            checkbox_click = true;
                            break;
                        }
                    }
                }
                checkbox_click
            };
            if !checkbox_click {
                // record instant for double/triple click
                cursor.process_click_instant(Instant::now());

                let mut double_click = false;
                let mut triple_click = false;
                if !modifiers.shift {
                    // click: end selection
                    cursor.selection_origin = None;

                    double_click = cursor.double_click();
                    triple_click = cursor.triple_click();
                } else {
                    // shift+click: begin selection
                    cursor.set_selection_origin();
                }
                // any click: begin drag; update cursor
                cursor.set_click_and_drag_origin();
                if triple_click {
                    cursor.pos = pos_to_char_offset(*pos, galleys, &buffer.current.segs);

                    let (galley_idx, cur_cursor) =
                        galleys.galley_and_cursor_by_char_offset(cursor.pos, &buffer.current.segs);
                    let galley = &galleys[galley_idx];
                    let begin_of_row_cursor = galley.galley.cursor_begin_of_row(&cur_cursor);
                    let end_of_row_cursor = galley.galley.cursor_end_of_row(&cur_cursor);

                    cursor.selection_origin = Some(galleys.char_offset_by_galley_and_cursor(
                        galley_idx,
                        &begin_of_row_cursor,
                        &buffer.current.segs,
                    ));
                    cursor.pos = galleys.char_offset_by_galley_and_cursor(
                        galley_idx,
                        &end_of_row_cursor,
                        &buffer.current.segs,
                    );
                } else if double_click {
                    cursor.pos = pos_to_char_offset(*pos, galleys, &buffer.current.segs);

                    cursor.advance_word(false, &buffer.current, &buffer.current.segs, galleys);
                    let end_of_word_pos = cursor.pos;
                    cursor.advance_word(true, &buffer.current, &buffer.current.segs, galleys);
                    let begin_of_word_pos = cursor.pos;

                    cursor.selection_origin = Some(begin_of_word_pos);
                    cursor.pos = end_of_word_pos;
                } else {
                    cursor.pos = pos_to_char_offset(*pos, galleys, &buffer.current.segs);
                }
            }
        }
        if cursor != previous_cursor {
            modifications.push(SubModification::Cursor { cursor });
            // previous_cursor = cursor;
            // buffer.current.cursor = cursor;
        }
        buffer.apply(modifications, &mut self.debug);
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
