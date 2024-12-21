use egui::os::OperatingSystem;
use egui::{
    scroll_area, Color32, Context, EventFilter, Frame, Id, Margin, Rect, ScrollArea, Stroke, Ui,
};

use lb_rs::blocking::Lb;
use lb_rs::model::file_metadata::DocumentHmac;
use lb_rs::text::buffer::Buffer;
use lb_rs::text::offset_types::{DocCharOffset, RangeExt as _};
use lb_rs::Uuid;

use crate::tab::markdown_editor;
use crate::tab::ExtendedInput as _;
use markdown_editor::appearance::Appearance;
use markdown_editor::ast::{Ast, AstTextRangeType};
use markdown_editor::bounds::BoundExt as _;
use markdown_editor::bounds::Bounds;
use markdown_editor::debug::DebugInfo;
use markdown_editor::galleys::Galleys;
use markdown_editor::images::ImageCache;
use markdown_editor::input::capture::CaptureState;
use markdown_editor::input::cursor;
use markdown_editor::input::cursor::CursorState;
use markdown_editor::input::mutation::EventState;
use markdown_editor::input::Bound;
use markdown_editor::widgets::find::Find;
use markdown_editor::widgets::toolbar::{Toolbar, MOBILE_TOOL_BAR_SIZE};
use markdown_editor::Event;
use markdown_editor::{ast, bounds, galleys, images};

use serde::Serialize;
use std::time::{Duration, Instant};

#[derive(Debug, Serialize, Default)]
pub struct Response {
    // state changes
    pub text_updated: bool,
    pub cursor_screen_postition_updated: bool,
    pub scroll_updated: bool,

    // actions taken
    pub suggest_rename: Option<String>,
}

pub struct Editor {
    // dependencies
    pub core: Lb,
    pub client: reqwest::blocking::Client,

    // input
    pub file_id: Uuid,
    pub hmac: Option<DocumentHmac>,
    pub needs_name: bool,
    pub initialized: bool,
    pub appearance: Appearance,

    // internal systems
    pub buffer: Buffer,
    pub cursor: CursorState,
    pub debug: DebugInfo,
    pub images: ImageCache,
    pub ast: Ast,
    pub bounds: Bounds,
    pub galleys: Galleys,
    pub capture: CaptureState,
    pub toolbar: Toolbar,
    pub find: Find,
    pub event: EventState,

    pub virtual_keyboard_shown: bool,
}

impl Editor {
    pub fn new(
        core: Lb, content: &str, file_id: Uuid, hmac: Option<DocumentHmac>, needs_name: bool,
        plaintext_mode: bool,
    ) -> Self {
        Self {
            core,
            client: Default::default(),

            file_id,
            hmac,
            needs_name,
            initialized: false,
            appearance: Appearance { plaintext_mode, ..Default::default() },

            buffer: content.into(),
            cursor: Default::default(),
            debug: Default::default(),
            images: Default::default(),
            ast: Default::default(),
            bounds: Default::default(),
            galleys: Default::default(),
            capture: Default::default(),
            toolbar: Default::default(),
            find: Default::default(),
            event: Default::default(),

            virtual_keyboard_shown: false,
        }
    }

    pub fn past_first_frame(&self) -> bool {
        self.debug.frame_count > 1
    }

    pub fn reload(&mut self, text: String) {
        self.buffer.reload(text)
    }

    pub fn id(&self) -> Id {
        Id::new(self.file_id)
    }

    pub fn focus(&mut self, ctx: &Context) {
        ctx.memory_mut(|m| {
            m.request_focus(self.id());
        });
    }

    pub fn focus_lock(&mut self, ctx: &Context) {
        ctx.memory_mut(|m| {
            m.set_focus_lock_filter(
                self.id(),
                EventFilter {
                    tab: true,
                    horizontal_arrows: true,
                    vertical_arrows: true,
                    escape: true,
                },
            );
        });
    }

    pub fn focused(&self, ctx: &Context) -> bool {
        ctx.memory(|m| m.has_focus(self.id()))
    }

