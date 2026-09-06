//! Find & replace widget for the markdown editor.
//!
//! Key challenges that complicate this implementation:
//!
//! - **Decoupled from cursor**: Find highlights and navigates matches without
//!   moving the document selection. This required a parallel reveal system
//!   (`reveal_ranges()` in `inline/mod.rs`) so that the current match triggers
//!   syntax reveal just like the cursor does, plus fold reveal (a cursor in
//!   folded contents instead unfolds for real).
//!
//! - **Galley culling**: The editor only renders blocks that are visible or
//!   overlap the selection. To scroll to an off-screen match, we extended
//!   `galley_required_ranges()` to also include the current match, ensuring its
//!   galley exists before `scroll_to_rect` runs.
//!
//! - **Event ownership**: `GlyphonTextEdit` consumes keyboard events (including
//!   Enter) from the egui input queue. We call `process_events()` before
//!   rendering to capture Enter for navigation, then re-request focus so the
//!   input stays active.
//!
//! - **Layout cache invalidation**: The current find match affects node reveal
//!   state and thus cached heights. The caller snapshots
//!   [`Find::current_match_range`] before and after `show` and invalidates the
//!   layout cache when it changes — mirroring how the cursor selection is
//!   handled.

use egui::{
    Align, Color32, EventFilter, Frame, Id, Key, Layout, Margin, Sense, Stroke, StrokeKind, Ui,
    pos2, vec2,
};
use lb_rs::model::text::buffer::Buffer;
use lb_rs::model::text::offset_types::{Byte, Grapheme, RangeExt as _};

use crate::style::chrome::control_line_height;
use crate::style::layout::{inset, paint_control_pads};
use crate::style::space::control as control_space;
use crate::style::{
    CHROME_BAND_GLYPH, Radius, STROKE_HAIRLINE, Space, ThemeExt, TypeRole, claim, control_height,
    icon_button_glyph, origin, phosphor, place_at, sense_click, tip_text,
};
use crate::tab::ExtendedOutput as _;
use crate::widgets::GlyphonTextEdit;

use super::super::input::{Event, Region};

pub struct Find {
    pub id: egui::Id,
    replace_id: egui::Id,
    pub term: Option<String>,
    pub replace_term: String,
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub regex: bool,
    select_all_on_focus: bool,
    pub open_requested: bool,
    was_focused: bool,
    /// Whether a field held focus last frame. `begin_pass` clears live focus on
    /// Esc before `show` runs, so the Esc-to-close check reads this (#4646).
    prev_focused: bool,
    /// All match ranges in the document for the current search term.
    pub matches: Vec<(Grapheme, Grapheme)>,
    /// deps: `(text_seq, term, case_sensitive, whole_word, regex)`
    matches_deps: (u64, Option<String>, bool, bool, bool),
    /// Index into `matches` for the currently focused match, if any.
    pub current_match: Option<usize>,
}

impl Default for Find {
    fn default() -> Self {
        Self {
            id: Id::new("find"),
            replace_id: Id::new("find_replace"),
            term: None,
            replace_term: String::new(),
            case_sensitive: false,
            whole_word: false,
            regex: false,
            select_all_on_focus: false,
            open_requested: false,
            was_focused: false,
            prev_focused: false,
            matches: Vec::new(),
            matches_deps: (0, None, false, false, false),
            current_match: None,
        }
    }
}

/// Effects the caller must apply after a [`Find::show`] pass.
///
/// Match-driven layout-cache invalidation is *not* represented here — the
/// caller snapshots [`Find::current_match_range`] before and after `show` and
/// diffs it itself, the same way cursor-selection reveal invalidation is
/// handled elsewhere.
#[derive(Default)]
pub struct FindOutput {
    /// Buffer events the caller should push onto the editor's event queue
    /// (always `Event::Replace` for find-driven replacement).
    pub events: Vec<Event>,
    /// Caller should scroll to the current match on the next frame. Set when
    /// the user initiated navigation (term entry, Enter, chevrons).
    pub scroll_to_match: bool,
    /// Find was closed this frame.
    pub closed: bool,
}

