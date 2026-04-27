//! `MdEdit` orchestration — split into an early keyboard-input phase and a
//! later draw phase .
//!
//! - [`MdEdit::handle_input`] runs early, processes keyboard + internal
//!   events, mutates the buffer, and populates the fields the completion
//!   popups read (active state, search term range).
//! - [`MdEdit::show`] runs during draw, registers the widget for pointer
//!   hit-test, processes pointer + context-menu events, renders, draws
//!   cursor / selection / touch handles, and consumes a pending
//!   scroll-to-cursor.
//! - [`MdEdit::show_completions`] is the second draw call, invoked after
//!   `show` returns so popup callbacks land on a later glyphon layer and
//!   outside any scroll-area clip.

use comrak::Arena;
use egui::os::OperatingSystem;
use egui::{
    Context, EventFilter, Id, Pos2, Rect, Sense, Stroke, Ui, UiBuilder, Vec2, ViewportCommand,
};
use lb_rs::model::text::buffer::{self, Buffer};
use lb_rs::model::text::offset_types::{Grapheme, RangeExt as _, RangeIterExt as _};

use crate::tab::ExtendedOutput as _;
use crate::tab::markdown_editor::ScrollTarget;
use crate::tab::markdown_editor::bounds::{BoundExt as _, RangesExt as _};
use crate::theme::icons::Icon;
use crate::theme::palette_v2::ThemeExt as _;
use crate::widgets::IconButton;

use super::MdEdit;
use super::input::{Bound, Event, Location, Region};

impl MdEdit {
    /// Input phase: consume keyboard events, drain the internal event queue,
    /// and mutate the buffer. Called early — before any sibling widget's
    /// draw — so `consume_key` happens in priority order with other widgets
    /// (completions already consume inside this function; Find runs its own
    /// `handle_input` before this is invoked in `Editor::show`).
    ///
    /// Callers that want to intercept specific key chords (e.g. a chat
    /// composer treating `Cmd+Enter` as "send") should call `consume_key`
    /// themselves *before* invoking this function; otherwise the keys fall
    /// through to `translate_egui_keyboard_event` (plain `Enter` becomes
    /// `Event::Newline`).
    pub fn handle_input(&mut self, ctx: &Context, id: Id) -> buffer::Response {
        let focused = ctx.memory(|m| m.has_focus(id));

        let arena = Arena::new();
        let root = self.renderer.reparse(&arena);

        // Completion popups: update active state, then give them first dibs on
        // Up/Down/Enter/Cmd+num via `consume_key` so this function's keyboard
        // pass below doesn't see those keys.
        self.emoji_completions
            .update_active_state(&self.renderer.buffer, &self.renderer.bounds.inline_paragraphs);
        let files = self.renderer.files.clone();
        let file_id = self.file_id;
        self.link_completions.update_active_state(
            &self.renderer.buffer,
            &self.renderer.bounds.inline_paragraphs,
            &files,
            file_id,
        );
        if !self.renderer.readonly && !self.renderer.plaintext {
            self.emoji_completions.handle_input(
                ctx,
                &self.renderer.buffer,
                focused,
                &mut self.event.internal_events,
            );
            self.link_completions.handle_input(
                ctx,
                &self.renderer.buffer,
                &files,
                file_id,
                focused,
                &mut self.event.internal_events,
            );
        }

        let mut ops = Vec::new();
        // Some events (Undo, Redo, Camera) mutate state directly inside
        // `calc_operations` instead of pushing to `ops`. Their `Response`
        // must be merged with the queued-op response so reparse & cache
        // invalidation fire when undo changes the text.
        let mut direct_resp = buffer::Response::default();

        for event in std::mem::take(&mut self.event.internal_events) {
            direct_resp |= self.calc_operations(ctx, root, event, &mut ops);
        }

        if focused {
            let filter = EventFilter {
                tab: true,
                horizontal_arrows: true,
                vertical_arrows: true,
                escape: false,
            };
            let key_events: Vec<egui::Event> = ctx.input(|r| r.filtered_events(&filter));
            for e in key_events {
                if let Some(edit_event) = self.translate_egui_keyboard_event(e, root) {
                    direct_resp |= self.calc_operations(ctx, root, edit_event, &mut ops);
                }
            }
        }

        self.renderer.buffer.queue(ops);
        let mut buf_resp = direct_resp;
        buf_resp |= self.renderer.buffer.update();

        if buf_resp.text_updated {
            self.renderer.layout_cache.invalidate_text_change();
            // reparse to refresh bounds; the new root isn't needed here —
            // `show` re-parses for rendering.
            self.renderer.reparse(&arena);
        }

        // Populate reveal_ranges with the current selection. The caller may
        // append additional ranges (e.g. Editor appends the find-match range)
        // after this returns and before `show` — those append-only additions
        // survive into render. `show` itself doesn't touch reveal_ranges.
        self.renderer.reveal_ranges.clear();
        if !self.renderer.readonly && ctx.memory(|m| m.has_focus(id)) {
            self.renderer
                .reveal_ranges
                .push(self.renderer.buffer.current.selection);
        }
        self.renderer.text_highlight_range = self
            .emoji_completions
            .search_term_range
            .or(self.link_completions.search_term_range);

        buf_resp
    }

