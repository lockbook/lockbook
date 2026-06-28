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
use egui::{Context, EventFilter, Id, Pos2, Rect, Sense, Stroke, Ui, UiBuilder, ViewportCommand};
use lb_rs::model::text::buffer::{self, Buffer};
use lb_rs::model::text::offset_types::{Grapheme, RangeExt as _, RangeIterExt as _};

use crate::tab::ExtendedOutput as _;
use crate::tab::markdown_editor::ScrollTarget;
use crate::tab::markdown_editor::bounds::{BoundExt as _, RangesExt as _};
use crate::theme::icons::Icon;
use crate::theme::palette_v2::ThemeExt as _;
use crate::widgets::IconButton;

use super::MdEdit;
use super::input::cursor::SELECTION_HANDLE_HEIGHT;
use super::input::{Bound, Event, Location, Region};

/// Hand-off between [`MdEdit::pre_render`] and [`MdEdit::post_render`].
pub struct PreRenderState {
    focused: bool,
    /// Painter slot reserved before block rendering so the selection
    /// highlight paints *behind* the block content drawn afterwards
    /// (bullets, quote bars, checkboxes — painted immediately, not via
    /// the deferred glyph callback). Filled in `post_render`.
    selection_shape: egui::layers::ShapeIdx,
}

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
        // Undo/redo restore the selection exactly, possibly into folded
        // contents (undoing an auto-unfold restores both the tag and the
        // triggering selection); the fold check below stands down for the
        // frame so undo isn't immediately re-undone.
        let mut undo_redo = false;

        for event in std::mem::take(&mut self.event.internal_events) {
            undo_redo |= matches!(event, Event::Undo | Event::Redo);
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
                    undo_redo |= matches!(edit_event, Event::Undo | Event::Redo);
                    direct_resp |= self.calc_operations(ctx, root, edit_event, &mut ops);
                }
            }
        }

        self.renderer.buffer.queue(ops);
        let mut buf_resp = direct_resp;
        buf_resp |= self.renderer.buffer.update();

        if buf_resp.text_updated {
            self.renderer.bump_text_seq();
            self.renderer.reparse(&arena);
        }

        // selections are automatically snapped out of fold sections but
        // sometimes you can still end up in them e.g. by indenting an item into
        // a folded item
        if (buf_resp.text_updated || buf_resp.selection_user_moved)
            && !undo_redo
            && !self.renderer.readonly
            && !self.renderer.plaintext
        {
            buf_resp |= self.unfold_at_selection(&arena);
        }

        let new_reveal_selection = (!self.renderer.readonly && ctx.memory(|m| m.has_focus(id)))
            .then_some(self.renderer.buffer.current.selection);
        if self.renderer.reveal_selection != new_reveal_selection {
            self.renderer.reveal_selection = new_reveal_selection;
            self.renderer.reveal_seq = self
                .renderer
                .ws_seq
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        self.renderer.search_range = self
            .emoji_completions
            .search_term_range
            .or(self.link_completions.search_term_range);

        buf_resp
    }

    /// Draw the document inline at `rect`: pointer + context menu via
    /// [`MdEdit::pre_render`], paint every block, then cursor /
    /// selection / IME / scroll-to-cursor / text callback via
    /// [`MdEdit::post_render`].
    pub fn show(&mut self, ui: &mut Ui, rect: Rect, id: Id) {
        let arena = Arena::new();
        let root = self.renderer.reparse(&arena);
        let pre = self.pre_render(ui, rect, id, root);

        self.renderer.fragments.clear();
        self.renderer.block_boxes.clear();
        self.renderer.in_progress_block_drag = self.in_progress_block_drag;
        self.renderer.bounds.wrap_lines.clear();
        self.renderer.text_areas.clear();
        self.renderer.deco_lines.clear();
        let height = self.renderer.height(root);
        let render_rect = Rect::from_min_size(rect.min, egui::Vec2::new(rect.width(), height));
        ui.scope_builder(UiBuilder::new().max_rect(render_rect), |ui| {
            self.renderer.show_block(ui, root, rect.min);
        });
        self.renderer.fragments.sort_by_key(|f| f.source_range);

        self.handle_block_drag(ui);
        self.post_render(ui, rect, id, pre);
        self.draw_dragged_overlay(ui, root);
    }

    /// Consume the marker's [`BlockDragAction`] for the frame and, on
    /// release, commit the reorder via [`MdEdit::move_block`].
    pub(crate) fn handle_block_drag(&mut self, ui: &mut Ui) {
        use crate::tab::markdown_editor::widget::block::drag::BlockDragAction;

        match self.renderer.block_drag_action.take() {
            Some(BlockDragAction::Started(drag)) => {
                self.in_progress_block_drag = Some(drag);
                // Select what's being dragged so the selection follows
                // the move. Shift extends the existing selection — works
                // through a click-that-becomes-a-tiny-drag, so a bare
                // shift+click respects the modifier too.
                let shift = ui.input(|i| i.modifiers.shift);
                let region = if shift {
                    let sel = self.renderer.buffer.current.selection;
                    let lo = sel.start().min(drag.section_range.start());
                    let hi = sel.end().max(drag.section_range.end());
                    (lo, hi)
                } else {
                    drag.section_range
                };
                self.renderer
                    .render_events
                    .push(Event::Select { region: region.into() });
            }
            Some(BlockDragAction::Dragged(_)) => {}
            Some(BlockDragAction::Released(pointer)) => {
                if let Some(drag) = self.in_progress_block_drag.take() {
                    if let Some(gap) = self.renderer.drop_gap_for(&drag, pointer) {
                        self.move_block(drag.section_range, gap.insert_offset);
                    }
                }
            }
            None => {}
        }
        if self.in_progress_block_drag.is_some() {
            self.auto_scroll_to_drag_pointer(ui);
            ui.ctx().request_repaint();
        }
    }

    /// Edge-scroll while a block drag is in flight: reveal the pointer
    /// padded by `row_height` on each side. The pointer y is clamped to
    /// the viewport so dragging past the toolbar / window edge still
    /// scrolls at full rate.
    fn auto_scroll_to_drag_pointer(&mut self, ui: &mut Ui) {
        use crate::tab::markdown_editor::scroll_content::DocScrollContent;
        use crate::widgets::affine_scroll::{Align, Reveal};

        let Some(pointer) = ui.input(|i| i.pointer.latest_pos()) else { return };
        let viewport = ui.clip_rect();
        let viewport_y = (pointer.y - viewport.min.y).clamp(0.0, viewport.height());
        let pad = self.renderer.layout.row_height;

        let arena = comrak::Arena::new();
        let root = self.renderer.reparse(&arena);
        let content = DocScrollContent::for_frame(&self.renderer, root, viewport.height());
        let Some(top) = self
            .scroll_area
            .state
            .offset_at_viewport_y(&content, viewport_y - pad)
        else {
            return;
        };
        let Some(bottom) = self
            .scroll_area
            .state
            .offset_at_viewport_y(&content, viewport_y + pad)
        else {
            return;
        };
        self.scroll_area
            .reveal(&content, Reveal { top, bottom }, Align::Nearest);
    }

    /// Drop indicator + source cutout + floating card. Must run after
    /// `post_render` so its paint and second glyph callback composite
    /// above the main text — same ordering trick `show_completions` uses.
    pub(crate) fn draw_dragged_overlay<'a>(
        &mut self, ui: &mut Ui, root: &'a comrak::nodes::AstNode<'a>,
    ) {
        let Some(drag) = self.in_progress_block_drag else { return };
        let Some(p) = ui.input(|i| i.pointer.latest_pos()) else { return };
        let theme = self.renderer.ctx.get_lb_theme();
        let primary = theme.bg().get_color(theme.prefs().primary);

        // Drop-gap indicator: soft capsule with a hollow leading dot.
        if let Some(gap) = self.renderer.drop_gap_for(&drag, p) {
            let (x0, x1) = self
                .renderer
                .block_boxes
                .iter()
                .filter(|b| b.parent_start == drag.parent_start)
                .fold((f32::MAX, f32::MIN), |(a, b), bx| {
                    (a.min(bx.rect.left()), b.max(bx.rect.right()))
                });
            if x0 <= x1 {
                // Start in the gutter so the dot isn't hidden by markers.
                let x0 = x0 - self.renderer.layout.indent;
                let y = gap.y as f32;
                let dot = 4.0;
                let half = 1.25;
                let painter = ui.painter();
                painter.rect_filled(
                    egui::Rect::from_min_max(Pos2::new(x0, y - 3.0), Pos2::new(x1, y + 3.0)),
                    egui::CornerRadius::same(3),
                    primary.gamma_multiply(0.18),
                );
                painter.rect_filled(
                    egui::Rect::from_min_max(
                        Pos2::new(x0 + dot, y - half),
                        Pos2::new(x1, y + half),
                    ),
                    egui::CornerRadius::same(1),
                    primary,
                );
                painter.circle_filled(Pos2::new(x0 + dot, y), dot, primary);
                painter.circle_filled(Pos2::new(x0 + dot, y), dot - 1.5, theme.neutral_bg());
            }
        }

        let Some(src_rect) = self.renderer.section_rect(drag.section_range) else { return };
        // Vertical pad covers content that overflows the section rect
        // (taller cursor row, code/spoiler outlines).
        let vpad = self.renderer.layout.block_spacing / 2.0;
        let card_rect = src_rect.expand2(egui::Vec2::new(0.0, vpad));
        let hole = src_rect.expand2(egui::Vec2::new(0.0, vpad));

        // Cutout with a CSS-`box-shadow: inset` on top + left edges:
        // `Shadow::as_shape` only blurs outward, so place each source
        // rect outside the hole, flush against it, and clip the
        // painter — the blur fades from the rim inward.
        let radius = egui::CornerRadius::same(3);
        ui.painter()
            .rect_filled(hole, radius, theme.neutral_bg_secondary());
        let alpha_top: u8 = if self.renderer.dark_mode { 70 } else { 28 };
        let alpha_left: u8 = if self.renderer.dark_mode { 45 } else { 18 };
        let blur: u8 = 18;
        let inset = ui.painter().with_clip_rect(hole);
        let top_source = egui::Rect::from_min_max(
            Pos2::new(hole.left() - 60.0, hole.top() - 60.0),
            Pos2::new(hole.right() + 60.0, hole.top()),
        );
        inset.add(
            egui::epaint::Shadow {
                offset: [0, 0],
                blur,
                spread: 0,
                color: egui::Color32::from_black_alpha(alpha_top),
            }
            .as_shape(top_source, egui::CornerRadius::ZERO),
        );
        let left_source = egui::Rect::from_min_max(
            Pos2::new(hole.left() - 60.0, hole.top() - 60.0),
            Pos2::new(hole.left(), hole.bottom() + 60.0),
        );
        inset.add(
            egui::epaint::Shadow {
                offset: [0, 0],
                blur,
                spread: 0,
                color: egui::Color32::from_black_alpha(alpha_left),
            }
            .as_shape(left_source, egui::CornerRadius::ZERO),
        );

        // Floating card translated so the grab-point stays under the
        // pointer regardless of mid-drag scroll.
        let offset = (p - src_rect.left_top()) - drag.grab_offset;
        let card = card_rect.translate(offset);
        // Flat canvas-island styling; values match `svg_editor::mod.rs`.
        let card_corner = egui::CornerRadius::same(4);
        let (fill, stroke_color) = if self.renderer.dark_mode {
            (egui::Color32::from_rgb(30, 30, 30), egui::Color32::from_rgb(56, 56, 56))
        } else {
            (ui.visuals().extreme_bg_color, egui::Color32::from_rgb(235, 235, 235))
        };
        ui.painter().rect_filled(card, card_corner, fill);
        ui.painter().rect_stroke(
            card,
            card_corner,
            Stroke::new(0.5, stroke_color),
            egui::epaint::StrokeKind::Inside,
        );

        // Re-draw the span translated; submit a second callback above
        // the main text. Discard the fragments/block_boxes the float
        // produces — keeping them would corrupt hit-testing.
        let frag_len = self.renderer.fragments.len();
        let box_len = self.renderer.block_boxes.len();
        let section = drag.section_range;
        // Top-most items in the span; descendants paint via `show_block`.
        let in_span: Vec<crate::tab::markdown_editor::widget::block::drag::BlockBox> = self
            .renderer
            .block_boxes
            .iter()
            .filter(|b| {
                b.node_range.start() >= section.start() && b.node_range.end() <= section.end()
            })
            .copied()
            .collect();
        let constituents: Vec<(egui::Rect, (Grapheme, Grapheme))> = in_span
            .iter()
            .filter(|b| {
                !in_span.iter().any(|o| {
                    o.node_range != b.node_range
                        && o.node_range.start() <= b.node_range.start()
                        && b.node_range.end() <= o.node_range.end()
                })
            })
            .map(|b| (b.rect, b.node_range))
            .collect();
        ui.push_id("md_block_drag_float", |ui| {
            for (rect, nr) in &constituents {
                if let Some(node) = root
                    .descendants()
                    .find(|n| self.renderer.node_range(n) == *nr)
                {
                    self.renderer.show_block(ui, node, rect.min + offset);
                }
            }
        });
        let floating_text = std::mem::take(&mut self.renderer.text_areas);
        let floating_deco = std::mem::take(&mut self.renderer.deco_lines);
        self.renderer.fragments.truncate(frag_len);
        self.renderer.block_boxes.truncate(box_len);

        if !floating_text.is_empty() {
            ui.painter()
                .add(egui_wgpu_renderer::egui_wgpu::Callback::new_paint_callback(
                    ui.clip_rect(),
                    crate::GlyphonRendererCallback::new(floating_text),
                ));
        }
        for d in floating_deco {
            ui.painter().hline(d.x, d.y, Stroke::new(1.0, d.color));
        }
    }

    /// Per-frame setup that runs before block rendering: dark-mode +
    /// viewport snapshot, pointer hit-test, context menu, pointer →
    /// selection. Returns the focus state for [`MdEdit::post_render`].
    pub fn pre_render<'a>(
        &mut self, ui: &mut Ui, rect: Rect, id: Id, root: &'a comrak::nodes::AstNode<'a>,
    ) -> PreRenderState {
        self.renderer.dark_mode = ui.style().visuals.dark_mode;
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

        // Per-fragment interacts must come after the editor's main
        // interact above so they sit on top in z-order.
        self.renderer.interact_fragments(ui);
        self.renderer.handle_link_interactions(root, ui);
        self.renderer.handle_spoiler_interactions(root, ui);
        self.renderer.handle_fold_interactions(ui);

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
        // Reads last-frame fragments to resolve pos → offset. Skipped
        // when fragments is empty (first frame / empty doc) — the
        // click will land next frame once render populates them.
        let modifiers = ui.ctx().input(|i| i.modifiers);
        let ctx = ui.ctx().clone();
        let have_galleys = !self.renderer.fragments.is_empty();
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
                        ctx.set_context_menu(
                            self.context_menu_pos(range, rect.intersect(ui.clip_rect()))
                                .unwrap_or(pos),
                        );
                        Some(Region::BoundAt { bound: Bound::Word, location, backwards: true })
                    } else if self.renderer.buffer.current.selection.is_empty() {
                        Some(Region::BoundAt { bound: Bound::Word, location, backwards: true })
                    } else {
                        Some(Region::BoundAt { bound: Bound::Paragraph, location, backwards: true })
                    }
                } else if response.clicked() && modifiers.shift && !cfg!(target_os = "android") {
                    Some(Region::ToLocation(location))
                } else if response.clicked() && self.scroll_area.momentum_cancel_press() {
                    None
                } else if response.clicked() {
                    if cfg!(target_os = "android") && self.selection_tap(pos) {
                        let selection = self.renderer.buffer.current.selection;
                        ctx.set_context_menu(
                            self.context_menu_pos(selection, rect.intersect(ui.clip_rect()))
                                .unwrap_or(pos),
                        );
                        None
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
                    self.pending_scroll = Some(ScrollTarget::Cursor);
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
                    self.pending_scroll = Some(ScrollTarget::Cursor);
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

        // Reserved before block content so the highlight sits behind it;
        // see the `selection_shape` field.
        let selection_shape = ui.painter().add(egui::Shape::Noop);

        PreRenderState { focused, selection_shape }
    }

    /// Per-frame teardown that runs after block rendering: cursor /
    /// selection / IME, touch handles, scroll-to-cursor consumption,
    /// focus lock, glyphon text-callback submission, draining of
    /// internal events.
    pub fn post_render(&mut self, ui: &mut Ui, rect: Rect, id: Id, pre: PreRenderState) {
        let focused = pre.focused;

        // Clip subsequent overlay paints (selection, cursor) to the
        // editor rect so they don't bleed over toolbars or sidebars.
        ui.set_clip_rect(rect.intersect(ui.clip_rect()));

        // cursor / selection — iOS draws natively
        if ui.ctx().os() != OperatingSystem::IOS && !self.renderer.readonly {
            let selection = self
                .in_progress_selection
                .unwrap_or(self.renderer.buffer.current.selection);
            let theme = self.renderer.ctx.get_lb_theme();
            let color = theme.bg().get_color(theme.prefs().primary);

            // Fill the reserved slot so the highlight paints behind the
            // markers (drawn during block render); the cursor stays on top.
            let sel_color = color.lerp_to_gamma(theme.neutral_bg(), 0.7);
            let shapes: Vec<egui::Shape> = self
                .range_rects(selection)
                .into_iter()
                .map(|r| egui::Shape::rect_filled(r, 2.0, sel_color))
                .collect();
            ui.painter()
                .set(pre.selection_shape, egui::Shape::Vec(shapes));
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

        // FindMatch is consumed by the caller (it owns find state).
        if matches!(self.pending_scroll, Some(ScrollTarget::Cursor)) {
            self.pending_scroll = None;
            self.scroll_to_cursor(rect);
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

        // strikethroughs and underlines painted on top of text
        for deco in std::mem::take(&mut self.renderer.deco_lines) {
            ui.painter()
                .hline(deco.x, deco.y, Stroke::new(1.0, deco.color));
        }

        let has_selection_handles = !self.renderer.buffer.current.selection.is_empty()
            || self.in_progress_selection.is_some();
        if ui.ctx().os() == OperatingSystem::Android && has_selection_handles {
            self.show_selection_handles(ui);
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
        self.renderer.set_width(width);
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
        self.renderer.bump_text_seq();
        self.in_progress_selection = None;
        self.event.internal_events.clear();
    }

    /// Center-top of the first wrap line of the given range — where a
    /// context menu should anchor. Android uses this on tap-in-selection.
    pub fn context_menu_pos(
        &self, range: (Grapheme, Grapheme), visble_rect: egui::Rect,
    ) -> Option<Pos2> {
        let lines = self
            .renderer
            .bounds
            .wrap_lines
            .find_intersecting(range, false);
        let first_line = lines.iter().next()?;
        let last_line = lines.iter().next_back()?;

        let mut first_line = self.renderer.bounds.wrap_lines[first_line];
        if first_line.0 < range.start() {
            first_line.0 = range.start();
        }
        if first_line.1 > range.end() {
            first_line.1 = range.end();
        }

        let mut last_line = self.renderer.bounds.wrap_lines[last_line];
        if last_line.0 < range.start() {
            last_line.0 = range.start();
        }
        if last_line.1 > range.end() {
            last_line.1 = range.end();
        }

        let start_line = self.cursor_line(first_line.0)?;
        let end_line = self.cursor_line(last_line.1)?;

        let context_menu_approx_height = 65.0; // might be a bit falky if the user has a custom system font size in android, but good enough for now

        let anchor_y = if visble_rect.contains(start_line[1]) {
            start_line[1].y - SELECTION_HANDLE_HEIGHT
        } else {
            end_line[1].y + context_menu_approx_height + SELECTION_HANDLE_HEIGHT
        };

        Some(Pos2 { x: (start_line[1].x + end_line[1].x) / 2., y: anchor_y })
    }
}
