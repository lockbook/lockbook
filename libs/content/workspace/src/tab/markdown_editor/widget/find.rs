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
//! - **Frame-consistent state**: Cmd+F must be checked before rendering (so the
//!   widget doesn't disappear for a frame on close), but re-focusing must not
//!   skip rendering (so the widget doesn't flicker). The `refocus_term_changed`
//!   flag defers the term_changed signal to the render frame.
//!
//! - **Layout cache invalidation**: The current find match affects node reveal
//!   state and thus cached heights. When the match changes, we invalidate the
//!   layout cache at both the old and new match positions.

use egui::{EventFilter, Frame, Id, Key, Label, Margin, Ui, Widget as _};
use lb_rs::model::text::buffer::Buffer;
use lb_rs::model::text::offset_types::{DocByteOffset, DocCharOffset};

use crate::theme::icons::Icon;
use crate::theme::palette_v2::ThemeExt as _;
use crate::widgets::{GlyphonTextEdit, IconButton};

use super::super::Editor;

pub struct Find {
    pub id: egui::Id,
    replace_id: egui::Id,
    pub term: Option<String>,
    pub replace_term: String,
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub regex: bool,
    select_all_on_focus: bool,
    refocus_term_changed: bool,
    /// All match ranges in the document for the current search term.
    pub matches: Vec<(DocCharOffset, DocCharOffset)>,
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
            refocus_term_changed: false,
            matches: Vec::new(),
            current_match: None,
        }
    }
}

#[derive(Default)]
pub struct Response {
    /// Signals that the user wants to navigate: Some(true) = next, Some(false) = previous.
    pub navigate: Option<bool>,
    /// The search term changed (including on first open).
    pub term_changed: bool,
    /// The find widget was closed this frame.
    pub closed: bool,
    /// Replace the current match with the replacement text.
    pub replace_one: bool,
    /// Replace all matches with the replacement text.
    pub replace_all: bool,
}

impl Find {
    pub fn show(&mut self, buffer: &Buffer, ui: &mut Ui) -> Response {
        if ui.input(|i| i.key_pressed(Key::F) && i.modifiers.command && !i.modifiers.shift) {
            if self.term.is_none() {
                let term = String::from(&buffer[buffer.current.selection]);
                self.term = Some(term);
                self.select_all_on_focus = true;
                ui.memory_mut(|m| m.request_focus(self.id));
                return Response { term_changed: true, ..Default::default() };
            } else {
                let find_focused = ui.memory(|m| m.has_focus(self.id));
                let replace_focused = ui.memory(|m| m.has_focus(self.replace_id));
                if find_focused || replace_focused {
                    self.term = None;
                    self.matches.clear();
                    self.current_match = None;
                    return Response { closed: true, ..Default::default() };
                } else {
                    let selected = String::from(&buffer[buffer.current.selection]);
                    if !selected.is_empty() {
                        *self.term.as_mut().unwrap() = selected;
                        self.refocus_term_changed = true;
                    }
                    self.select_all_on_focus = true;
                    ui.memory_mut(|m| m.request_focus(self.id));
                }
            }
        }

        let resp = if self.term.is_some() {
            Frame::NONE
                .inner_margin(Margin::symmetric(0, 8))
                .show(ui, |ui| self.show_inner(ui))
                .inner
        } else {
            Response::default()
        };
        let focus_filter =
            EventFilter { tab: true, horizontal_arrows: true, vertical_arrows: true, escape: true };
        let find_focused = ui.memory(|m| m.has_focus(self.id));
        let replace_focused = ui.memory(|m| m.has_focus(self.replace_id));
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

        resp
    }

