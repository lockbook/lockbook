use std::sync::{Arc, Mutex};
use web_time::Duration;

use egui::{Context, Event, EventFilter, Id, ImeEvent, Key, Rect, Response, Sense, Ui};
use glyphon::{Attrs, Family, FontSystem, Metrics, Shaping};

use crate::theme::palette_v2::ThemeExt as _;
use crate::widgets::glyphon_cache::{GlyphonCache, GlyphonCacheKey, GlyphonFontFamily};

const EVENT_FILTER: EventFilter =
    EventFilter { horizontal_arrows: true, vertical_arrows: false, tab: false, escape: false };

/// Apply one editor event to `state` and `text`.
///
/// Returns `(text_changed, submitted)`. When `submitted` is `true` the caller
/// should surrender focus on the widget id so that `lost_focus()` fires.
fn apply_event(
    event: Event, state: &mut State, text: &mut String, now: f64, ctx: &Context,
) -> (bool, bool) {
    let (mut changed, mut submitted) = (false, false);
    match event {
        Event::Key { key: Key::Enter, pressed: true, .. } => {
            submitted = true;
        }
        Event::Text(s) => {
            if state.has_selection() {
                state.delete_selection(text);
            }
            text.insert_str(state.cursor, &s);
            state.cursor += s.len();
            state.anchor = state.cursor;
            state.last_interaction_time = now;
            changed = true;
        }
        Event::Ime(ImeEvent::Commit(s)) => {
            if state.has_selection() {
                state.delete_selection(text);
            }
            text.insert_str(state.cursor, &s);
            state.cursor += s.len();
            state.anchor = state.cursor;
            state.last_interaction_time = now;
            changed = true;
        }
        Event::Key { key: Key::Backspace, pressed: true, modifiers, .. } => {
            if state.has_selection() {
                state.delete_selection(text);
            } else if state.cursor > 0 {
                if modifiers.command {
                    text.drain(0..state.cursor);
                    state.cursor = 0;
                    state.anchor = 0;
                } else {
                    let prev = prev_grapheme_boundary(text, state.cursor);
                    text.drain(prev..state.cursor);
                    state.cursor = prev;
                    state.anchor = prev;
                }
            }
            state.last_interaction_time = now;
            changed = true;
        }
        Event::Key { key: Key::Delete, pressed: true, .. } => {
            if state.has_selection() {
                state.delete_selection(text);
            } else if state.cursor < text.len() {
                let next = next_grapheme_boundary(text, state.cursor);
                text.drain(state.cursor..next);
            }
            state.last_interaction_time = now;
            changed = true;
        }
        Event::Key { key: Key::ArrowLeft, pressed: true, modifiers, .. } => {
            if !modifiers.shift && state.has_selection() {
                let lo = state.selection().0;
                state.move_cursor(lo, false);
            } else if state.cursor > 0 {
                let prev = prev_grapheme_boundary(text, state.cursor);
                state.move_cursor(prev, modifiers.shift);
            }
            state.last_interaction_time = now;
        }
        Event::Key { key: Key::ArrowRight, pressed: true, modifiers, .. } => {
            if !modifiers.shift && state.has_selection() {
                let hi = state.selection().1;
                state.move_cursor(hi, false);
            } else if state.cursor < text.len() {
                let next = next_grapheme_boundary(text, state.cursor);
                state.move_cursor(next, modifiers.shift);
            }
            state.last_interaction_time = now;
        }
        Event::Key { key: Key::Home, pressed: true, modifiers, .. } => {
            state.move_cursor(0, modifiers.shift);
            state.last_interaction_time = now;
        }
        Event::Key { key: Key::End, pressed: true, modifiers, .. } => {
            state.move_cursor(text.len(), modifiers.shift);
            state.last_interaction_time = now;
        }
        Event::Key { key: Key::A, pressed: true, modifiers, .. } if modifiers.command => {
            state.anchor = 0;
            state.cursor = text.len();
            state.last_interaction_time = now;
        }
        Event::Copy => {
            if state.has_selection() {
                let (lo, hi) = state.selection();
                ctx.copy_text(text[lo..hi].to_owned());
            }
        }
        Event::Cut => {
            if state.has_selection() {
                let (lo, hi) = state.selection();
                ctx.copy_text(text[lo..hi].to_owned());
                state.delete_selection(text);
                state.last_interaction_time = now;
                changed = true;
            }
        }
        Event::Paste(s) => {
            if state.has_selection() {
                state.delete_selection(text);
            }
            text.insert_str(state.cursor, &s);
            state.cursor += s.len();
            state.anchor = state.cursor;
            state.last_interaction_time = now;
            changed = true;
        }
        _ => {}
    }
    (changed, submitted)
}