    pub fn show(&mut self, ui: &mut Ui) -> Response {
        let touch_mode = matches!(ui.ctx().os(), OperatingSystem::Android | OperatingSystem::IOS);
        ui.vertical(|ui| {
            if touch_mode {
                // touch devices: show find...
                let find_resp = self.find.show(&self.buffer, ui);
                if let Some(term) = find_resp.term {
                    ui.ctx()
                        .push_markdown_event(Event::Find { term, backwards: find_resp.backwards });
                }

                // ...then show editor content...
                let resp = ui
                    .allocate_ui(
                        egui::vec2(
                            ui.available_width(),
                            ui.available_height() - MOBILE_TOOL_BAR_SIZE,
                        ),
                        |ui| self.show_inner(touch_mode, ui),
                    )
                    .inner;

                // ...then show toolbar at the bottom
                self.toolbar.show(
                    &self.ast,
                    &self.bounds,
                    self.buffer.current.selection,
                    self.virtual_keyboard_shown,
                    ui,
                );
                resp
            } else {
                // non-touch devices: show toolbar...
                self.toolbar.show(
                    &self.ast,
                    &self.bounds,
                    self.buffer.current.selection,
                    self.virtual_keyboard_shown,
                    ui,
                );

                // ...then show find...
                let find_resp = self.find.show(&self.buffer, ui);
                if let Some(term) = find_resp.term {
                    ui.ctx()
                        .push_markdown_event(Event::Find { term, backwards: find_resp.backwards });
                }

                // ...then show editor content
                self.show_inner(touch_mode, ui)
            }
        })
        .inner
    }

    pub fn show_inner(&mut self, touch_mode: bool, ui: &mut Ui) -> Response {
        if ui.style_mut().visuals.dark_mode {
            // #282828 raisin black
            ui.style_mut().visuals.code_bg_color = Color32::from_rgb(40, 40, 40);
        } else {
            // #F5F5F5 white smoke
            ui.style_mut().visuals.code_bg_color = Color32::from_rgb(245, 245, 245);
        }

        let scroll_area_id = ui.id().with("child").with(egui::Id::new(self.file_id));
        let prev_scroll_area_offset = ui.data_mut(|d| {
            d.get_persisted(scroll_area_id)
                .map(|s: scroll_area::State| s.offset)
                .unwrap_or_default()
        });

        if touch_mode {
            ui.ctx().style_mut(|style| {
                style.spacing.scroll = egui::style::ScrollStyle::solid();
            });
        }
        Frame::canvas(ui.style())
            .stroke(Stroke::NONE)
            .show(ui, |ui| {
                let scroll_area_output = ScrollArea::vertical()
                    .drag_to_scroll(touch_mode)
                    .id_source(self.file_id)
                    .show(ui, |ui| {
                        ui.vertical_centered(|ui| {
                            // register widget id
                            ui.ctx().check_for_id_clash(self.id(), Rect::NOTHING, "");

                            Frame::canvas(ui.style())
                                .stroke(Stroke::NONE)
                                .inner_margin(Margin::same(15.))
                                .show(ui, |ui| self.show_inner_inner(ui, touch_mode))
                                .inner
                        })
                        .inner
                    });
                let mut resp = scroll_area_output.inner;

                resp.scroll_updated = scroll_area_output.state.offset != prev_scroll_area_offset;

                resp
            })
            .inner
    }