    pub fn show_inner(&mut self, ui: &mut Ui) -> Response {
        ui.vertical(|ui| {
            let mut result = Response::default();
            if std::mem::take(&mut self.refocus_term_changed) {
                result.term_changed = true;
            }
            let Some(term) = &mut self.term else {
                return result;
            };

            let theme = ui.ctx().get_lb_theme();
            let input_bg = theme.neutral_bg_secondary();
            let input_rounding = 4.;
            let input_padding = Margin::symmetric(6, 4);

            // measure input height from a dummy text edit to size buttons consistently
            let input_height = 14.0_f32 * 1.4 + input_padding.sum().y;
            let icon = |i: Icon| {
                IconButton::new(i.size(14.))
                    .subdued(true)
                    .size(input_height)
            };

            // reserve space for buttons on the right; input area fills the rest
            // search row: 3 toggles + 2 nav + close = 6 icon buttons + count ~70px
            let buttons_width = 6. * input_height + 70. + 4. * 8.;
            let input_area_width = (ui.available_width() - buttons_width).max(100.);

            // process keyboard events before layout so Enter is captured
            let before_term = term.clone();
            let find_submitted = GlyphonTextEdit::process_events(ui, self.id, term);
            let find_shift = ui.input(|i| i.modifiers.shift);
            let replace_submitted =
                GlyphonTextEdit::process_events(ui, self.replace_id, &mut self.replace_term);

            if *term != before_term {
                result.term_changed = true;
            }
            if find_submitted {
                result.navigate = Some(!find_shift);
                ui.memory_mut(|m| m.request_focus(self.id));
            }
            if replace_submitted {
                result.replace_one = true;
                ui.memory_mut(|m| m.request_focus(self.replace_id));
            }

            // search row: [  input   count  ] [Aa] [W] [.*] [↑] [↓] [×]
            let find_has_focus = ui
                .horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.;

                    // input area with background containing text edit + match count
                    let input_resp = Frame::NONE
                        .fill(input_bg)
                        .corner_radius(input_rounding)
                        .inner_margin(input_padding)
                        .show(ui, |ui| {
                            ui.set_max_width(input_area_width);
                            ui.horizontal(|ui| {
                                // text edit takes available minus room for count label
                                let count_width = 70.;
                                let edit_width = (ui.available_width() - count_width).max(60.);
                                ui.allocate_ui(
                                    egui::vec2(edit_width, ui.spacing().interact_size.y),
                                    |ui| {
                                        let mut edit = GlyphonTextEdit::new(term)
                                            .id(self.id)
                                            .hint_text("Search");
                                        if self.select_all_on_focus {
                                            edit = edit.select_all();
                                            self.select_all_on_focus = false;
                                        }
                                        edit.show(ui);
                                    },
                                );

                                // right-aligned match count
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        let match_label = if let Some(idx) = self.current_match {
                                            format!("{} / {}", idx + 1, self.matches.len())
                                        } else if self.matches.is_empty() && !term.is_empty() {
                                            "No results".to_string()
                                        } else {
                                            String::new()
                                        };
                                        if !match_label.is_empty() {
                                            let text = egui::RichText::new(match_label).small();
                                            Label::new(text).selectable(false).ui(ui);
                                        }
                                    },
                                );
                            });
                        });
                    let has_focus = ui.memory(|m| m.has_focus(self.id));

                    // clicking the background area focuses the input
                    if input_resp.response.clicked() {
                        ui.memory_mut(|m| m.request_focus(self.id));
                    }

                    if icon(Icon::CASE_SENSITIVE)
                        .tooltip("Match Case")
                        .colored(self.case_sensitive)
                        .show(ui)
                        .clicked()
                    {
                        self.case_sensitive = !self.case_sensitive;
                        result.term_changed = true;
                    }
                    if icon(Icon::WHOLE_WORD)
                        .tooltip("Whole Word")
                        .colored(self.whole_word)
                        .show(ui)
                        .clicked()
                    {
                        self.whole_word = !self.whole_word;
                        result.term_changed = true;
                    }
                    if icon(Icon::REGEX)
                        .tooltip("Regex")
                        .colored(self.regex)
                        .show(ui)
                        .clicked()
                    {
                        self.regex = !self.regex;
                        result.term_changed = true;
                    }

                    if icon(Icon::CHEVRON_UP)
                        .tooltip("Previous")
                        .show(ui)
                        .clicked()
                    {
                        result.navigate = Some(false);
                    }
                    if icon(Icon::CHEVRON_DOWN).tooltip("Next").show(ui).clicked() {
                        result.navigate = Some(true);
                    }

                    if icon(Icon::CLOSE).tooltip("Close").show(ui).clicked() {
                        result.closed = true;
                    }

                    has_focus
                })
                .inner;

            ui.add_space(4.);

            // replace row: [  input  ] [replace] [replace all]
            let replace_has_focus = ui
                .horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.;

                    Frame::NONE
                        .fill(input_bg)
                        .corner_radius(input_rounding)
                        .inner_margin(input_padding)
                        .show(ui, |ui| {
                            ui.set_max_width(input_area_width);
                            GlyphonTextEdit::new(&mut self.replace_term)
                                .id(self.replace_id)
                                .hint_text("Replace")
                                .show(ui);
                        });
                    let has_focus = ui.memory(|m| m.has_focus(self.replace_id));

                    if icon(Icon::REPLACE).tooltip("Replace").show(ui).clicked() {
                        result.replace_one = true;
                    }
                    if icon(Icon::REPLACE_ALL)
                        .tooltip("Replace All")
                        .show(ui)
                        .clicked()
                    {
                        result.replace_all = true;
                    }

                    has_focus
                })
                .inner;

            if ui.input(|i| i.key_pressed(Key::Escape)) && (find_has_focus || replace_has_focus) {
                result.closed = true;
            }

            if result.closed {
                self.term = None;
                self.matches.clear();
                self.current_match = None;
                ui.ctx().request_repaint();
            }

            result
        })
        .inner
    }
}