#[derive(Clone, Default)]
struct State {
    /// Moving end of the selection range (the caret).
    cursor: usize,
    /// Fixed end of the selection range. Equal to `cursor` when there is no selection.
    anchor: usize,
    /// Horizontal scroll offset in logical pixels. Positive shifts the view right,
    /// revealing text that starts left of the widget origin.
    singleline_offset: f32,
    /// Timestamp of the last edit or cursor movement, used to reset the blink phase.
    last_interaction_time: f64,
    /// Whether the widget was focused last frame, used to detect focus-gained.
    was_focused: bool,
}

impl State {
    fn selection(&self) -> (usize, usize) {
        (self.cursor.min(self.anchor), self.cursor.max(self.anchor))
    }

    fn has_selection(&self) -> bool {
        self.cursor != self.anchor
    }

    fn delete_selection(&mut self, text: &mut String) {
        let (lo, hi) = self.selection();
        text.drain(lo..hi);
        self.cursor = lo;
        self.anchor = lo;
    }

    fn move_cursor(&mut self, to: usize, extend: bool) {
        self.cursor = to;
        if !extend {
            self.anchor = to;
        }
    }
}

/// A single-line text-edit widget that uses glyphon for shaping so that emoji
/// and complex scripts render correctly.
pub struct GlyphonTextEdit<'a> {
    text: &'a mut String,
    font_size: f32,
    line_height: Option<f32>,
    cursor_at_end: bool,
    /// Selection to apply whenever focus is newly gained.
    focus_selection: Option<(usize, usize)>,
    id: Option<Id>,
    hint_text: Option<String>,
}

impl<'a> GlyphonTextEdit<'a> {
    pub fn new(text: &'a mut String) -> Self {
        Self {
            text,
            font_size: 14.0,
            line_height: None,
            cursor_at_end: false,
            focus_selection: None,
            id: None,
            hint_text: None,
        }
    }

    pub fn font_size(self, font_size: f32) -> Self {
        Self { font_size, ..self }
    }

    pub fn line_height(self, line_height: f32) -> Self {
        Self { line_height: Some(line_height), ..self }
    }

    pub fn cursor_at_end(self) -> Self {
        Self { cursor_at_end: true, ..self }
    }

    pub fn id(self, id: Id) -> Self {
        Self { id: Some(id), ..self }
    }

    pub fn select_all(self) -> Self {
        let len = self.text.len();
        Self { focus_selection: Some((0, len)), ..self }
    }

    pub fn select_on_focus(self, anchor: usize, cursor: usize) -> Self {
        Self { focus_selection: Some((anchor, cursor)), ..self }
    }

    pub fn hint_text(self, hint: impl Into<String>) -> Self {
        Self { hint_text: Some(hint.into()), ..self }
    }

    /// Process keyboard/text events for this widget id without rendering.
    ///
    /// Call this before any sizing that depends on the current text so that
    /// the buffer is up-to-date when the rect is measured.
    ///
    /// Returns `true` if Enter was pressed.
    pub fn process_events(ui: &mut Ui, id: Id, text: &mut String) -> bool {
        if !ui.memory(|m| m.has_focus(id)) {
            return false;
        }

        let now = ui.input(|i| i.time);
        let mut state: State = ui.data(|d| d.get_temp(id)).unwrap_or_default();
        state.cursor = state.cursor.min(text.len());
        state.anchor = state.anchor.min(text.len());

        ui.memory_mut(|m| m.set_focus_lock_filter(id, EVENT_FILTER));

        let events = ui.input_mut(|i| {
            let (matching, remaining): (Vec<_>, Vec<_>) = std::mem::take(&mut i.events)
                .into_iter()
                .partition(|e| EVENT_FILTER.matches(e));
            i.events = remaining;
            matching
        });

        let mut submitted = false;
        for event in events {
            let (_, sub) = apply_event(event, &mut state, text, now, ui.ctx());
            if sub {
                ui.memory_mut(|m| m.surrender_focus(id));
                submitted = true;
            }
        }

        ui.data_mut(|d| d.insert_temp(id, state));
        submitted
    }