impl Find {
    /// Range of the currently focused match, if any. Caller snapshots this
    /// before and after [`Find::show`] to detect reveal-state changes.
    pub fn current_match_range(&self) -> Option<(Grapheme, Grapheme)> {
        self.current_match
            .and_then(|idx| self.matches.get(idx).copied())
    }

    /// Render the find widget and advance its state. All term/match/navigation
    /// transitions happen inside this call; [`FindOutput`] carries only the
    /// effects the caller must apply (buffer events, scroll hint, close).
    pub fn show(
        &mut self, buffer: &Buffer, text_seq: u64, virtual_keyboard_shown: bool, ui: &mut Ui,
    ) -> FindOutput {
        let mut output = FindOutput::default();

        // Consume Esc before the text fields do (they only surrender their own
        // focus), or the widget never closes (#4646).
        let field_focused = self.prev_focused
            || ui.memory(|m| m.has_focus(self.id) || m.has_focus(self.replace_id));
        if self.term.is_some()
            && field_focused
            && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Escape))
        {
            self.close_to_match(&mut output);
            ui.ctx().request_repaint();
            return output;
        }

        let open = std::mem::take(&mut self.open_requested)
            || ui.input(|i| i.key_pressed(Key::F) && i.modifiers.command && !i.modifiers.shift);
        if open {
            if self.term.is_none() {
                let term = String::from(&buffer[buffer.current.selection]);
                self.term = Some(term);
                self.select_all_on_focus = true;
                ui.memory_mut(|m| m.request_focus(self.id));
                ui.ctx().set_virtual_keyboard_shown(true);
                self.refresh_matches(buffer, text_seq, buffer.current.selection.start());
                output.scroll_to_match = !self.matches.is_empty();
                return output;
            }

            // Cmd+F on an open widget re-focuses and reselects the search
            // field, adopting any fresh editor selection as the term.
            let selected = String::from(&buffer[buffer.current.selection]);
            if !selected.is_empty() {
                *self.term.as_mut().unwrap() = selected;
                let anchor = self
                    .current_match_range()
                    .map(|m| m.start())
                    .unwrap_or(buffer.current.selection.start());
                self.refresh_matches(buffer, text_seq, anchor);
                output.scroll_to_match = !self.matches.is_empty();
            }
            self.select_all_on_focus = true;
            ui.memory_mut(|m| m.request_focus(self.id));
        }

        if self.term.is_some() {
            Frame::NONE
                .inner_margin(Margin::symmetric(Space::Sm.pts() as i8, Space::Xs.pts() as i8))
                .show(ui, |ui| self.show_inner(buffer, text_seq, ui, &mut output));
        }

        let focus_filter =
            EventFilter { tab: true, horizontal_arrows: true, vertical_arrows: true, escape: true };
        let find_focused = ui.memory(|m| m.has_focus(self.id));
        let replace_focused = ui.memory(|m| m.has_focus(self.replace_id));
        let focused = find_focused || replace_focused;
        self.prev_focused = focused;
        if focused && !self.was_focused {
            ui.ctx().set_virtual_keyboard_shown(true);
        }
        // reset when keyboard is dismissed so re-tapping requests it again
        self.was_focused = focused && virtual_keyboard_shown;
        if find_focused {
            ui.memory_mut(|m| m.set_focus_lock_filter(self.id, focus_filter));
            if ui.input(|i| i.key_pressed(Key::Tab) && !i.modifiers.shift) {
                ui.memory_mut(|m| m.request_focus(self.replace_id));
            }
        }
        if replace_focused {
            ui.memory_mut(|m| m.set_focus_lock_filter(self.replace_id, focus_filter));
            if ui.input(|i| i.key_pressed(Key::Tab) && i.modifiers.shift) {
                ui.memory_mut(|m| m.request_focus(self.id));
            }
        }

        output
    }

    fn show_inner(&mut self, buffer: &Buffer, text_seq: u64, ui: &mut Ui, output: &mut FindOutput) {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
            let Some(term) = &mut self.term else {
                return;
            };

            // don't render if there's not enough space
            if ui.available_width() < 100. {
                return;
            }

            let t = ui.ctx().get_lb_theme();
            let gap = Space::Xs.pts();
            let rtl = Layout::right_to_left(Align::Center);
            let icon_spacing = egui::vec2(gap, 0.0);

            // process keyboard events before layout so Enter is captured
            let before_term = term.clone();
            let find_submitted = GlyphonTextEdit::process_events(ui, self.id, term);
            let find_shift = ui.input(|i| i.modifiers.shift);
            let replace_submitted =
                GlyphonTextEdit::process_events(ui, self.replace_id, &mut self.replace_term);

            let mut term_changed = *term != before_term;
            let mut navigate: Option<bool> = None;
            let mut replace_one = false;
            let mut replace_all = false;
            let mut closed = false;

            if find_submitted {
                navigate = Some(!find_shift);
                ui.memory_mut(|m| m.request_focus(self.id));
            }
            if replace_submitted {
                replace_one = true;
                ui.memory_mut(|m| m.request_focus(self.replace_id));
            }

            // Measured rows the same height as the field so icons share its
            // vertical center (don't let leftover editor height steal Align::Center).
            let row_w = ui.available_width();
            let row_h = control_height();

            // search row: RTL — draw buttons first, input fills remainder
            let mut input_width = 0f32;
            let search_row = egui::Rect::from_min_size(origin(ui), vec2(row_w, row_h));
            place_at(ui, search_row, rtl, |ui| {
                ui.spacing_mut().item_spacing = icon_spacing;

                if find_icon(ui, &t, phosphor::X, true, "Close") {
                    closed = true;
                }
                for (ic, tip, flag) in [
                    (phosphor::FUNCTION, "Regex", &mut self.regex),
                    (phosphor::TEXT_T, "Whole Word", &mut self.whole_word),
                    (phosphor::TEXT_AA, "Match Case", &mut self.case_sensitive),
                ] {
                    if find_icon(ui, &t, ic, *flag, tip) {
                        *flag = !*flag;
                        term_changed = true;
                    }
                }

                input_width = ui.available_width();
                let find_focus = ui.memory(|m| m.has_focus(self.id));
                let input_resp = find_field(ui, &t, input_width, find_focus, |ui| {
                    ui.with_layout(rtl, |ui| {
                        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                        let label = match self.current_match {
                            Some(idx) => format!("{} / {}", idx + 1, self.matches.len()),
                            None if !term.is_empty() => "No results".into(),
                            _ => String::new(),
                        };
                        if !label.is_empty() {
                            find_count(ui, &t, &label);
                            ui.add_space(gap);
                        }

                        let mut edit = GlyphonTextEdit::new(term)
                            .id(self.id)
                            .font_size(TypeRole::Body.size())
                            .line_height(control_line_height())
                            .hint_text("Search");
                        if self.select_all_on_focus {
                            edit = edit.select_all();
                            self.select_all_on_focus = false;
                        }
                        edit.show(ui);
                    });
                });
                if input_resp.clicked() {
                    ui.memory_mut(|m| m.request_focus(self.id));
                }
            });
            claim(ui, search_row);

            ui.add_space(gap);

            // replace row: LTR — input at same width as search, then buttons
            let replace_row = egui::Rect::from_min_size(origin(ui), vec2(row_w, row_h));
            place_at(ui, replace_row, Layout::left_to_right(Align::Center), |ui| {
                ui.spacing_mut().item_spacing = icon_spacing;

                let replace_focus = ui.memory(|m| m.has_focus(self.replace_id));
                find_field(ui, &t, input_width, replace_focus, |ui| {
                    GlyphonTextEdit::new(&mut self.replace_term)
                        .id(self.replace_id)
                        .font_size(TypeRole::Body.size())
                        .line_height(control_line_height())
                        .hint_text("Replace")
                        .show(ui);
                });
                if find_icon(ui, &t, phosphor::REPEAT, true, "Replace") {
                    replace_one = true;
                }
                if find_icon(ui, &t, phosphor::SWAP, true, "Replace All") {
                    replace_all = true;
                }
                if find_icon(ui, &t, phosphor::CARET_UP, true, "Previous") {
                    navigate = Some(false);
                }
                if find_icon(ui, &t, phosphor::CARET_DOWN, true, "Next") {
                    navigate = Some(true);
                }
            });
            claim(ui, replace_row);

            // apply state transitions
            if term_changed {
                let anchor = self
                    .current_match_range()
                    .map(|m| m.start())
                    .unwrap_or(buffer.current.selection.start());
                self.refresh_matches(buffer, text_seq, anchor);
                if !self.matches.is_empty() {
                    output.scroll_to_match = true;
                }
            }
            if let Some(forward) = navigate {
                if self.navigate(forward, buffer.current.selection.1) {
                    output.scroll_to_match = true;
                }
            }
            if replace_one {
                if let Some(range) = self.current_match_range() {
                    output.events.push(Event::Replace {
                        region: Region::from(range),
                        text: self.replace_term.clone(),
                        advance_cursor: false,
                    });
                }
            }
            if replace_all {
                for &range in self.matches.iter().rev() {
                    output.events.push(Event::Replace {
                        region: Region::from(range),
                        text: self.replace_term.clone(),
                        advance_cursor: false,
                    });
                }
            }
            if closed {
                self.close_to_match(output);
                ui.ctx().request_repaint();
            }
        });
    }

    /// Close the widget, seeding the editor selection from the current match so
    /// the user resumes editing where they searched (#4646).
    fn close_to_match(&mut self, output: &mut FindOutput) {
        if let Some(range) = self.current_match_range() {
            output
                .events
                .push(Event::Select { region: Region::from(range) });
        }
        self.close_state();
        output.closed = true;
    }

    /// Recompute `matches` for the current term, positioning `current_match`
    /// at the first match at or after `anchor`.
    fn refresh_matches(&mut self, buffer: &Buffer, text_seq: u64, anchor: Grapheme) {
        self.ensure_matches(buffer, text_seq);
        if !self.matches.is_empty() {
            let idx = self.matches.iter().position(|m| m.0 >= anchor).unwrap_or(0);
            self.current_match = Some(idx);
        }
    }

    fn close_state(&mut self) {
        self.term = None;
        self.matches.clear();
        self.current_match = None;
    }

    /// Recompute `matches` if any input (buffer text, term, search flags)
    /// has advanced since last compute.
    pub fn ensure_matches(&mut self, buffer: &Buffer, text_seq: u64) {
        let deps = (text_seq, self.term.clone(), self.case_sensitive, self.whole_word, self.regex);
        if deps == self.matches_deps {
            return;
        }
        self.matches = match &self.term {
            Some(term) => self.find_all(buffer, term),
            None => Vec::new(),
        };
        if self.matches.is_empty() {
            self.current_match = None;
        } else if let Some(idx) = self.current_match {
            if idx >= self.matches.len() {
                self.current_match = Some(self.matches.len() - 1);
            }
        }
        self.matches_deps = deps;
    }

    /// Compute all match ranges in the document for the given search term.
    pub fn find_all(&self, buffer: &Buffer, term: &str) -> Vec<(Grapheme, Grapheme)> {
        if term.is_empty() {
            return Vec::new();
        }
        let text = &buffer.current.text;
        let segs = &buffer.current.segs;

        if self.regex {
            return self.find_all_regex(buffer, term);
        }

        let (search_text, search_term) = if self.case_sensitive {
            (text.to_string(), term.to_string())
        } else {
            (text.to_lowercase(), term.to_lowercase())
        };

        let mut matches = Vec::new();
        let mut byte_start = 0;
        while let Some(pos) = search_text[byte_start..].find(&search_term) {
            let abs_pos = byte_start + pos;
            let abs_end = abs_pos + search_term.len();

            if !self.whole_word || is_whole_word(text, abs_pos, abs_end) {
                matches
                    .push((segs.offset_to_char(Byte(abs_pos)), segs.offset_to_char(Byte(abs_end))));
            }

            byte_start = abs_end;
        }
        matches
    }

    fn find_all_regex(&self, buffer: &Buffer, term: &str) -> Vec<(Grapheme, Grapheme)> {
        let text = &buffer.current.text;
        let segs = &buffer.current.segs;

        let pattern = if self.whole_word { format!(r"\b(?:{})\b", term) } else { term.to_string() };

        let re = regex::RegexBuilder::new(&pattern)
            .case_insensitive(!self.case_sensitive)
            .build();

        let Ok(re) = re else {
            return Vec::new();
        };

        re.find_iter(text)
            .map(|m| (segs.offset_to_char(Byte(m.start())), segs.offset_to_char(Byte(m.end()))))
            .collect()
    }

    /// Navigate to the next or previous match relative to the cursor. Sets
    /// `current_match` and returns true if a match is present.
    fn navigate(&mut self, forward: bool, cursor: Grapheme) -> bool {
        if self.matches.is_empty() {
            self.current_match = None;
            return false;
        }

        let new_idx = if forward {
            match self.current_match {
                Some(idx) => (idx + 1) % self.matches.len(),
                None => self.matches.iter().position(|m| m.0 >= cursor).unwrap_or(0),
            }
        } else {
            match self.current_match {
                Some(idx) => {
                    if idx == 0 {
                        self.matches.len() - 1
                    } else {
                        idx - 1
                    }
                }
                None => self
                    .matches
                    .iter()
                    .rposition(|m| m.0 < cursor)
                    .unwrap_or(self.matches.len() - 1),
            }
        };

        self.current_match = Some(new_idx);
        true
    }
}

