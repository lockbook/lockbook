use crate::tab::markdown_editor::bounds::BoundExt as _;
use crate::tab::{markdown_editor, ExtendedInput};
use egui::os::OperatingSystem;
use egui::{
    scroll_area, Context, CursorIcon, EventFilter, Frame, Id, Margin, Rect, ScrollArea, Sense,
    Stroke, Ui, Vec2,
};
use lb_rs::text::buffer::Buffer;
use lb_rs::text::offset_types::{DocCharOffset, RangeExt as _};
use lb_rs::{DocumentHmac, Uuid};
use markdown_editor::appearance::Appearance;
use markdown_editor::ast::{Ast, AstTextRangeType};
use markdown_editor::bounds::Bounds;
use markdown_editor::debug::DebugInfo;
use markdown_editor::galleys::Galleys;
use markdown_editor::images::ImageCache;
use markdown_editor::input::capture::CaptureState;
use markdown_editor::input::cursor;
use markdown_editor::input::cursor::CursorState;
use markdown_editor::input::Bound;
use markdown_editor::{ast, bounds, galleys, images};
use serde::Serialize;
use std::time::{Duration, Instant};

use super::find::Find;
use super::input::mutation::EventState;
use super::input::{Location, Region};
use super::Event;

#[derive(Debug, Serialize, Default)]
pub struct Response {
    // state changes
    pub text_updated: bool,
    pub selection_updated: bool,
    pub scroll_updated: bool,

    // actions taken
    pub suggest_rename: Option<String>,
}

pub struct Editor {
    // dependencies
    pub core: lb_rs::Core,
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
    pub find: Find,
    pub event: EventState,

    // referenced by toolbar for keyboard toggle (todo: cleanup)
    pub is_virtual_keyboard_showing: bool,

    // referenced by toolbar for layout (todo: cleanup)
    pub rect: Rect,
}

impl Editor {
    pub fn new(
        core: lb_rs::Core, content: &str, file_id: Uuid, hmac: Option<DocumentHmac>,
        needs_name: bool, plaintext_mode: bool,
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
            find: Default::default(),
            event: Default::default(),

            is_virtual_keyboard_showing: false,

            rect: Rect::ZERO,
        }
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
        let scroll_area_id = ui.id().with("child").with(egui::Id::new(self.file_id));
        let prev_scroll_area_offset = ui.data_mut(|d| {
            d.get_persisted(scroll_area_id)
                .map(|s: scroll_area::State| s.offset)
                .unwrap_or_default()
        });

        let touch_mode = matches!(ui.ctx().os(), OperatingSystem::Android | OperatingSystem::IOS);

        // show find toolbar
        let find_resp = self.find.show(&self.buffer, ui);
        if let Some(term) = find_resp.term {
            ui.ctx()
                .push_markdown_event(Event::Find { term, backwards: find_resp.backwards });
        }

        // show ui
        if touch_mode {
            ui.ctx().style_mut(|style| {
                style.spacing.scroll = egui::style::ScrollStyle::solid();
            });
        }
        Frame::canvas(ui.style())
            .stroke(Stroke::NONE)
            .outer_margin(Margin::same(2.))
            .show(ui, |ui| {
                let scroll_area_output = ScrollArea::vertical()
                    .drag_to_scroll(true)
                    .id_source(self.file_id)
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing = Vec2::ZERO;
                        let max_rect = ui.max_rect();

                        let resp = ui
                            .vertical_centered(|ui| {
                                // clip elements width
                                let max_width = 800.0;
                                if ui.max_rect().width() > max_width + 15. {
                                    ui.set_max_width(max_width);
                                } else {
                                    ui.set_max_width(ui.max_rect().width() - 15.);
                                }

                                // register widget id
                                ui.ctx().check_for_id_clash(self.id(), Rect::NOTHING, "");

                                Frame::canvas(ui.style())
                                    .stroke(Stroke::NONE)
                                    .inner_margin(Margin::same(15.))
                                    .show(ui, |ui| self.show_inner(ui, touch_mode))
                                    .inner
                            })
                            .inner;

                        // fill available space / end of text padding
                        let inner_content_height = ui.cursor().min.y + prev_scroll_area_offset.y;
                        let padding_height = if inner_content_height < max_rect.height() {
                            // fill available space
                            max_rect.height() - inner_content_height
                        } else {
                            // end of text padding
                            max_rect.height() / 2.
                        };
                        let padding_response = ui.allocate_response(
                            Vec2::new(max_rect.width(), padding_height),
                            Sense { click: true, drag: false, focusable: false },
                        );
                        if padding_response.clicked() {
                            ui.ctx().push_markdown_event(Event::Select {
                                region: Region::Location(Location::DocCharOffset(
                                    self.buffer.current.segs.last_cursor_position(),
                                )),
                            });
                            ui.ctx().request_repaint();
                        }
                        if padding_response.hovered() {
                            ui.ctx().set_cursor_icon(CursorIcon::Text);
                        }

                        ui.painter().rect(
                            padding_response.rect,
                            egui::Rounding::ZERO,
                            egui::Color32::BLUE,
                            Stroke::NONE,
                        );

                        resp
                    });
                let mut resp = scroll_area_output.inner;

                self.rect = scroll_area_output.inner_rect;
                resp.scroll_updated = scroll_area_output.state.offset != prev_scroll_area_offset;

                resp
            })
            .inner
    }

    fn show_inner(&mut self, ui: &mut Ui, touch_mode: bool) -> Response {
        self.debug.frame_start();

        // update theme
        let theme_updated = self.appearance.set_theme(ui.visuals());

        // remember state for change detection
        let prior_suggested_title = self.get_suggested_title();

        // process events
        let (text_updated, selection_updated) = if self.initialized {
            self.process_events(ui.ctx())
        } else {
            self.initialized = true;
            (true, true)
        };

        // recalculate dependent state
        if text_updated {
            self.ast = ast::calc(&self.buffer);
            self.bounds.ast = bounds::calc_ast(&self.ast);
            self.bounds.words =
                bounds::calc_words(&self.buffer, &self.ast, &self.bounds.ast, &self.appearance);
            self.bounds.paragraphs = bounds::calc_paragraphs(&self.buffer);
        }
        if text_updated || selection_updated || self.capture.mark_changes_processed() {
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
                self.buffer.current.selection.end(),
                &self.galleys,
                &self.bounds.text,
                &self.appearance,
            );
            let rect = Rect { min: cursor_end_line[0], max: cursor_end_line[1] };
            ui.scroll_to_rect(rect, None);
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
            selection_updated,
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
