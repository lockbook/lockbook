use crate::tab::markdown_editor;
use crate::tab::ExtendedInput as _;
use egui::os::OperatingSystem;
use egui::{Color32, Frame, PointerButton, Rect, Sense, TouchPhase, Ui, Vec2};
use lb_rs::{DocumentHmac, Uuid};
use markdown_editor::appearance::Appearance;
use markdown_editor::ast::{Ast, AstTextRangeType};
use markdown_editor::bounds::{BoundCase, Bounds};
use markdown_editor::buffer::Buffer;
use markdown_editor::debug::DebugInfo;
use markdown_editor::galleys::Galleys;
use markdown_editor::images::ImageCache;
use markdown_editor::input::capture::CaptureState;
use markdown_editor::input::click_checker::{ClickChecker, EditorClickChecker};
use markdown_editor::input::cursor;
use markdown_editor::input::cursor::{CursorState, PointerState};
use markdown_editor::input::{Bound, Event, Offset, Region};
use markdown_editor::offset_types::{DocCharOffset, RangeExt as _};
use markdown_editor::{ast, bounds, galleys, images};
use serde::Serialize;
use std::time::{Duration, Instant};

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
    pub id: egui::Id,
    pub file_id: Uuid,
    pub hmac: Option<DocumentHmac>,
    pub needs_name: bool,
    pub initialized: bool,
    pub appearance: Appearance,

    // internal systems
    pub buffer: Buffer,
    pub cursor: CursorState,
    pub pointer_state: PointerState,
    pub debug: DebugInfo,
    pub images: ImageCache,
    pub ast: Ast,
    pub bounds: Bounds,
    pub galleys: Galleys,
    pub capture: CaptureState,

    // state from last frame for focus & change detection
    pub has_focus: bool,
    pub ui_rect: Rect,
    pub scroll_area_rect: Rect,
    pub scroll_area_offset: Vec2,

    // referenced by toolbar for keyboard toggle (todo: cleanup)
    pub is_virtual_keyboard_showing: bool,
}

impl Editor {
    pub fn new(
        core: lb_rs::Core, content: &str, file_id: Uuid, hmac: Option<DocumentHmac>,
        needs_name: bool, plaintext_mode: bool,
    ) -> Self {
        Self {
            core,
            client: Default::default(),

            id: egui::Id::new(file_id),
            file_id,
            hmac,
            needs_name,
            initialized: false,
            appearance: Appearance { plaintext_mode, ..Default::default() },

            buffer: content.into(),
            cursor: Default::default(),
            pointer_state: Default::default(),
            debug: Default::default(),
            images: Default::default(),
            ast: Default::default(),
            bounds: Default::default(),
            galleys: Default::default(),
            capture: Default::default(),

            has_focus: true,
            ui_rect: Rect { min: Default::default(), max: Default::default() },
            scroll_area_rect: Rect { min: Default::default(), max: Default::default() },
            scroll_area_offset: Default::default(),

            is_virtual_keyboard_showing: false,
        }
    }

    /// Loads a new string into the buffer, merging out-of-editor changes made since last load with in-editor changes
    /// made since last load. The buffer's undo history is preserved; undo'ing will revert in-editor changes only.
    /// Exercising undo's may put the buffer in never-before-seen states and exercising all undo's will revert the
    /// buffer to the most recently loaded state (undo limit permitting).
    pub fn reload(&mut self, text: String) {
        self.buffer.reload(text)
    }