    /// Draw phase: register the widget for pointer hit-test, process pointer
    /// and context-menu events (piggybacking on the fresh `Response`), then
    /// render.
    pub fn show(&mut self, ui: &mut Ui, rect: Rect, id: Id) {
        self.renderer.dark_mode = ui.style().visuals.dark_mode;
        self.renderer.width = rect.width();
        self.renderer.viewport_height = ui.clip_rect().height();

        ui.ctx().check_for_id_clash(id, rect, "");
        let prev_focused = ui.memory(|m| m.has_focus(id));
        let response = ui.interact(
            rect,
            id,
            if self.renderer.touch_mode { Sense::click() } else { Sense::click_and_drag() },
        );
        // interact surrenders focus if anything was clicked, even a child —
        // restore if we thought we were focused at entry
        if prev_focused && !ui.memory(|m| m.has_focus(id)) {
            ui.memory_mut(|m| m.request_focus(id));
        }
        let focused = ui.memory(|m| m.has_focus(id));

        let response_properly_clicked = response.clicked_by(egui::PointerButton::Primary);
        if response.hovered() || response_properly_clicked {
            ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Text);
        }

        // Re-parse for rendering. handle_input already parsed and computed
        // bounds; `clear()` or other inter-phase mutations by the caller
        // could have left those stale, so recompute.
        let arena = Arena::new();
        let root = self.renderer.reparse(&arena);

        let mut ops = Vec::new();

        // --- context menu (desktop only) -------------------------------------
        ui.ctx()
            .style_mut(|s| s.spacing.menu_margin = egui::vec2(10., 5.).into());
        ui.ctx()
            .style_mut(|s| s.visuals.menu_corner_radius = egui::CornerRadius::same(2));
        ui.ctx()
            .style_mut(|s| s.visuals.window_fill = s.visuals.extreme_bg_color);
        ui.ctx()
            .style_mut(|s| s.visuals.window_stroke = Stroke::NONE);
        if !cfg!(target_os = "ios") && !cfg!(target_os = "android") {
            let readonly = self.renderer.readonly;
            let mut menu_events: Vec<Event> = Vec::new();
            response.context_menu(|ui| {
                ui.horizontal(|ui| {
                    ui.set_min_height(30.);
                    ui.style_mut().spacing.button_padding = egui::vec2(5.0, 5.0);

                    if IconButton::new(Icon::CONTENT_CUT)
                        .tooltip("Cut")
                        .disabled(readonly)
                        .show(ui)
                        .clicked()
                    {
                        menu_events.push(Event::Cut);
                        ui.close();
                    }
                    ui.add_space(5.);
                    if IconButton::new(Icon::CONTENT_COPY)
                        .tooltip("Copy")
                        .show(ui)
                        .clicked()
                    {
                        menu_events.push(Event::Copy);
                        ui.close();
                    }
                    ui.add_space(5.);
                    if IconButton::new(Icon::CONTENT_PASTE)
                        .tooltip("Paste")
                        .disabled(readonly)
                        .show(ui)
                        .clicked()
                    {
                        ui.ctx().send_viewport_cmd(ViewportCommand::RequestPaste);
                        ui.close();
                    }
                });
            });
            for ev in menu_events {
                self.calc_operations(ui.ctx(), root, ev, &mut ops);
            }
        }

