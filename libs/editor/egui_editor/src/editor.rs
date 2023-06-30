use rand::Rng;
use std::mem;

use egui::os::OperatingSystem;
use egui::{Color32, Context, Event, FontDefinitions, Frame, Margin, Pos2, Rect, Sense, Ui, Vec2};

use crate::appearance::Appearance;
use crate::ast::Ast;
use crate::bounds::{Paragraphs, Words};
use crate::buffer::Buffer;
use crate::debug::DebugInfo;
use crate::element::{Element, ItemType};
use crate::galleys::Galleys;
use crate::images::ImageCache;
use crate::input::canonical::{Bound, Modification, Offset, Region};
use crate::input::click_checker::{ClickChecker, EditorClickChecker};
use crate::input::cursor::{Cursor, PointerState};
use crate::input::events;
use crate::layouts::Annotation;
use crate::offset_types::RangeExt;
use crate::test_input::TEST_MARKDOWN;
use crate::{ast, bounds, galleys, images, register_fonts};

#[repr(C)]
#[derive(Debug, Default)]
pub struct EditorResponse {
    pub text_updated: bool,

    pub show_edit_menu: bool,
    pub has_selection: bool,
    pub edit_menu_x: f32,
    pub edit_menu_y: f32,

    pub cursor_in_heading: bool,
    pub cursor_in_bullet_list: bool,
    pub cursor_in_number_list: bool,
    pub cursor_in_todo_list: bool,
    pub cursor_in_bold: bool,
    pub cursor_in_italic: bool,
    pub cursor_in_inline_code: bool,
}

pub struct Editor {
    pub id: u32,
    pub initialized: bool,

    // config
    pub appearance: Appearance,
    pub client: reqwest::blocking::Client, // todo: don't download images on the UI thread

    // state
    pub buffer: Buffer,
    pub pointer_state: PointerState, // state of cursor not subject to undo history
    pub debug: DebugInfo,
    pub images: ImageCache,

    // cached intermediate state
    pub ast: Ast,
    pub words: Words,
    pub paragraphs: Paragraphs,
    pub galleys: Galleys,

    // computed state from last frame
    pub ui_rect: Rect,

    // state computed from processing events but not yet incorporated into drawn frame
    pub maybe_to_clipboard: Option<String>,
    pub maybe_opened_url: Option<String>,
    pub text_updated: bool,
    pub selection_updated: bool,
    pub maybe_menu_location: Option<Pos2>,

    // events not supported by egui; integrations push to this vec and editor processes and clears it
    pub custom_events: Vec<Modification>,

    // state for detecting clicks and converting global to local coordinates
    pub scroll_area_rect: Rect,
    pub scroll_area_offset: Vec2,
}

impl Default for Editor {
    fn default() -> Self {
        let id: u32 = rand::thread_rng().gen();
        Self {
            id,
            initialized: Default::default(),

            appearance: Default::default(),
            client: Default::default(),

            buffer: TEST_MARKDOWN.into(),
            pointer_state: Default::default(),
            debug: Default::default(),
            images: Default::default(),

            ast: Default::default(),
            words: Default::default(),
            paragraphs: Default::default(),
            galleys: Default::default(),

            ui_rect: Rect { min: Default::default(), max: Default::default() },

            maybe_to_clipboard: Default::default(),
            maybe_opened_url: Default::default(),
            text_updated: Default::default(),
            selection_updated: Default::default(),
            maybe_menu_location: Default::default(),

            custom_events: Default::default(),

            scroll_area_rect: Rect { min: Default::default(), max: Default::default() },
            scroll_area_offset: Default::default(),
        }
    }
}

impl Editor {
    pub fn draw(&mut self, ctx: &Context) -> EditorResponse {
        let fill = if ctx.style().visuals.dark_mode { Color32::BLACK } else { Color32::WHITE };
        egui::CentralPanel::default()
            .frame(Frame::default().fill(fill))
            .show(ctx, |ui| self.scroll_ui(ui))
            .inner
    }