    pub fn show(&mut self, ui: &mut Ui) -> Response {
        let touch_mode = matches!(ui.ctx().os(), OperatingSystem::Android | OperatingSystem::IOS);

        let events = ui.ctx().input(|i| i.events.clone());
        ui.interact(self.scroll_area_rect, self.id, Sense::focusable_noninteractive());

        // calculate focus
        let mut request_focus = ui.memory(|m| m.has_focus(self.id));
        let mut surrender_focus = false;
        for event in &events {
            if let egui::Event::PointerButton { pos, pressed: true, .. } = event {
                if ui.is_enabled() && self.scroll_area_rect.contains(*pos) && self.has_focus {
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
                        m.request_focus(self.id);
                    });
                }
                if surrender_focus {
                    ui.memory_mut(|m| m.surrender_focus(self.id));
                }
                ui.memory_mut(|m| {
                    if m.has_focus(self.id) {
                        focus = true;
                        m.set_focus_lock_filter(
                            self.id,
                            egui::EventFilter {
                                tab: true,
                                horizontal_arrows: true,
                                vertical_arrows: true,
                                escape: true,
                            },
                        );
                    }
                });

                let fill =
                    if ui.style().visuals.dark_mode { Color32::BLACK } else { Color32::WHITE };

                Frame::default()
                    .fill(fill)
                    .inner_margin(egui::Margin::symmetric(0.0, 15.0))
                    .show(ui, |ui| {
                        ui.vertical_centered(|ui| self.show_inner(ui, self.id, touch_mode, events))
                    })
            });
        self.ui_rect = sao.inner_rect;

        // set focus again because egui clears it for our widget for some reason
        if focus {
            ui.memory_mut(|m| {
                m.request_focus(self.id);
                m.set_focus_lock_filter(
                    self.id,
                    egui::EventFilter {
                        tab: true,
                        horizontal_arrows: true,
                        vertical_arrows: true,
                        escape: true,
                    },
                );
            });
        }

        let mut resp = sao.inner.inner.inner;
        resp.scroll_updated = self.scroll_area_offset != sao.state.offset;

        // remember scroll area rect for focus next frame
        self.scroll_area_rect = sao.inner_rect;

        // remember scroll area offset for change detection
        self.scroll_area_offset = sao.state.offset;

        resp
    }

    fn show_inner(
        &mut self, ui: &mut Ui, id: egui::Id, touch_mode: bool, events: Vec<egui::Event>,
    ) -> Response {
        self.debug.frame_start();

        // update theme
        let theme_updated = self.appearance.set_theme(ui.visuals());

        // clip elements width
        let max_width = 800.0;
        if ui.max_rect().width() > max_width {
            ui.set_max_width(max_width);
        } else {
            ui.set_max_width(ui.max_rect().width() - 15.);
        }

        // remember state for change detection
        let prior_suggested_title = self.get_suggested_title();

        // process events
        let (text_updated, selection_updated) = if self.initialized {
            if ui.memory(|m| m.has_focus(id))
                || cfg!(target_os = "ios")
                || cfg!(target_os = "android")
            {
                let custom_events = ui.ctx().pop_events();
                self.process_events(ui.ctx(), events, custom_events, touch_mode)
            } else {
                (false, false)
            }
        } else {
            ui.memory_mut(|m| m.request_focus(id));

            // put the cursor at the first valid cursor position
            ui.ctx().push_markdown_event(Event::Select {
                region: Region::ToOffset {
                    offset: Offset::To(Bound::Doc),
                    backwards: true,
                    extend_selection: false,
                },
            });

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
                &self.bounds.paragraphs,
                &self.appearance,
                &self.buffer.current.segs,
                self.buffer.current.selection,
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
            ui,
        );
        self.bounds.lines = bounds::calc_lines(&self.galleys, &self.bounds.ast, &self.bounds.text);
        self.capture.update(
            Instant::now(),
            &self.pointer_state,
            &self.galleys,
            &self.buffer.current.segs,
            &self.bounds,
            &self.ast,
        );
        self.initialized = true;

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
        self.draw_text(self.ui_rect.size(), ui, touch_mode);
        if ui.memory(|m| m.has_focus(id)) && !cfg!(target_os = "ios") {
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

        // set cursor style
        {
            let click_checker = &EditorClickChecker {
                ui_rect: self.ui_rect,
                galleys: &self.galleys,
                buffer: &self.buffer,
                ast: &self.ast,
                appearance: &self.appearance,
                bounds: &self.bounds,
            };
            let hovering_link = ui
                .input(|r| r.pointer.hover_pos())
                .map(|pos| click_checker.link(pos).is_some())
                .unwrap_or_default();
            let hovering_text = ui
                .input(|r| r.pointer.hover_pos())
                .map(|pos| click_checker.text(pos).is_some())
                .unwrap_or_default();
            let cmd_down = ui.input(|i| i.modifiers.command);
            if hovering_link && cmd_down {
                ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
            } else if hovering_text {
                ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Text);
            }
        }

        Response {
            text_updated,
            selection_updated,
            scroll_updated: false, // set by scroll_ui
            suggest_rename,
        }
    }

    fn process_events(
        &mut self, ctx: &egui::Context, mut events: Vec<egui::Event>,
        mut custom_events: Vec<crate::Event>, touch_mode: bool,
    ) -> (bool, bool) {
        // if the cursor is in an invalid location, move it to the next valid location
        {
            let mut fixed_selection = self.buffer.current.selection;
            if let BoundCase::BetweenRanges { range_after, .. } =
                fixed_selection.0.bound_case(&self.bounds.text)
            {
                fixed_selection.0 = range_after.start();
            }
            if let BoundCase::BetweenRanges { range_after, .. } =
                fixed_selection.1.bound_case(&self.bounds.text)
            {
                fixed_selection.1 = range_after.start();
            }
            if fixed_selection != self.buffer.current.selection {
                let event =
                    crate::Event::Markdown(Event::Select { region: fixed_selection.into() });
                custom_events.splice(0..0, std::iter::once(event));
            }
        }

        // remove clicks that are also touches so we don't click to set selection while touching to open a context menu
        // todo: O(n), fewer clones
        let mut i = 1;
        loop {
            if i >= events.len() {
                break;
            }

            if matches!((events[i - 1].clone(), events[i].clone()),
                // touch start / pointer pressed
                (
                    egui::Event::Touch { phase: TouchPhase::Start, pos: touch_pos, .. },
                    egui::Event::PointerButton {
                        pos: pointer_pos,
                        button: PointerButton::Primary,
                        pressed: true,
                        ..
                    },
                )
                // touch move / pointer move
                | (
                    egui::Event::Touch { phase: TouchPhase::Move, pos: touch_pos, .. },
                    egui::Event::PointerMoved(pointer_pos),
                )
                // touch end / pointer release
                | (
                    egui::Event::Touch { phase: TouchPhase::End, pos: touch_pos, .. },
                    egui::Event::PointerButton {
                        pos: pointer_pos,
                        button: PointerButton::Primary,
                        pressed: false,
                        ..
                    },
                ) if touch_pos == pointer_pos)
            {
                events.remove(i);
            } else if matches!((events[i - 1].clone(), events[i].clone()),
                // pointer pressed / touch start
                (
                    egui::Event::PointerButton {
                        pos: pointer_pos,
                        button: PointerButton::Primary,
                        pressed: true,
                        ..
                    },
                    egui::Event::Touch { phase: TouchPhase::Start, pos: touch_pos, .. },
                )
                // pointer move / touch move
                | (
                    egui::Event::PointerMoved(pointer_pos),
                    egui::Event::Touch { phase: TouchPhase::Move, pos: touch_pos, .. },
                )
                // pointer release / touch end
                | (
                    egui::Event::PointerButton {
                        pos: pointer_pos,
                        button: PointerButton::Primary,
                        pressed: false,
                        ..
                    },
                    egui::Event::Touch { phase: TouchPhase::End, pos: touch_pos, .. },
                ) if touch_pos == pointer_pos)
            {
                events.remove(i - 1);
            } else {
                i += 1;
            }
        }

        let combined_events = self.combine_events(ctx, events.clone(), custom_events, touch_mode);
        self.process_combined_events(ctx, combined_events)
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