impl Editor {
    /// Compute all match ranges in the document for the given search term.
    pub fn find_all(&self, term: &str) -> Vec<(DocCharOffset, DocCharOffset)> {
        if term.is_empty() {
            return Vec::new();
        }
        let text = &self.buffer.current.text;
        let segs = &self.buffer.current.segs;

        if self.find.regex {
            return self.find_all_regex(term);
        }

        let (search_text, search_term) = if self.find.case_sensitive {
            (text.to_string(), term.to_string())
        } else {
            (text.to_lowercase(), term.to_lowercase())
        };

        let mut matches = Vec::new();
        let mut byte_start = 0;
        while let Some(pos) = search_text[byte_start..].find(&search_term) {
            let abs_pos = byte_start + pos;
            let abs_end = abs_pos + search_term.len();

            if !self.find.whole_word || self.is_whole_word(text, abs_pos, abs_end) {
                matches.push((
                    segs.offset_to_char(DocByteOffset(abs_pos)),
                    segs.offset_to_char(DocByteOffset(abs_end)),
                ));
            }

            byte_start = abs_pos + 1;
        }
        matches
    }

    fn find_all_regex(&self, term: &str) -> Vec<(DocCharOffset, DocCharOffset)> {
        let text = &self.buffer.current.text;
        let segs = &self.buffer.current.segs;

        let pattern =
            if self.find.whole_word { format!(r"\b(?:{})\b", term) } else { term.to_string() };

        let re = regex::RegexBuilder::new(&pattern)
            .case_insensitive(!self.find.case_sensitive)
            .build();

        let Ok(re) = re else {
            return Vec::new();
        };

        re.find_iter(text)
            .map(|m| {
                (
                    segs.offset_to_char(DocByteOffset(m.start())),
                    segs.offset_to_char(DocByteOffset(m.end())),
                )
            })
            .collect()
    }

    fn is_whole_word(&self, text: &str, byte_start: usize, byte_end: usize) -> bool {
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

    /// Navigate to the next or previous match relative to the current cursor position.
    /// Sets `find.current_match` and returns true if a match was found.
    pub fn find_navigate(&mut self, forward: bool) -> bool {
        if self.find.matches.is_empty() {
            self.find.current_match = None;
            return false;
        }

        let cursor_pos = self.buffer.current.selection.1;
        let new_idx = if forward {
            match self.find.current_match {
                Some(idx) => (idx + 1) % self.find.matches.len(),
                None => {
                    // Find the first match at or after cursor
                    self.find
                        .matches
                        .iter()
                        .position(|m| m.0 >= cursor_pos)
                        .unwrap_or(0)
                }
            }
        } else {
            match self.find.current_match {
                Some(idx) => {
                    if idx == 0 {
                        self.find.matches.len() - 1
                    } else {
                        idx - 1
                    }
                }
                None => {
                    // Find the last match before cursor
                    self.find
                        .matches
                        .iter()
                        .rposition(|m| m.0 < cursor_pos)
                        .unwrap_or(self.find.matches.len() - 1)
                }
            }
        };

        self.find.current_match = Some(new_idx);
        true
    }
}