    pub fn show(self, ui: &mut Ui) -> Response {
        let id = self.id.unwrap_or_else(|| ui.next_auto_id());
        self.show_impl(ui, id)
    }

    fn show_impl(self, ui: &mut Ui, id: Id) -> Response {
        let font_system = ui
            .ctx()
            .data(|d| d.get_temp::<Arc<Mutex<FontSystem>>>(egui::Id::NULL))
            .expect("cosmic-text font system used before registered");

        let focused = ui.memory(|m| m.has_focus(id));
        let ppi = ui.ctx().pixels_per_point();
        let line_height = self.line_height.unwrap_or(self.font_size * 1.4);
        let now = ui.input(|i| i.time);

        // Restore or initialise per-widget state
        let mut state: State = ui.data(|d| d.get_temp(id)).unwrap_or_else(|| {
            let pos = if self.cursor_at_end { self.text.len() } else { 0 };
            if let Some((anchor, cursor)) = self.focus_selection {
                State { cursor, anchor, ..Default::default() }
            } else {
                State { cursor: pos, anchor: pos, ..Default::default() }
            }
        });
        state.cursor = state.cursor.min(self.text.len());
        state.anchor = state.anchor.min(self.text.len());

        // Re-apply the focus selection whenever focus is newly acquired
        if focused && !state.was_focused {
            if let Some((anchor, cursor)) = self.focus_selection {
                state.anchor = anchor.min(self.text.len());
                state.cursor = cursor.min(self.text.len());
            }
        }
        state.was_focused = focused;

        // Keyboard / text input
        let mut text_changed = false;
        if focused {
            ui.memory_mut(|m| m.set_focus_lock_filter(id, EVENT_FILTER));

            let events = ui.input_mut(|i| i.filtered_events(&EVENT_FILTER));
            for event in events {
                let (changed, submitted) = apply_event(event, &mut state, self.text, now, ui.ctx());
                if changed {
                    text_changed = true;
                }
                if submitted {
                    ui.memory_mut(|m| m.surrender_focus(id));
                }
            }
        }

        let glyphon_cache = ui
            .ctx()
            .data(|d| d.get_temp::<Arc<Mutex<GlyphonCache>>>(egui::Id::NULL))
            .expect("glyphon cache used before registered");

        // Shape the full text on a single unbounded line; singleline_offset scrolls the view
        let buffer = glyphon_cache.lock().unwrap().get_or_shape(
            GlyphonCacheKey::single(
                self.text.as_str(),
                GlyphonFontFamily::SansSerif,
                false,
                false,
                None,
                (self.font_size * ppi).to_bits(),
                (line_height * ppi).to_bits(),
                f32::MAX.to_bits(),
            ),
            || {
                let mut fs = font_system.lock().unwrap();
                let mut buf = glyphon::Buffer::new(
                    &mut fs,
                    Metrics::new(self.font_size * ppi, line_height * ppi),
                );
                buf.set_size(&mut fs, Some(f32::MAX), None);
                buf.set_text(
                    &mut fs,
                    self.text,
                    &Attrs::new().family(Family::SansSerif),
                    Shaping::Advanced,
                );
                buf.shape_until_scroll(&mut fs, false);
                buf
            },
        );

        let buf_read = buffer.read().unwrap();
        let total_text_width = buf_read
            .layout_runs()
            .map(|r| r.line_w)
            .fold(0.0f32, f32::max)
            / ppi;
        let cursor_x = cursor_x_from_buffer(&buf_read, state.cursor, ppi);

        let visible_width = ui.available_width();
        let (rect, _) =
            ui.allocate_exact_size(egui::vec2(visible_width, line_height), Sense::hover());
        let mut response = ui.interact(rect, id, Sense::click_and_drag());

        if response.clicked() || response.drag_started() {
            ui.memory_mut(|m| m.request_focus(id));
        }

        if focused && (response.clicked() || response.drag_started() || response.dragged()) {
            if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
                let buf_x = (pos.x - rect.min.x + state.singleline_offset) * ppi;
                let buf_y = line_height * ppi * 0.5;
                let byte = hit_test_buffer(&buf_read, buf_x, buf_y);
                if response.drag_started() || response.clicked() {
                    state.move_cursor(byte, false);
                } else {
                    state.cursor = byte;
                }
                state.last_interaction_time = now;
            }
        }

