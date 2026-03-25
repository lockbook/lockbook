use std::sync::{Arc, Mutex, RwLock};
use web_time::Duration;

use egui::{Context, Event, EventFilter, Id, ImeEvent, Key, Rect, Response, Sense, Ui};
use glyphon::{Attrs, Family, FontSystem, Metrics, Shaping};

use crate::theme::palette_v2::ThemeExt as _;

// ---------------------------------------------------------------------------
// Shared event filter — identical for both process_events and show_impl
// ---------------------------------------------------------------------------

const EVENT_FILTER: EventFilter =
    EventFilter { horizontal_arrows: true, vertical_arrows: false, tab: false, escape: false };

// ---------------------------------------------------------------------------
// Single authoritative event dispatch
// ---------------------------------------------------------------------------

/// Apply one editor event to `state` and `text`.
///
/// Returns `(text_changed, submitted)`.  When `submitted` is `true` the caller
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

// ---------------------------------------------------------------------------
// Per-widget persistent state (stored in egui's temp data map)
// ---------------------------------------------------------------------------

#[derive(Clone, Default)]
struct State {
    /// Moving end of the selection range (the caret).
    cursor: usize,
    /// Fixed end of the selection range. Equal to `cursor` when there is no selection.
    anchor: usize,
    /// Horizontal scroll in logical pixels. Positive means the view is shifted right
    /// so that text that starts left of the widget origin becomes visible.
    singleline_offset: f32,
    /// Timestamp of the last edit or cursor movement, used to reset the blink phase.
    last_interaction_time: f64,
    /// Whether the widget was focused on the previous frame, used to detect focus-gained.
    was_focused: bool,
}

impl State {
    /// Returns `(lo, hi)` byte offsets of the selection in document order.
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

    /// Move cursor, optionally extending the selection anchor.
    fn move_cursor(&mut self, to: usize, extend: bool) {
        self.cursor = to;
        if !extend {
            self.anchor = to;
        }
    }
}

// ---------------------------------------------------------------------------
// Widget definition
// ---------------------------------------------------------------------------