fn find_icon(
    ui: &mut Ui, t: &crate::style::Theme, icon: &'static str, on: bool, tip: &str,
) -> bool {
    let r = icon_button_glyph(ui, t, icon, on, t.neutral_bg(), control_height(), CHROME_BAND_GLYPH);
    tip_text(ui.ctx(), &r, tip);
    r.clicked()
}

/// Match count in the search field: same line box as the glyphon edit so it
/// shares the field's vertical center (stock `ui.label` does not).
fn find_count(ui: &mut Ui, t: &crate::style::Theme, label: &str) {
    let g = ui.painter().layout_no_wrap(
        label.to_owned(),
        TypeRole::Mono.font_id(),
        Color32::PLACEHOLDER,
    );
    let (rect, _) = ui.allocate_exact_size(vec2(g.size().x, control_line_height()), Sense::hover());
    ui.painter().galley(
        pos2(rect.left(), rect.center().y - g.size().y / 2.0),
        g,
        t.neutral_fg_secondary(),
    );
}

fn find_field(
    ui: &mut Ui, t: &crate::style::Theme, width: f32, focused: bool, add: impl FnOnce(&mut Ui),
) -> egui::Response {
    let h = control_height();
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(width.max(1.0), h), sense_click());
    let (fill, stroke_c) = if focused {
        (t.neutral_bg(), t.neutral_fg())
    } else {
        (t.neutral_bg_secondary(), t.neutral())
    };
    ui.painter()
        .rect_filled(rect, Radius::Control.corner(), fill);
    ui.painter().rect_stroke(
        rect,
        Radius::Control.corner(),
        Stroke::new(STROKE_HAIRLINE, stroke_c),
        StrokeKind::Inside,
    );
    let pad_x = control_space::PAD_X;
    let pad_y = control_space::PAD_Y;
    paint_control_pads(ui, rect, pad_x, pad_y);
    let inner = inset(rect, pad_x.pts(), pad_y.pts());
    place_at(ui, inner, Layout::left_to_right(Align::Center), add);
    resp
}

fn is_whole_word(text: &str, byte_start: usize, byte_end: usize) -> bool {
    let is_word_char = |c: char| c.is_alphanumeric() || c == '_';
    let before_ok = byte_start == 0
        || !text[..byte_start]
            .chars()
            .next_back()
            .is_some_and(is_word_char);
    let after_ok =
        byte_end >= text.len() || !text[byte_end..].chars().next().is_some_and(is_word_char);
    before_ok && after_ok
}