        // --- pointer → selection change ---------------------------------------
        // Reads last-frame galleys to resolve pos → offset. Skipped when
        // galleys is empty (first frame / empty doc) — the click will land
        // next frame once render populates galleys.
        let modifiers = ui.ctx().input(|i| i.modifiers);
        let ctx = ui.ctx().clone();
        let have_galleys = !self.renderer.galleys.galleys.is_empty();
        if have_galleys {
            if let Some(pos) = response.interact_pointer_pos() {
                let location = Location::Pos(pos);

                // deliberate order; double click is also click
                let region_opt: Option<Region> = if response.double_clicked()
                    || response.triple_clicked()
                {
                    // egui triple click detection is flaky; treat non-empty
                    // selection double-click as triple-click (paragraph).
                    if cfg!(target_os = "android") {
                        // android native context menu: set position from the
                        // word that will be selected
                        let offset = self.location_to_char_offset(location);
                        let range = offset
                            .range_bound(Bound::Word, true, true, &self.renderer.bounds)
                            .unwrap_or((offset, offset));
                        ctx.set_context_menu(self.context_menu_pos(range).unwrap_or(pos));
                        Some(Region::BoundAt { bound: Bound::Word, location, backwards: true })
                    } else if self.renderer.buffer.current.selection.is_empty() {
                        Some(Region::BoundAt { bound: Bound::Word, location, backwards: true })
                    } else {
                        Some(Region::BoundAt { bound: Bound::Paragraph, location, backwards: true })
                    }
                } else if response.clicked() && modifiers.shift {
                    Some(Region::ToLocation(location))
                } else if response.clicked() {
                    if cfg!(target_os = "android") {
                        // android native context menu: tap-on-selection opens the menu
                        let offset = self.pos_to_char_offset(pos);
                        if self
                            .renderer
                            .buffer
                            .current
                            .selection
                            .contains(offset, true, true)
                        {
                            ctx.set_context_menu(
                                self.context_menu_pos(self.renderer.buffer.current.selection)
                                    .unwrap_or(pos),
                            );
                            None
                        } else {
                            Some(Region::Location(location))
                        }
                    } else {
                        Some(Region::Location(location))
                    }
                } else if response.secondary_clicked() {
                    ctx.set_context_menu(pos);
                    None
                } else if response.drag_stopped() {
                    std::mem::take(&mut self.in_progress_selection).map(Region::from)
                } else if response.dragged() && modifiers.shift {
                    self.in_progress_selection =
                        Some(self.region_to_range(Region::ToLocation(location)));
                    None
                } else if response.dragged() {
                    if response.drag_started() {
                        // capture drag origin on first frame so auto-scroll doesn't pull it
                        let drag_origin =
                            ctx.input(|i| i.pointer.press_origin()).unwrap_or_default();
                        self.in_progress_selection = Some(
                            self.region_to_range(Region::Location(Location::Pos(drag_origin))),
                        );
                    }
                    let offset = self.location_to_char_offset(location);
                    if let Some(sel) = &mut self.in_progress_selection {
                        sel.1 = offset;
                    }
                    None
                } else {
                    None
                };

                // iOS handles cursor placement via virtual-keyboard FFI
                if !cfg!(target_os = "ios") {
                    if let Some(region) = region_opt {
                        ui.memory_mut(|m| m.request_focus(id));
                        self.calc_operations(ui.ctx(), root, Event::Select { region }, &mut ops);
                    }
                }
            }
        }

        self.renderer.buffer.queue(ops);
        self.renderer.buffer.update();
        // Pointer only emits Select ops (no text change), so no re-parse
        // needed. A pointer-driven selection change means reveal_ranges (set
        // in handle_input) reflects the pre-click selection — the new
        // selection appears in reveal_ranges next frame. Accepted lag.

        self.renderer.in_progress_selection = self.in_progress_selection;

        // --- render -----------------------------------------------------------
        self.renderer.galleys.galleys.clear();
        self.renderer.bounds.wrap_lines.clear();
        self.renderer.text_areas.clear();

        let height = self.renderer.height(root);
        let render_rect = Rect::from_min_size(rect.min, Vec2::new(rect.width(), height));
        ui.scope_builder(UiBuilder::new().max_rect(render_rect), |ui| {
            self.renderer.show_block(ui, root, rect.min);
        });
        self.renderer.galleys.galleys.sort_by_key(|g| g.range);