        // Scroll to cursor
        if focused {
            if cursor_x < state.singleline_offset {
                state.singleline_offset = cursor_x;
            } else if cursor_x > state.singleline_offset + visible_width {
                state.singleline_offset = cursor_x - visible_width;
            }
            state.singleline_offset = state
                .singleline_offset
                .clamp(0.0, (total_text_width - visible_width).max(0.0));
        }

        if text_changed {
            response.mark_changed();
        }

        if focused {
            let cx =
                (rect.min.x + cursor_x - state.singleline_offset).clamp(rect.min.x, rect.max.x);
            ui.output_mut(|o| {
                o.ime = Some(egui::output::IMEOutput {
                    rect,
                    cursor_rect: Rect::from_min_max(
                        egui::pos2(cx, rect.min.y),
                        egui::pos2(cx + 1.5, rect.max.y),
                    ),
                });
            });
        }

        ui.data_mut(|d| d.insert_temp(id, state.clone()));

        if ui.is_rect_visible(rect) {
            let visuals = ui.style().interact(&response);

            if state.has_selection() {
                let (lo, hi) = state.selection();
                let x0 = (cursor_x_from_buffer(&buf_read, lo, ppi) - state.singleline_offset)
                    .clamp(0.0, visible_width);
                let x1 = (cursor_x_from_buffer(&buf_read, hi, ppi) - state.singleline_offset)
                    .clamp(0.0, visible_width);
                let theme = ui.ctx().get_lb_theme();
                let sel_color = theme
                    .bg()
                    .get_color(theme.prefs().primary)
                    .lerp_to_gamma(theme.neutral_bg(), 0.7);
                ui.painter().rect_filled(
                    Rect::from_min_max(
                        egui::pos2(rect.min.x + x0, rect.min.y + 2.0),
                        egui::pos2(rect.min.x + x1, rect.max.y - 2.0),
                    ),
                    0.0,
                    sel_color,
                );
            }

            // The draw rect slides left by singleline_offset so the visible window
            // scrolls over the full laid-out line. clip_rect keeps text inside the widget.
            let draw_rect = Rect::from_min_size(
                egui::pos2(rect.min.x - state.singleline_offset, rect.min.y),
                egui::vec2(total_text_width.max(visible_width), line_height),
            );

            let mut areas = Vec::new();

            // Show hint text when the text buffer is empty
            if self.text.is_empty() {
                if let Some(ref hint) = self.hint_text {
                    let hint_buf = glyphon_cache.lock().unwrap().get_or_shape(
                        GlyphonCacheKey::single(
                            hint.as_str(),
                            GlyphonFontFamily::SansSerif,
                            false,
                            false,
                            None,
                            (self.font_size * ppi).to_bits(),
                            (line_height * ppi).to_bits(),
                            (visible_width * ppi).to_bits(),
                        ),
                        || {
                            let mut fs = font_system.lock().unwrap();
                            let mut buf = glyphon::Buffer::new(
                                &mut fs,
                                Metrics::new(self.font_size * ppi, line_height * ppi),
                            );
                            buf.set_size(&mut fs, Some(visible_width * ppi), None);
                            buf.set_text(
                                &mut fs,
                                hint,
                                &Attrs::new().family(Family::SansSerif),
                                Shaping::Advanced,
                            );
                            buf.shape_until_scroll(&mut fs, false);
                            buf
                        },
                    );
                    let c = ui.visuals().weak_text_color();
                    areas.push(crate::TextBufferArea::new(
                        hint_buf,
                        Rect::from_min_size(rect.min, egui::vec2(visible_width, line_height)),
                        glyphon::Color::rgba(c.r(), c.g(), c.b(), c.a()),
                        ui.ctx(),
                        rect,
                    ));
                }
            }

            drop(buf_read);
            let c = visuals.text_color();
            areas.push(crate::TextBufferArea::new(
                buffer,
                draw_rect,
                glyphon::Color::rgba(c.r(), c.g(), c.b(), c.a()),
                ui.ctx(),
                rect,
            ));
            // egui_wgpu clamps the callback rect to the screen and drops a zero-area result.
            let callback_rect = rect.intersect(ui.clip_rect());
            ui.painter()
                .add(egui_wgpu_renderer::egui_wgpu::Callback::new_paint_callback(
                    callback_rect,
                    crate::GlyphonRendererCallback::new(areas),
                ));

            if focused {
                let elapsed = now - state.last_interaction_time;
                let blink_on = elapsed < 0.5 || (elapsed * 2.0).fract() < 0.5;
                if blink_on {
                    const CURSOR_W: f32 = 1.5;
                    let cx = (rect.min.x + cursor_x - state.singleline_offset)
                        .clamp(rect.min.x, (rect.max.x - CURSOR_W).max(rect.min.x));
                    ui.painter().rect_filled(
                        Rect::from_min_max(
                            egui::pos2(cx, rect.min.y + 2.0),
                            egui::pos2(cx + CURSOR_W, rect.max.y - 2.0),
                        ),
                        0.0,
                        visuals.text_color(),
                    );
                }
                ui.ctx().request_repaint_after(Duration::from_millis(300));
            }
        }