    fn show_inner_inner(&mut self, ui: &mut Ui, touch_mode: bool) -> Response {
        self.debug.frame_start();

        // update theme
        let theme_updated = self.appearance.set_theme(ui.visuals());

        // remember state for change detection
        let prior_suggested_title = self.get_suggested_title();
        let prior_selection = self.buffer.current.selection;

        // process events
        let text_updated = if self.initialized {
            self.process_events(ui.ctx())
        } else {
            self.initialized = true;
            true
        };
        let selection_updated = prior_selection != self.buffer.current.selection;

        // recalculate dependent state
        if text_updated {
            self.ast = ast::calc(&self.buffer);
            self.bounds.ast = bounds::calc_ast(&self.ast);
            self.bounds.words =
                bounds::calc_words(&self.buffer, &self.ast, &self.bounds.ast, &self.appearance);
            self.bounds.paragraphs = bounds::calc_paragraphs(&self.buffer);
        }
        let cursor_screen_postition_updated =
            text_updated || selection_updated || self.capture.mark_changes_processed();
        if cursor_screen_postition_updated {
            self.bounds.text = bounds::calc_text(
                &self.ast,
                &self.bounds.ast,
                &self.appearance,
                &self.buffer.current.segs,
                self.buffer.current.selection,
                ui.ctx().input(|i| i.pointer.primary_down()),
                &self.capture,
            );
            self.bounds.links = bounds::calc_links(&self.buffer, &self.bounds.text, &self.ast);
        }
        if text_updated || selection_updated || theme_updated {
            self.images =
                images::calc(&self.ast, &self.images, &self.client, &self.core, self.file_id, ui);
        }
        self.galleys = galleys::calc(
            &self.ast,
            &self.buffer,
            &self.bounds,
            &self.images,
            &self.appearance,
            touch_mode,
            ui,
        );
        self.bounds.lines = bounds::calc_lines(&self.galleys, &self.bounds.ast, &self.bounds.text);
        self.capture.update(
            ui.input(|i| i.pointer.latest_pos()),
            Instant::now(),
            &self.galleys,
            &self.buffer.current.segs,
            &self.bounds,
            &self.ast,
        );

        // repaint conditions
        let mut repaints = Vec::new();
        if self.images.any_loading() {
            // repaint every 50ms until images load
            repaints.push(Duration::from_millis(50));
        }
        if let Some(recalc_after) = self.capture.recalc_after() {
            // repaint when capture state needs it
            repaints.push(recalc_after);
        }
        if let Some(&repaint_after) = repaints.iter().min() {
            ui.ctx().request_repaint_after(repaint_after);
        }

        // draw
        self.draw_text(ui);
        if self.focused(ui.ctx()) && !cfg!(target_os = "ios") {
            self.draw_cursor(ui, touch_mode);
        }
        if self.debug.draw_enabled {
            self.draw_debug(ui);
        }

        // scroll
        let all_selection = {
            DocCharOffset(0)
                .range_bound(Bound::Doc, false, false, &self.bounds)
                .unwrap() // there's always a document
        };
        if selection_updated && self.buffer.current.selection != all_selection {
            let cursor_end_line = cursor::line(
                self.buffer.current.selection.1,
                &self.galleys,
                &self.bounds.text,
                &self.appearance,
            );
            let rect = Rect { min: cursor_end_line[0], max: cursor_end_line[1] };
            ui.scroll_to_rect(rect.expand(rect.height()), None);
        }

        let suggested_title = self.get_suggested_title();
        let suggest_rename =
            if suggested_title != prior_suggested_title { suggested_title } else { None };

        // focus editor by default
        if ui.memory(|m| m.focused().is_none()) {
            self.focus(ui.ctx());
        }
        if self.focused(ui.ctx()) {
            self.focus_lock(ui.ctx());
        }

        Response {
            text_updated,
            cursor_screen_postition_updated,
            scroll_updated: false, // set by scroll_ui
            suggest_rename,
        }
    }

    fn get_suggested_title(&self) -> Option<String> {
        if !self.needs_name {
            return None;
        }

        let ast_ranges = self
            .bounds
            .ast
            .iter()
            .map(|range| range.range)
            .collect::<Vec<_>>();
        for ([ast_idx, paragraph_idx], text_range_portion) in
            bounds::join([&ast_ranges, &self.bounds.paragraphs])
        {
            if let Some(ast_idx) = ast_idx {
                let ast_text_range = &self.bounds.ast[ast_idx];
                if ast_text_range.range_type != AstTextRangeType::Text {
                    continue; // no syntax characters in suggested title
                }
                if ast_text_range.is_empty() {
                    continue; // no empty text in suggested title
                }
            }
            if paragraph_idx > Some(0) {
                break; // suggested title must be from first paragraph
            }

            return Some(String::from(&self.buffer[text_range_portion]) + ".md");
        }
        None
    }
}