/// A single-line text-edit widget that uses glyphon for shaping so that emoji
/// and complex scripts render correctly. It has no intrinsic width — it fills
/// whatever rect the caller negotiates with egui (typically via `ui.put`).
pub struct GlyphonTextEdit<'a> {
    text: &'a mut String,
    font_size: f32,
    /// Override the line height used for allocation and glyph metrics.
    ///
    /// When `Some`, the widget allocates exactly this height so that it matches
    /// a sibling [`GlyphonLabel`] shaped with the same value — preventing
    /// `ui.place`'s `centered_and_justified` layout from introducing a vertical
    /// offset when the caller's `body_line_height` differs from `font_size * 1.4`.
    line_height: Option<f32>,
    cursor_at_end: bool,
    /// `Some((anchor, cursor))` — selection to apply whenever focus is gained.
    focus_selection: Option<(usize, usize)>,
    id: Option<Id>,
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
        }
    }

    pub fn font_size(self, size: f32) -> Self {
        Self { font_size: size, ..self }
    }

    /// Fix the row height used for allocation and glyphon metrics.
    ///
    /// Pass the same value used in [`GlyphonLabel::shape_and_measure`] so that
    /// both widgets occupy identical vertical space and `ui.place`'s internal
    /// `centered_and_justified` layout produces zero centering offset.
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

    /// Apply `(anchor, cursor)` selection whenever focus is newly gained.
    /// Used by the tab-rename flow to pre-select the stem of the filename.
    pub fn select_on_focus(self, anchor: usize, cursor: usize) -> Self {
        Self { focus_selection: Some((anchor, cursor)), ..self }
    }

    // -----------------------------------------------------------------------
    // Pre-frame event processing
    // -----------------------------------------------------------------------

    /// Process keyboard/text events for this widget id *without* rendering.
    ///
    /// Call this **before** any sizing that depends on the current text content
    /// (e.g. before computing `text_rect` for a tab rename) so that the text is
    /// up-to-date before the rect is measured. When `show` / the `Widget` impl
    /// subsequently runs, the event queue will already be drained and the input
    /// pass inside `show_impl` becomes a no-op.
    ///
    /// Returns `true` if Enter was pressed (submit signal).
    pub fn process_events(ui: &mut Ui, id: Id, text: &mut String) -> bool {
        if !ui.memory(|m| m.has_focus(id)) {
            return false;
        }

        let now = ui.input(|i| i.time);
        let mut state: State = ui.data(|d| d.get_temp(id)).unwrap_or_default();
        state.cursor = state.cursor.min(text.len());
        state.anchor = state.anchor.min(text.len());

        // Claim horizontal-arrow keys so they don't bubble to the scroll area.
        ui.memory_mut(|m| m.set_focus_lock_filter(id, EVENT_FILTER));

        // Drain only the events we care about; put the rest back.
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

    // -----------------------------------------------------------------------
    // Rendering entry points
    // -----------------------------------------------------------------------

    pub fn show(self, ui: &mut Ui) -> Response {
        let id = self.id.unwrap_or_else(|| ui.next_auto_id());
        self.show_impl(ui, id)
    }

    fn show_impl(self, ui: &mut Ui, id: Id) -> Response {
        // Fall back to a plain egui TextEdit if the glyphon font system is not
        // registered (e.g. in tests or non-wgpu builds).
        let font_system = ui
            .ctx()
            .data(|d| d.get_temp::<Arc<Mutex<FontSystem>>>(egui::Id::NULL));
        let Some(font_system) = font_system else {
            return ui.add(egui::TextEdit::singleline(self.text));
        };

        let focused = ui.memory(|m| m.has_focus(id));
        let ppi = ui.ctx().pixels_per_point();
        // line_height is the full row height including leading; font renders in the middle.
        // Use the caller-supplied value when provided so this widget's allocated height
        // matches an adjacent GlyphonLabel exactly, keeping the text baseline stable
        // across display↔rename transitions.
        let line_height = self.line_height.unwrap_or(self.font_size * 1.4);
        let now = ui.input(|i| i.time);

        // --- Restore or initialise per-widget state ---
        let mut state: State = ui.data(|d| d.get_temp(id)).unwrap_or_else(|| {
            let pos = if self.cursor_at_end { self.text.len() } else { 0 };
            if let Some((anchor, cursor)) = self.focus_selection {
                State { cursor, anchor, ..Default::default() }
            } else {
                State { cursor: pos, anchor: pos, ..Default::default() }
            }
        });
        // Guard against stale offsets after text was changed externally.
        state.cursor = state.cursor.min(self.text.len());
        state.anchor = state.anchor.min(self.text.len());

        // Re-apply the focus selection whenever focus is newly acquired.
        if focused && !state.was_focused {
            if let Some((anchor, cursor)) = self.focus_selection {
                state.anchor = anchor.min(self.text.len());
                state.cursor = cursor.min(self.text.len());
            }
        }
        state.was_focused = focused;

        // --- Keyboard / text input ---
        // This is a no-op when process_events() has already drained the queue
        // this frame, because filtered_events() will find nothing left to take.
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

        // --- Shape ---
        // Layout the full text on a single infinite-width line; singleline_offset
        // provides the scrolling window. We never wrap.
        let buffer = {
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
        };

        let total_text_width = buffer
            .layout_runs()
            .map(|r| r.line_w)
            .fold(0.0f32, f32::max)
            / ppi;
        let cursor_x = cursor_x_from_buffer(&buffer, state.cursor, ppi);

        // --- Allocate ---
        // Fill whatever width the caller gave us. Using available_width() rather
        // than max_rect().width() means the widget correctly sizes to the
        // remaining space when placed inline after other widgets (e.g. after an
        // icon in a ui.horizontal), while still filling the full rect when placed
        // via ui.put (where available_width == max_rect width).
        let visible_width = ui.available_width();
        let desired_size = egui::vec2(visible_width, line_height);
        let (rect, _) = ui.allocate_exact_size(desired_size, Sense::hover());
        let mut response = ui.interact(rect, id, Sense::click_and_drag());

        if response.clicked() || response.drag_started() {
            ui.memory_mut(|m| m.request_focus(id));
        }

        // --- Click / drag to reposition cursor ---
        if focused && (response.clicked() || response.drag_started() || response.dragged()) {
            if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
                let buf_x = (pos.x - rect.min.x + state.singleline_offset) * ppi;
                let buf_y = line_height * ppi * 0.5;
                let byte = hit_test_buffer(&buffer, buf_x, buf_y);
                if response.drag_started() || response.clicked() {
                    state.move_cursor(byte, false);
                } else {
                    // Extend selection while dragging.
                    state.cursor = byte;
                }
                state.last_interaction_time = now;
            }
        }

        // --- Singleline scroll: keep the cursor inside the visible window ---
        if focused {
            if cursor_x < state.singleline_offset {
                state.singleline_offset = cursor_x;
            } else if cursor_x > state.singleline_offset + visible_width {
                state.singleline_offset = cursor_x - visible_width;
            }
            // Clamp so we never scroll past the end of the text.
            state.singleline_offset = state
                .singleline_offset
                .clamp(0.0, (total_text_width - visible_width).max(0.0));
        }

        if text_changed {
            response.mark_changed();
        }

        // --- IME hint (enables the macOS Character Viewer via the fn key) ---
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

        // --- Render ---
        if ui.is_rect_visible(rect) {
            let visuals = ui.style().interact(&response);

            // Selection highlight — subtle primary tint matching the markdown editor.
            if state.has_selection() {
                let (lo, hi) = state.selection();
                let x0 = (cursor_x_from_buffer(&buffer, lo, ppi) - state.singleline_offset)
                    .clamp(0.0, visible_width);
                let x1 = (cursor_x_from_buffer(&buffer, hi, ppi) - state.singleline_offset)
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

            // Text — the draw rect slides left by singleline_offset so that the
            // visible window scrolls over the full laid-out line. Its width is
            // exactly the measured text width (no inflation), so the GPU scissor
            // matches the clip_rect and nothing bleeds outside `rect`.
            let draw_rect = Rect::from_min_size(
                egui::pos2(rect.min.x - state.singleline_offset, rect.min.y),
                egui::vec2(total_text_width.max(visible_width), line_height),
            );
            let c = visuals.text_color();
            let area = crate::TextBufferArea::new(
                Arc::new(RwLock::new(buffer)),
                draw_rect,
                glyphon::Color::rgba(c.r(), c.g(), c.b(), c.a()),
                ui.ctx(),
                rect, // clip_rect — hard boundary; nothing paints outside this
            );
            ui.painter()
                .add(egui_wgpu_renderer::egui_wgpu::Callback::new_paint_callback(
                    rect, // paint_callback bounds match widget rect exactly
                    crate::GlyphonRendererCallback::new(vec![area]),
                ));

            // Blinking text cursor — resets phase on any interaction.
            if focused {
                let elapsed = now - state.last_interaction_time;
                let blink_on = elapsed < 0.5 || (elapsed * 2.0).fract() < 0.5;
                if blink_on {
                    const CURSOR_W: f32 = 1.5;
                    let cx = (rect.min.x + cursor_x - state.singleline_offset)
                        .clamp(rect.min.x, rect.max.x - CURSOR_W);
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

        // Escape cancels the edit without submitting.
        if focused && ui.input(|i| i.key_pressed(Key::Escape)) {
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

// ---------------------------------------------------------------------------
// Grapheme / hit-test helpers
// ---------------------------------------------------------------------------

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