        // cursor / selection — iOS draws natively
        if ui.ctx().os() != OperatingSystem::IOS && !self.renderer.readonly {
            let selection = self
                .in_progress_selection
                .unwrap_or(self.renderer.buffer.current.selection);
            let theme = self.renderer.ctx.get_lb_theme();
            let color = theme.bg().get_color(theme.prefs().primary);
            self.show_range(ui, selection, color.lerp_to_gamma(theme.neutral_bg(), 0.7));
            self.show_offset(ui, selection.1, color);

            if focused {
                if let Some([top, bot]) = self.cursor_line(selection.1) {
                    let cursor_rect = Rect::from_min_max(top, bot);
                    ui.output_mut(|o| {
                        o.ime = Some(egui::output::IMEOutput { rect, cursor_rect });
                    });
                }
            }
        }

        if ui.ctx().os() == OperatingSystem::Android {
            self.show_selection_handles(ui);
        }

        // consume a pending scroll-to-cursor if queued. FindMatch is left to
        // the caller (it owns find state).
        if matches!(self.pending_scroll, Some(ScrollTarget::Cursor)) {
            self.pending_scroll = None;
            self.scroll_to_cursor(ui);
        }

        // lock focus filter so arrow keys / tab / shift+enter keep reaching us
        if focused {
            ui.memory_mut(|m| {
                m.set_focus_lock_filter(
                    id,
                    EventFilter {
                        tab: true,
                        horizontal_arrows: true,
                        vertical_arrows: true,
                        escape: true,
                    },
                );
            });
        }

        // Submit the shaped editor text as one glyphon callback. The caller
        // draws any overlays (find-match highlights, etc.) and then calls
        // show_completions, which submits its own callback on a later layer.
        let text_areas = std::mem::take(&mut self.renderer.text_areas);
        if !text_areas.is_empty() {
            // The callback rect is `clip_rect`, not `max_rect`: this `ui` is
            // inside the editor's `ScrollArea + Frame`, and its `max_rect`
            // scrolls off-screen once the user passes one viewport-height.
            // `egui_wgpu` clamps the callback rect to the screen, and a
            // zero-area result silently drops the callback — no text paints.
            ui.painter()
                .add(egui_wgpu_renderer::egui_wgpu::Callback::new_paint_callback(
                    ui.clip_rect(),
                    crate::GlyphonRendererCallback::new(text_areas),
                ));
        }

        // drain renderer's interactive-element events into the edit queue for next frame
        self.event
            .internal_events
            .append(&mut self.renderer.render_events);
    }

    /// Measure the rendered height at the given `width` without drawing.
    /// Chat composers call this between `handle_input` and `show` to size
    /// the composer bubble for this frame — otherwise the bubble tracks
    /// content a frame late. The parse is cheap (~µs); `height()` is
    /// memoized by `LayoutCache`, so the subsequent `show` re-uses the
    /// cached work.
    pub fn measure_height(&mut self, width: f32) -> f32 {
        self.renderer.width = width;
        let arena = Arena::new();
        let root = self.renderer.reparse(&arena);
        self.renderer.height(root)
    }

    /// Render active completion popups. Caller invokes this after the main
    /// text paint callback has been submitted, and outside any scroll-area
    /// clip — so popup text lands on a later glyphon layer and can extend
    /// past the editor's viewport (e.g. over a toolbar above the cursor).
    pub fn show_completions(&mut self, ui: &mut Ui) {
        if self.renderer.plaintext {
            return;
        }
        self.show_emoji_completions(ui);
        self.show_link_completions(ui);
    }

    /// Reset the buffer to empty state. Chat composers call this after
    /// capturing outgoing text in response to `handle_input` returning
    /// `true`.
    pub fn clear(&mut self) {
        self.renderer.buffer = Buffer::from("");
        self.renderer.layout_cache.invalidate_text_change();
        self.in_progress_selection = None;
        self.event.internal_events.clear();
    }

    /// Center-top of the first wrap line of the given range — where a
    /// context menu should anchor. Android uses this on tap-in-selection.
    pub fn context_menu_pos(&self, range: (Grapheme, Grapheme)) -> Option<Pos2> {
        let lines = self
            .renderer
            .bounds
            .wrap_lines
            .find_intersecting(range, false);
        let first_line = lines.iter().next()?;
        let mut line = self.renderer.bounds.wrap_lines[first_line];
        if line.0 < range.start() {
            line.0 = range.start();
        }
        if line.1 > range.end() {
            line.1 = range.end();
        }

        let start_line = self.cursor_line(line.0)?;
        let end_line = self.cursor_line(line.1)?;
        Some(Pos2 { x: (start_line[1].x + end_line[1].x) / 2., y: start_line[0].y })
    }
}
