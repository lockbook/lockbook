use std::mem;

use colored::Colorize as _;
use comrak::Arena;
use comrak::nodes::AstNode;
use core::time::Duration;
use egui::os::OperatingSystem;
use egui::scroll_area::{ScrollAreaOutput, ScrollBarVisibility, ScrollSource};
use egui::{Frame, Pos2, Rect, ScrollArea, Sense, Stroke, Ui, UiBuilder, Vec2, scroll_area};
use web_time::Instant;

use super::input::Event;
use super::widget::toolbar::MOBILE_TOOL_BAR_SIZE;
use super::{MdEdit, MdFilePersistence, Response};
use crate::resolvers::EmbedResolver;
use crate::resolvers::LinkResolver;
use crate::theme::palette_v2::ThemeExt as _;

static PRINT: bool = false;

impl<E: EmbedResolver, L: LinkResolver> MdEdit<E, L> {
    pub fn show(&mut self, ui: &mut Ui) -> Response {
        let mut resp: Response = mem::take(&mut self.next_resp);

        let height = ui.available_size().y.round();
        let width = ui
            .max_rect()
            .width()
            .min(self.renderer.layout.max_width)
            .round();
        let height_updated = self.renderer.height != height;
        let width_updated = self.renderer.width != width;
        self.renderer.height = height;
        self.renderer.width = width;

        let dark_mode = ui.style().visuals.dark_mode;
        if dark_mode != self.renderer.dark_mode {
            self.renderer.syntax.clear();
            self.renderer.dark_mode = dark_mode;
        }

        self.renderer.calc_source_lines();

        let start = web_time::Instant::now();

        let arena = Arena::new();
        let options = Self::comrak_options();

        let text_with_newline = self.renderer.buffer.current.text.to_string() + "\n"; // todo: probably not okay but this parser quirky af sometimes
        let mut root = comrak::parse_document(&arena, &text_with_newline, &options);

        let ast_elapsed = start.elapsed();
        let start = web_time::Instant::now();

        if PRINT {
            println!(
                "{}",
                "================================================================================"
                    .bright_black()
            );
            super::debug_ast::print_ast(root);
        }

        let print_elapsed = start.elapsed();
        let start = web_time::Instant::now();

        self.renderer.embed_resolver.begin_frame();

        // process events
        let prior_selection = self.renderer.buffer.current.selection;
        let images_updated = {
            let last_modified = self.renderer.embed_resolver.last_modified();
            if last_modified > self.embed_resolver_last_processed {
                self.embed_resolver_last_processed = last_modified;
                true
            } else {
                false
            }
        };

        self.emoji_completions
            .update_active_state(&self.renderer.buffer, &self.renderer.bounds.inline_paragraphs);
        self.link_completions.update_active_state(
            &self.renderer.buffer,
            &self.renderer.bounds.inline_paragraphs,
            &self.files,
            self.file_id,
        );
        let buffer_resp = self.process_events(ui.ctx(), root);
        resp.open_camera = buffer_resp.open_camera;

        if !self.initialized || buffer_resp.text_updated {
            resp.text_updated = true;

            // need to re-parse ast to compute bounds which are referenced by mobile virtual keyboard between frames
            let text_with_newline = self.renderer.buffer.current.text.to_string() + "\n"; // todo: probably not okay but this parser quirky af sometimes
            root = comrak::parse_document(&arena, &text_with_newline, &options);

            self.renderer.prepare(root);

            // recompute find matches when text changes
            if let Some(term) = &self.find.term {
                let term = term.clone();
                self.find.matches = self.find_all(&term);
                if self.find.matches.is_empty() {
                    self.find.current_match = None;
                } else if let Some(idx) = self.find.current_match {
                    if idx >= self.find.matches.len() {
                        self.find.current_match = Some(self.find.matches.len() - 1);
                    }
                }
            }

            ui.ctx().request_repaint();
        }
        resp.selection_updated = prior_selection
            != self
                .in_progress_selection
                .unwrap_or(self.renderer.buffer.current.selection);

        let ctx = self.renderer.ctx.clone();
        self.renderer.interactive = !self.readonly;
        self.renderer.reveal_ranges.clear();
        if self.renderer.interactive && self.focused(&ctx) {
            self.renderer
                .reveal_ranges
                .push(self.renderer.buffer.current.selection);
        }
        if let Some(idx) = self.find.current_match {
            if let Some(&range) = self.find.matches.get(idx) {
                self.renderer.reveal_ranges.push(range);
            }
        }
        self.renderer.text_highlight_range = self
            .emoji_completions
            .search_term_range
            .or(self.link_completions.search_term_range);

        self.populate_galley_required_ranges();

        ui.painter()
            .rect_filled(ui.max_rect(), 0., self.renderer.ctx.get_lb_theme().neutral_bg());
        self.renderer.apply_theme(ui);
        ui.spacing_mut().item_spacing.x = 0.;

        let scroll_area_id = ui
            .vertical(|ui| {
                let scroll_area_id = if self.renderer.touch_mode {
                    self.show_find_centered(ui);

                    // ...then show editor content (or toolbar settings)...
                    let available_width = ui.available_width();
                    let toolbar_height = if !self.readonly
                        && (self.virtual_keyboard_shown || self.toolbar.menu_open)
                    {
                        MOBILE_TOOL_BAR_SIZE
                    } else {
                        0.
                    };
                    let scroll_area_id = ui
                        .allocate_ui(
                            egui::vec2(
                                ui.available_width(),
                                ui.available_height() - toolbar_height,
                            ),
                            |ui| {
                                ui.ctx().style_mut(|style| {
                                    style.spacing.scroll = egui::style::ScrollStyle::solid();
                                    style.spacing.scroll.bar_width = 10.;
                                });

                                if !self.toolbar.menu_open {
                                    // these are computed during render
                                    self.renderer.galleys.galleys.clear();
                                    self.renderer.bounds.wrap_lines.clear();
                                    self.renderer.touch_consuming_rects.clear();

                                    // show editor
                                    let scroll_area_id = ui.id().with(egui::Id::new(self.file_id));
                                    let scroll_area_offset = ui.data_mut(|d| {
                                        d.get_persisted(scroll_area_id)
                                            .map(|s: scroll_area::State| s.offset)
                                            .unwrap_or_default()
                                            .y
                                    });

                                    let scroll_area_output = self.show_scrollable_editor(ui, root);
                                    self.next_resp.scroll_updated =
                                        scroll_area_output.state.offset.y != scroll_area_offset;
                                    self.scroll_area_velocity = scroll_area_output.state.velocity();

                                    Some(scroll_area_id)
                                } else {
                                    // show toolbar settings
                                    self.show_toolbar_menu(ui);

                                    None
                                }
                            },
                        )
                        .inner;

                    // ...then show toolbar at the bottom
                    if !self.readonly && (self.virtual_keyboard_shown || self.toolbar.menu_open) {
                        let (_, rect) =
                            ui.allocate_space(egui::vec2(available_width, MOBILE_TOOL_BAR_SIZE));
                        ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
                            self.show_toolbar(root, ui);
                        });
                    }

                    scroll_area_id
                } else {
                    let scroll_area_id = ui.id().with(egui::Id::new(self.file_id));
                    let scroll_area_offset = ui.data_mut(|d| {
                        d.get_persisted(scroll_area_id)
                            .map(|s: scroll_area::State| s.offset)
                            .unwrap_or_default()
                            .y
                    });

                    if !self.readonly {
                        self.show_toolbar(root, ui);
                    }
                    self.show_find_centered(ui);

                    // these are computed during render
                    self.renderer.galleys.galleys.clear();
                    self.renderer.bounds.wrap_lines.clear();
                    self.renderer.touch_consuming_rects.clear();

                    // ...then show editor content
                    let scroll_area_output = self.show_scrollable_editor(ui, root);
                    self.next_resp.scroll_updated =
                        scroll_area_output.state.offset.y != scroll_area_offset;
                    self.scroll_area_velocity = scroll_area_output.state.velocity();

                    Some(scroll_area_id)
                };

                // persistence: read
                if !self.initialized {
                    let persisted = self
                        .persistence
                        .get_markdown()
                        .file
                        .get(&self.file_id)
                        .cloned()
                        .unwrap_or_default();
                    if let Some(scroll_area_id) = scroll_area_id {
                        ui.data_mut(|d| {
                            let state: Option<scroll_area::State> = d.get_persisted(scroll_area_id);
                            if let Some(mut state) = state {
                                state.offset.y = persisted.scroll_offset;
                                d.insert_temp(scroll_area_id, state);
                            }
                        });
                    }
                    // set the selection using low-level API; using internal
                    // events causes touch devices to scroll to cursor on 2nd
                    // frame
                    let (start, end) = persisted.selection;
                    let selection = (
                        start.clamp(
                            0.into(),
                            self.renderer.buffer.current.segs.last_cursor_position(),
                        ),
                        end.clamp(
                            0.into(),
                            self.renderer.buffer.current.segs.last_cursor_position(),
                        ),
                    );
                    self.renderer.buffer.queue(vec![
                        lb_rs::model::text::operation_types::Operation::Select(selection),
                    ]);
                    self.renderer.buffer.update();
                }

                scroll_area_id
            })
            .inner;

        self.event
            .internal_events
            .append(&mut self.renderer.render_events);

        let text_areas = std::mem::take(&mut self.renderer.text_areas);
        if !text_areas.is_empty() {
            ui.painter()
                .add(egui_wgpu_renderer::egui_wgpu::Callback::new_paint_callback(
                    ui.max_rect(),
                    crate::GlyphonRendererCallback::new(text_areas),
                ));
        }
        self.show_emoji_completions(ui);
        self.show_link_completions(ui);

        self.renderer.syntax.garbage_collect();

        let render_elapsed = start.elapsed();

        if self.renderer.debug {
            self.renderer.show_debug_fps(ui);
        }

        if PRINT {
            println!(
                "{}",
                "--------------------------------------------------------------------------------"
                    .bright_black()
            );
            println!("document: {:?}", self.renderer.buffer.current.text);
            println!(
                "{}",
                "--------------------------------------------------------------------------------"
                    .bright_black()
            );
            println!(
                "                                                                 ast: {ast_elapsed:?}"
            );
            println!(
                "                                                               print: {print_elapsed:?}"
            );
            println!(
                "                                                              render: {render_elapsed:?}"
            );
        }

        // post-frame bookkeeping
        let all_selected = self.renderer.buffer.current.selection
            == (0.into(), self.renderer.last_cursor_position());
        if images_updated || height_updated || width_updated {
            self.renderer.layout_cache.clear();
            ui.ctx().request_repaint();
        } else if resp.selection_updated {
            let new_selection = self
                .in_progress_selection
                .unwrap_or(self.renderer.buffer.current.selection);
            self.renderer
                .layout_cache
                .invalidate_reveal_change(prior_selection, new_selection);
            ui.ctx().request_repaint();
        }
        if self.initialized && resp.selection_updated && !all_selected {
            self.scroll_to_cursor = true;
            ui.ctx().request_repaint();
        }
        if self.initialized && self.renderer.touch_mode && height_updated {
            self.scroll_to_cursor = true;
            ui.ctx().request_repaint();
        }
        if self.next_resp.scroll_updated {
            self.unprocessed_scroll = Some(Instant::now());
            ui.ctx().request_repaint();
        }
        if !self.event.internal_events.is_empty() {
            ui.ctx().request_repaint();
        }

        // persistence: write
        let mut persistence_updated = false;
        if resp.selection_updated {
            let mut persistence = self.persistence.data.write().unwrap();
            persistence
                .markdown
                .file
                .entry(self.file_id)
                .and_modify(|f| f.selection = self.renderer.buffer.current.selection)
                .or_insert(MdFilePersistence {
                    scroll_offset: Default::default(),
                    selection: self.renderer.buffer.current.selection,
                });
            persistence_updated = true;
        }

        let mut scroll_end_processed = false;
        if let Some(unprocessed_scroll) = self.unprocessed_scroll {
            if unprocessed_scroll.elapsed() > Duration::from_millis(100) {
                if let Some(scroll_area_id) = scroll_area_id {
                    let state: Option<scroll_area::State> = ui.data(|d| d.get_temp(scroll_area_id));
                    let scroll_offset = if let Some(state) = state { state.offset.y } else { 0. };

                    let mut persistence = self.persistence.data.write().unwrap();
                    persistence
                        .markdown
                        .file
                        .entry(self.file_id)
                        .and_modify(|f| f.scroll_offset = scroll_offset)
                        .or_insert(MdFilePersistence {
                            scroll_offset,
                            selection: Default::default(),
                        });
                    persistence_updated = true;

                    scroll_end_processed = true;
                }
            }
        };

        if scroll_end_processed {
            self.unprocessed_scroll = None;
        }
        if persistence_updated {
            self.persistence.write_to_file();
        }

        // focus editor when first shown or when nothing else has focus
        if !self.initialized || ui.memory(|m| m.focused().is_none()) {
            self.focus(ui.ctx());
        }
        if self.focused(ui.ctx()) {
            self.focus_lock(ui.ctx());
        }

        self.initialized = true;

        self.renderer.embed_resolver.end_frame();

        resp
    }

    pub fn will_consume_touch(&self, pos: Pos2) -> bool {
        self.renderer
            .touch_consuming_rects
            .iter()
            .any(|rect| rect.contains(pos))
            || self.scroll_area_velocity.abs().max_elem() > 0.
            || self.toolbar.menu_open
    }

    fn show_scrollable_editor<'a>(
        &mut self, ui: &mut Ui, root: &'a AstNode<'a>,
    ) -> ScrollAreaOutput<()> {
        let margin: egui::Margin = if cfg!(target_os = "android") {
            egui::Margin::symmetric(0, 60)
        } else {
            egui::Margin::symmetric(0, 15)
        };
        ScrollArea::vertical()
            .scroll_source(if self.renderer.touch_mode {
                ScrollSource::ALL
            } else {
                ScrollSource::SCROLL_BAR | ScrollSource::MOUSE_WHEEL
            })
            .id_salt(self.file_id)
            .scroll_bar_visibility(if self.renderer.touch_mode {
                ScrollBarVisibility::AlwaysVisible
            } else {
                ScrollBarVisibility::VisibleWhenNeeded
            })
            .show(ui, |ui| {
                ui.vertical_centered_justified(|ui| {
                    Frame::canvas(ui.style())
                        .inner_margin(margin)
                        .stroke(Stroke::NONE)
                        .show(ui, |ui| {
                            let scroll_view_height = ui.max_rect().height();
                            ui.allocate_space(Vec2 { x: ui.available_width(), y: 0. });

                            let padding = (ui.available_width() - self.renderer.width) / 2.;

                            self.renderer.top_left = ui.max_rect().min
                                + (padding + self.renderer.layout.margin) * Vec2::X;
                            let height = {
                                let document_height = self.renderer.height(root, &[root]);
                                let unfilled_space = if document_height < scroll_view_height {
                                    scroll_view_height - document_height
                                } else {
                                    0.
                                };
                                let end_of_text_padding = scroll_view_height / 2.;

                                document_height + unfilled_space.max(end_of_text_padding)
                            };
                            let rect = Rect::from_min_size(
                                self.renderer.top_left,
                                Vec2::new(
                                    self.renderer.width - 2. * self.renderer.layout.margin,
                                    height,
                                ),
                            );

                            ui.ctx().check_for_id_clash(self.id(), rect, ""); // registers this widget so it's not forgotten by next frame
                            let focused = self.focused(ui.ctx());
                            let response = ui.interact(
                                rect,
                                self.id(),
                                if self.renderer.touch_mode {
                                    Sense::click()
                                } else {
                                    Sense::click_and_drag()
                                },
                            );
                            if focused && !self.focused(ui.ctx()) {
                                // interact surrenders focus if we don't have sense focusable, but also if user clicks elsewhere, even on a child
                                self.focus(ui.ctx());
                            }
                            let response_properly_clicked =
                                response.clicked_by(egui::PointerButton::Primary);
                            if response.hovered() || response_properly_clicked {
                                ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Text);
                                // overridable by widgets
                            }

                            ui.advance_cursor_after_rect(rect);

                            ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
                                self.renderer
                                    .show_block(ui, root, self.renderer.top_left, &[root]);
                            });
                        });
                });
                self.renderer.galleys.galleys.sort_by_key(|g| g.range);

                if !self.readonly && ui.ctx().os() != OperatingSystem::IOS {
                    let selection = self
                        .in_progress_selection
                        .unwrap_or(self.renderer.buffer.current.selection);
                    let theme = self.renderer.ctx.get_lb_theme();
                    let color = theme.bg().get_color(theme.prefs().primary);
                    self.show_range(ui, selection, color.lerp_to_gamma(theme.neutral_bg(), 0.7));
                    self.show_offset(ui, selection.1, color);

                    if self.focused(ui.ctx()) {
                        if let Some([top, bot]) = self.cursor_line(selection.1) {
                            let cursor_rect = egui::Rect::from_min_max(top, bot);
                            ui.output_mut(|o| {
                                o.ime = Some(egui::output::IMEOutput {
                                    rect: ui.max_rect(),
                                    cursor_rect,
                                });
                            });
                        }
                    }
                }

                // show find match highlights
                if !self.find.matches.is_empty() {
                    let theme = self.renderer.ctx.get_lb_theme();
                    let highlight_color = theme.neutral_bg_tertiary();
                    let current_color = theme.fg().yellow.lerp_to_gamma(theme.neutral_bg(), 0.5);
                    for (i, &match_range) in self.find.matches.iter().enumerate() {
                        let color = if self.find.current_match == Some(i) {
                            current_color
                        } else {
                            highlight_color
                        };
                        self.show_range(ui, match_range, color);
                    }
                }

                if ui.ctx().os() == OperatingSystem::Android {
                    self.show_selection_handles(ui);
                }
                if mem::take(&mut self.scroll_to_cursor) {
                    self.scroll_to_cursor(ui);
                }
                if mem::take(&mut self.scroll_to_find_match) {
                    self.scroll_to_find_match(ui);
                }
            })
    }

    fn show_find_centered(&mut self, ui: &mut Ui) {
        let available = ui.available_width();
        let content_width = if self.renderer.touch_mode {
            self.renderer.width
        } else {
            self.toolbar_width().min(self.renderer.width)
        };
        let content_left = ui.max_rect().left() + (available - content_width) / 2.;
        let top = ui.cursor().min.y;
        let find_rect =
            Rect::from_min_size(egui::pos2(content_left, top), egui::vec2(content_width, 0.));
        let scope_resp = ui.scope_builder(egui::UiBuilder::new().max_rect(find_rect), |ui| {
            self.find
                .show(&self.renderer.buffer, self.virtual_keyboard_shown, ui)
        });
        let find_resp = scope_resp.inner;
        let rendered_rect = scope_resp.response.rect;
        ui.advance_cursor_after_rect(rendered_rect);
        self.next_resp.find_widget_height = rendered_rect.height();
        self.process_find_response(find_resp);
    }

    fn process_find_response(&mut self, resp: super::widget::find::Response) {
        if resp.replace_one {
            if let Some(idx) = self.find.current_match {
                if let Some(&match_range) = self.find.matches.get(idx) {
                    let replacement = self.find.replace_term.clone();
                    self.event.internal_events.push(Event::Replace {
                        region: match_range.into(),
                        text: replacement,
                        advance_cursor: false,
                    });
                }
            }
        }
        if resp.replace_all {
            let replacement = self.find.replace_term.clone();
            for &match_range in self.find.matches.iter().rev() {
                self.event.internal_events.push(Event::Replace {
                    region: match_range.into(),
                    text: replacement.clone(),
                    advance_cursor: false,
                });
            }
        }
        if resp.term_changed {
            let term = self.find.term.clone().unwrap_or_default();
            self.event.internal_events.push(Event::FindSearch { term });
        }
        if let Some(forward) = resp.navigate {
            self.event
                .internal_events
                .push(Event::FindNavigate { backwards: !forward });
        }
        if resp.closed {
            self.find.matches.clear();
            self.find.current_match = None;
            self.renderer.layout_cache.clear();
        }
    }

    fn scroll_to_find_match(&self, ui: &mut Ui) {
        if let Some(idx) = self.find.current_match {
            if let Some(match_range) = self.find.matches.get(idx) {
                let rects = self.range_rects(*match_range);
                if let Some(rect) = rects.first() {
                    ui.scroll_to_rect(rect.expand(rect.height()), Some(egui::Align::Center));
                }
            }
        }
    }
}