    pub fn scroll_ui(&mut self, ui: &mut Ui) -> EditorResponse {
        let touch_mode = matches!(ui.ctx().os(), OperatingSystem::Android | OperatingSystem::IOS);

        let events = ui.ctx().input(|i| i.events.clone());

        // create id (even though we don't use interact response)
        let id = ui.auto_id_with("lbeditor");
        ui.interact(self.scroll_area_rect, id, Sense::focusable_noninteractive());

        // calculate focus
        let mut request_focus = ui.memory(|m| m.has_focus(id));
        let mut surrender_focus = false;
        for event in &events {
            if let Event::PointerButton { pos, pressed: true, .. } = event {
                if ui.is_enabled() && self.scroll_area_rect.contains(*pos) {
                    request_focus = true;
                } else {
                    surrender_focus = true;
                }
            }
        }

        // show ui
        let mut focus = false;
        let sao = egui::ScrollArea::vertical()
            .drag_to_scroll(touch_mode)
            .id_source(self.id)
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing = Vec2::ZERO;

                // set focus
                if request_focus {
                    ui.memory_mut(|m| {
                        m.request_focus(id);
                    });
                }
                if surrender_focus {
                    ui.memory_mut(|m| m.surrender_focus(id));
                }
                ui.memory_mut(|m| {
                    if m.has_focus(id) {
                        focus = true;
                        m.lock_focus(id, true);
                    }
                });

                Frame::default()
                    .inner_margin(Margin::symmetric(
                        clamp(0.04, 0.1, 0.25, ui.max_rect().width()),
                        0.0,
                    ))
                    .show(ui, |ui| self.ui(ui, id, touch_mode, &events))
            });
        self.ui_rect = sao.inner_rect;

        // set focus again because egui clears it for our widget for some reason
        if focus {
            ui.memory_mut(|m| {
                m.request_focus(id);
                m.lock_focus(id, true);
            });
        }

        // remember scroll area rect for focus next frame
        self.scroll_area_rect = sao.inner_rect;
        self.scroll_area_offset = sao.state.offset;

        sao.inner.inner
    }

    fn ui(
        &mut self, ui: &mut Ui, id: egui::Id, touch_mode: bool, events: &[Event],
    ) -> EditorResponse {
        self.debug.frame_start();

        // update theme
        let theme_updated = self.appearance.set_theme(ui.visuals());

        // process events
        let (text_updated, selection_updated) = if self.initialized {
            if ui.memory(|m| m.has_focus(id)) {
                let custom_events = mem::take(&mut self.custom_events);
                self.process_events(events, &custom_events, touch_mode);
                if let Some(to_clipboard) = &self.maybe_to_clipboard {
                    ui.output_mut(|o| o.copied_text = to_clipboard.clone());
                }
                if let Some(opened_url) = &self.maybe_opened_url {
                    ui.output_mut(|o| {
                        o.open_url = Some(egui::output::OpenUrl::new_tab(opened_url))
                    });
                }
                (self.text_updated, self.selection_updated)
            } else {
                (false, false)
            }
        } else {
            ui.memory_mut(|m| m.request_focus(id));

            // put the cursor at the first valid cursor position
            self.custom_events.push(Modification::Select {
                region: Region::ToOffset {
                    offset: Offset::To(Bound::Doc),
                    backwards: false,
                    extend_selection: false,
                },
            });

            (true, true)
        };

        // recalculate dependent state
        if text_updated {
            self.ast = ast::calc(&self.buffer.current);
            self.words = bounds::calc_words(&self.buffer.current, &self.ast);
            self.paragraphs = bounds::calc_paragraphs(&self.buffer.current, &self.ast);
        }
        if text_updated || selection_updated || theme_updated {
            self.images = images::calc(&self.ast, &self.images, &self.client, ui);
        }
        self.galleys = galleys::calc(
            &self.ast,
            &self.buffer.current,
            &self.paragraphs,
            &self.images,
            &self.appearance,
            ui,
        );
        self.initialized = true;

        // draw
        self.draw_text(self.ui_rect.size(), ui);
        if ui.memory(|m| m.has_focus(id)) {
            self.draw_cursor(ui, touch_mode);
        }
        if self.debug.draw_enabled {
            self.draw_debug(ui);
        }

        // scroll
        let all_selection = {
            let mut select_all_cursor = Cursor::from(0);
            select_all_cursor.advance(
                Offset::To(Bound::Doc),
                true,
                &self.buffer.current,
                &self.galleys,
                &self.paragraphs,
            );
            let start = select_all_cursor.selection.1;
            select_all_cursor.advance(
                Offset::To(Bound::Doc),
                false,
                &self.buffer.current,
                &self.galleys,
                &self.paragraphs,
            );
            let end = select_all_cursor.selection.1;
            (start, end)
        };
        if selection_updated && self.buffer.current.cursor.selection != all_selection {
            ui.scroll_to_rect(self.buffer.current.cursor.end_rect(&self.galleys), None);
        }

        // determine cursor markup location
        let mut cursor_in_heading = false;
        let mut cursor_in_bullet_list = false;
        let mut cursor_in_number_list = false;
        let mut cursor_in_todo_list = false;
        let mut cursor_in_bold = false;
        let mut cursor_in_italic = false;
        let mut cursor_in_inline_code = false;

        let ast_node_idx = self
            .ast
            .ast_node_at_char(self.buffer.current.cursor.selection.start());
        let ast_node = &self.ast.nodes[ast_node_idx];

        match ast_node.element {
            Element::Heading(_) => cursor_in_heading = true,
            Element::InlineCode => cursor_in_inline_code = true,
            Element::Strong => cursor_in_bold = true,
            Element::Emphasis => cursor_in_italic = true,
            _ => {
                let galley_idx = self
                    .galleys
                    .galley_at_char(self.buffer.current.cursor.selection.start());
                let galley = &self.galleys.galleys[galley_idx];

                match galley.annotation {
                    Some(Annotation::Item(ItemType::Todo(_), ..)) => {
                        cursor_in_todo_list = true;
                    }
                    Some(Annotation::Item(ItemType::Numbered(_), ..)) => {
                        cursor_in_number_list = true;
                    }
                    Some(Annotation::Item(ItemType::Bulleted, ..)) => {
                        cursor_in_bullet_list = true;
                    }
                    _ => {}
                };
            }
        }

        EditorResponse {
            text_updated,
            show_edit_menu: self.maybe_menu_location.is_some(),
            has_selection: self.buffer.current.cursor.selection().is_some(),
            edit_menu_x: self.maybe_menu_location.map(|p| p.x).unwrap_or_default(),
            edit_menu_y: self.maybe_menu_location.map(|p| p.y).unwrap_or_default(),
            cursor_in_heading,
            cursor_in_bullet_list,
            cursor_in_number_list,
            cursor_in_todo_list,
            cursor_in_bold,
            cursor_in_italic,
            cursor_in_inline_code,
        }
    }

    pub fn process_events(
        &mut self, events: &[Event], custom_events: &[Modification], touch_mode: bool,
    ) {
        let prior_selection = self.buffer.current.cursor.selection;
        let click_checker = EditorClickChecker {
            ui_rect: self.ui_rect,
            galleys: &self.galleys,
            buffer: &self.buffer,
            ast: &self.ast,
            appearance: &self.appearance,
        };
        let combined_events = events::combine(
            events,
            custom_events,
            &click_checker,
            touch_mode,
            &mut self.pointer_state,
        );
        let (text_updated, maybe_to_clipboard, maybe_opened_url) = events::process(
            &combined_events,
            &self.galleys,
            &self.paragraphs,
            &self.ast,
            &mut self.buffer,
            &mut self.debug,
        );

        // in touch mode, check if we should open the menu
        let click_checker = EditorClickChecker {
            ui_rect: self.ui_rect,
            galleys: &self.galleys,
            buffer: &self.buffer,
            ast: &self.ast,
            appearance: &self.appearance,
        };
        if touch_mode {
            let current_cursor = self.buffer.current.cursor;
            let current_selection = current_cursor.selection;

            let touched_a_galley = events.iter().any(|e| {
                if let Event::Touch { pos, .. } | Event::PointerButton { pos, .. } = e {
                    (&click_checker).text(*pos).is_some()
                } else {
                    false
                }
            });

            let touched_cursor = current_selection.is_empty()
                && prior_selection == current_selection
                && touched_a_galley
                && combined_events
                    .iter()
                    .any(|e| matches!(e, Modification::Select { region: Region::Location(..) }));

            let touched_selection = current_selection.is_empty()
                && prior_selection.contains(current_selection.1)
                && touched_a_galley
                && combined_events
                    .iter()
                    .any(|e| matches!(e, Modification::Select { region: Region::Location(..) }));

            let double_touched_for_selection = !current_selection.is_empty()
                && touched_a_galley
                && combined_events.iter().any(|e| {
                    matches!(
                        e,
                        Modification::Select { region: Region::BoundAt { bound: Bound::Word, .. } }
                    )
                });

            if touched_cursor || touched_selection || double_touched_for_selection {
                // set menu location
                self.maybe_menu_location =
                    Some(self.buffer.current.cursor.end_rect(&self.galleys).min);
            } else {
                self.maybe_menu_location = None;
            }
            if touched_cursor || touched_selection {
                // put the cursor back the way it was
                self.buffer.current.cursor.selection = prior_selection;
            }
        }

        // put cut or copied text in clipboard
        self.maybe_to_clipboard = maybe_to_clipboard;
        self.maybe_opened_url = maybe_opened_url;
        self.text_updated = text_updated;
        self.selection_updated = self.buffer.current.cursor.selection != prior_selection;
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

fn clamp(min: f32, mid: f32, max: f32, viewport_width: f32) -> f32 {
    if viewport_width > 800.0 {
        return viewport_width * max;
    } else if viewport_width < 400.0 {
        return viewport_width * min;
    }
    viewport_width * mid
}
