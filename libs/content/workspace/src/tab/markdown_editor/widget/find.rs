//! Find & replace widget for the markdown editor.
//!
//! Key challenges that complicate this implementation:
//!
//! - **Decoupled from cursor**: Find highlights and navigates matches without
//!   moving the document selection. This required a parallel reveal system
//!   (`reveal_ranges()` in `inline/mod.rs`) so that the current match triggers
//!   syntax reveal and fold reveal just like the cursor does.
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

use egui::{EventFilter, Frame, Id, Key, Label, Margin, Ui, Widget as _};
use lb_rs::model::text::buffer::Buffer;
use lb_rs::model::text::offset_types::{Byte, Grapheme, RangeExt as _};

use crate::tab::ExtendedOutput as _;
use crate::theme::icons::Icon;
use crate::theme::palette_v2::ThemeExt as _;
use crate::widgets::{GlyphonTextEdit, IconButton};

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
    /// All match ranges in the document for the current search term.
    pub matches: Vec<(Grapheme, Grapheme)>,
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
            matches: Vec::new(),
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
        &mut self, buffer: &Buffer, virtual_keyboard_shown: bool, ui: &mut Ui,
    ) -> FindOutput {
        let mut output = FindOutput::default();

        let open = std::mem::take(&mut self.open_requested)
            || ui.input(|i| i.key_pressed(Key::F) && i.modifiers.command && !i.modifiers.shift);
        if open {
            if self.term.is_none() {
                let term = String::from(&buffer[buffer.current.selection]);
                self.term = Some(term);
                self.select_all_on_focus = true;
                ui.memory_mut(|m| m.request_focus(self.id));
                ui.ctx().set_virtual_keyboard_shown(true);
                self.refresh_matches(buffer, buffer.current.selection.start());
                output.scroll_to_match = !self.matches.is_empty();
                return output;
            }

            let find_focused = ui.memory(|m| m.has_focus(self.id));
            let replace_focused = ui.memory(|m| m.has_focus(self.replace_id));
            if find_focused || replace_focused {
                self.close_state();
                output.closed = true;
                return output;
            }

            let selected = String::from(&buffer[buffer.current.selection]);
            if !selected.is_empty() {
                *self.term.as_mut().unwrap() = selected;
                let anchor = self
                    .current_match_range()
                    .map(|m| m.start())
                    .unwrap_or(buffer.current.selection.start());
                self.refresh_matches(buffer, anchor);
                output.scroll_to_match = !self.matches.is_empty();
            }
            self.select_all_on_focus = true;
            ui.memory_mut(|m| m.request_focus(self.id));
        }

        if self.term.is_some() {
            Frame::NONE
                .inner_margin(Margin::symmetric(10, 10))
                .show(ui, |ui| self.show_inner(buffer, ui, &mut output));
        }

        let focus_filter =
            EventFilter { tab: true, horizontal_arrows: true, vertical_arrows: true, escape: true };
        let find_focused = ui.memory(|m| m.has_focus(self.id));
        let replace_focused = ui.memory(|m| m.has_focus(self.replace_id));
        let focused = find_focused || replace_focused;
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

    fn show_inner(&mut self, buffer: &Buffer, ui: &mut Ui, output: &mut FindOutput) {
        ui.vertical(|ui| {
            let Some(term) = &mut self.term else {
                return;
            };

            // don't render if there's not enough space
            if ui.available_width() < 100. {
                return;
            }

            let input_bg = ui.ctx().get_lb_theme().neutral_bg_secondary();
            let input_padding = Margin::symmetric(6, 4);
            let btn_height = 14.0_f32 * 1.4 + input_padding.sum().y;
            let toggle = |i: Icon| IconButton::new(i.size(14.)).subdued(true).size(btn_height);
            let action = |i: Icon| IconButton::new(i.size(14.)).size(btn_height);
            let input_frame = || {
                Frame::NONE
                    .fill(input_bg)
                    .corner_radius(4.)
                    .inner_margin(input_padding)
            };
            let rtl = egui::Layout::right_to_left(egui::Align::Center);

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

            // search row: RTL — draw buttons first, input fills remainder
            let mut input_width = 0f32;
            let find_has_focus = ui
                .with_layout(rtl, |ui| {
                    ui.spacing_mut().item_spacing.x = 4.;

                    if action(Icon::CLOSE).tooltip("Close").show(ui).clicked() {
                        closed = true;
                    }
                    for (ic, tip, flag) in [
                        (Icon::REGEX, "Regex", &mut self.regex),
                        (Icon::WHOLE_WORD, "Whole Word", &mut self.whole_word),
                        (Icon::CASE_SENSITIVE, "Match Case", &mut self.case_sensitive),
                    ] {
                        if toggle(ic).tooltip(tip).colored(*flag).show(ui).clicked() {
                            *flag = !*flag;
                            term_changed = true;
                        }
                    }

                    input_width = ui.available_width();
                    let input_resp = input_frame().show(ui, |ui| {
                        ui.with_layout(rtl, |ui| {
                            let label = match self.current_match {
                                Some(idx) => format!("{} / {}", idx + 1, self.matches.len()),
                                None if !term.is_empty() => "No results".into(),
                                _ => String::new(),
                            };
                            if !label.is_empty() {
                                Label::new(egui::RichText::new(label).small())
                                    .selectable(false)
                                    .ui(ui);
                            }

                            let mut edit =
                                GlyphonTextEdit::new(term).id(self.id).hint_text("Search");
                            if self.select_all_on_focus {
                                edit = edit.select_all();
                                self.select_all_on_focus = false;
                            }
                            edit.show(ui);
                        });
                    });
                    if input_resp.response.clicked() {
                        ui.memory_mut(|m| m.request_focus(self.id));
                    }

                    ui.memory(|m| m.has_focus(self.id))
                })
                .inner;

            ui.add_space(4.);

            // replace row: LTR — input at same width as search, then buttons
            let replace_has_focus = ui
                .horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.;

                    input_frame().show(ui, |ui| {
                        ui.set_width(input_width - input_padding.sum().x);
                        GlyphonTextEdit::new(&mut self.replace_term)
                            .id(self.replace_id)
                            .hint_text("Replace")
                            .show(ui);
                    });

                    if toggle(Icon::REPLACE).tooltip("Replace").show(ui).clicked() {
                        replace_one = true;
                    }
                    if toggle(Icon::REPLACE_ALL)
                        .tooltip("Replace All")
                        .show(ui)
                        .clicked()
                    {
                        replace_all = true;
                    }
                    if action(Icon::CHEVRON_UP)
                        .tooltip("Previous")
                        .show(ui)
                        .clicked()
                    {
                        navigate = Some(false);
                    }
                    if action(Icon::CHEVRON_DOWN)
                        .tooltip("Next")
                        .show(ui)
                        .clicked()
                    {
                        navigate = Some(true);
                    }

                    ui.memory(|m| m.has_focus(self.replace_id))
                })
                .inner;

            if ui.input(|i| i.key_pressed(Key::Escape)) && (find_has_focus || replace_has_focus) {
                closed = true;
            }

            // apply state transitions
            if term_changed {
                let anchor = self
                    .current_match_range()
                    .map(|m| m.start())
                    .unwrap_or(buffer.current.selection.start());
                self.refresh_matches(buffer, anchor);
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
                self.close_state();
                output.closed = true;
                ui.ctx().request_repaint();
            }
        });
    }

    /// Recompute `matches` for the current term, positioning `current_match`
    /// at the first match at or after `anchor`.
    fn refresh_matches(&mut self, buffer: &Buffer, anchor: Grapheme) {
        let term = self.term.clone().unwrap_or_default();
        self.matches = self.find_all(buffer, &term);
        if self.matches.is_empty() {
            self.current_match = None;
        } else {
            let idx = self.matches.iter().position(|m| m.0 >= anchor).unwrap_or(0);
            self.current_match = Some(idx);
        }
    }

    fn close_state(&mut self) {
        self.term = None;
        self.matches.clear();
        self.current_match = None;
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