        if focused && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Escape)) {
            ui.memory_mut(|m| m.surrender_focus(id));
        }

        if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Text);
        }

        response
    }
}

impl egui::Widget for GlyphonTextEdit<'_> {
    fn ui(self, ui: &mut egui::Ui) -> Response {
        self.show(ui)
    }
}

fn prev_grapheme_boundary(s: &str, from: usize) -> usize {
    use unicode_segmentation::UnicodeSegmentation as _;
    s[..from]
        .grapheme_indices(true)
        .next_back()
        .map(|(i, _)| i)
        .unwrap_or(0)
}

fn next_grapheme_boundary(s: &str, from: usize) -> usize {
    use unicode_segmentation::UnicodeSegmentation as _;
    s[from..]
        .graphemes(true)
        .next()
        .map(|g| from + g.len())
        .unwrap_or(s.len())
}

/// Returns the x position (logical pixels, relative to the buffer's left edge)
/// of the cursor at `byte_offset`.
fn cursor_x_from_buffer(buffer: &glyphon::Buffer, byte_offset: usize, ppi: f32) -> f32 {
    for run in buffer.layout_runs() {
        for glyph in run.glyphs.iter() {
            if byte_offset == glyph.start {
                return glyph.x / ppi;
            }
            // Cursor is inside or immediately after a multi-byte cluster.
            if byte_offset > glyph.start && byte_offset <= glyph.end {
                return (glyph.x + glyph.w) / ppi;
            }
        }
        // Cursor is at or past the end of the run.
        if let Some(last) = run.glyphs.last() {
            if byte_offset >= last.end {
                return (last.x + last.w) / ppi;
            }
        }
    }
    0.0
}

/// Returns the byte offset of the character boundary nearest to physical-pixel
/// position `x` measured from the buffer's left edge.
fn hit_test_buffer(buffer: &glyphon::Buffer, x: f32, _y: f32) -> usize {
    for run in buffer.layout_runs() {
        for glyph in run.glyphs.iter() {
            if x < glyph.x + glyph.w / 2.0 {
                return glyph.start;
            }
            if x < glyph.x + glyph.w {
                return glyph.end;
            }
        }
        if let Some(last) = run.glyphs.last() {
            return last.end;
        }
    }
    0
}
