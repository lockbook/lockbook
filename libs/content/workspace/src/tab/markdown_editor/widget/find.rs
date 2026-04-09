use egui::{
    Button, EventFilter, Frame, Id, Key, Label, Margin, Stroke, TextEdit, Ui, Widget as _,
};
use lb_rs::model::text::buffer::Buffer;
use lb_rs::model::text::offset_types::{DocByteOffset, DocCharOffset};

use crate::theme::icons::Icon;
use crate::widgets::IconButton;

use super::super::Editor;

pub struct Find {
    pub id: egui::Id,
    replace_id: egui::Id,
    pub term: Option<String>,
    pub replace_term: String,
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub regex: bool,
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
        let resp = if self.term.is_some() {
            Frame::canvas(ui.style())
                .stroke(Stroke::NONE)
                .inner_margin(Margin::symmetric(10, 10))
                .show(ui, |ui| self.show_inner(ui))
                .inner
        } else {
            Response::default()
        };

        if ui.input(|i| i.key_pressed(Key::F) && i.modifiers.command && !i.modifiers.shift) {
            if self.term.is_none() {
                let term = String::from(&buffer[buffer.current.selection]);
                self.term = Some(term);
                ui.memory_mut(|m| m.request_focus(self.id));
                return Response { term_changed: true, ..Default::default() };
            } else if ui.memory(|m| m.has_focus(self.id)) {
                self.term = None;
                self.matches.clear();
                self.current_match = None;
                return Response { closed: true, ..Default::default() };
            } else {
                ui.memory_mut(|m| m.request_focus(self.id));
            }
        }
        let focus_filter = EventFilter {
            tab: true,
            horizontal_arrows: true,
            vertical_arrows: true,
            escape: true,
        };
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
            let Some(term) = &mut self.term else {
                return result;
            };

            ui.spacing_mut().button_padding = egui::vec2(5., 5.);

            // search row
            let find_has_focus = ui
                .horizontal(|ui| {
                    ui.set_min_height(30.);

                    let before_term = term.clone();
                    let resp = TextEdit::singleline(term)
                        .return_key(None)
                        .id(self.id)
                        .desired_width(300.)
                        .hint_text("Search")
                        .ui(ui);
                    if term != &before_term {
                        result.term_changed = true;
                    }
                    ui.add_space(5.);

                    if IconButton::new(Icon::CHEVRON_LEFT)
                        .tooltip("Previous")
                        .show(ui)
                        .clicked()
                        || ui.input(|i| i.key_pressed(Key::Enter) && i.modifiers.shift)
                    {
                        result.navigate = Some(false);
                    }
                    ui.add_space(5.);
                    if IconButton::new(Icon::CHEVRON_RIGHT)
                        .tooltip("Next")
                        .show(ui)
                        .clicked()
                        || ui.input(|i| i.key_pressed(Key::Enter) && !i.modifiers.shift)
                    {
                        result.navigate = Some(true);
                    }
                    ui.add_space(5.);

                    let match_label = if let Some(idx) = self.current_match {
                        format!("{} of {}", idx + 1, self.matches.len())
                    } else if self.matches.is_empty() {
                        "No results".to_string()
                    } else {
                        format!("{} matches", self.matches.len())
                    };
                    Label::new(match_label).selectable(false).ui(ui);

                    ui.add_space(10.);

                    if Button::new("Aa").selected(self.case_sensitive).ui(ui)
                        .on_hover_text("Match Case")
                        .clicked()
                    {
                        self.case_sensitive = !self.case_sensitive;
                        result.term_changed = true;
                    }
                    if Button::new("W").selected(self.whole_word).ui(ui)
                        .on_hover_text("Whole Word")
                        .clicked()
                    {
                        self.whole_word = !self.whole_word;
                        result.term_changed = true;
                    }
                    if Button::new(".*").selected(self.regex).ui(ui)
                        .on_hover_text("Regex")
                        .clicked()
                    {
                        self.regex = !self.regex;
                        result.term_changed = true;
                    }

                    resp.has_focus()
                })
                .inner;

            // replace row
            let replace_has_focus = ui
                .horizontal(|ui| {
                    ui.set_min_height(30.);

                    let resp = TextEdit::singleline(&mut self.replace_term)
                        .return_key(None)
                        .id(self.replace_id)
                        .desired_width(300.)
                        .hint_text("Replace")
                        .ui(ui);
                    ui.add_space(5.);

                    if Button::new("Replace").ui(ui).clicked() {
                        result.replace_one = true;
                    }
                    ui.add_space(5.);
                    if Button::new("Replace All").ui(ui).clicked() {
                        result.replace_all = true;
                    }

                    resp.has_focus()
                })
                .inner;

            if ui.input(|i| i.key_pressed(Key::Escape))
                && (find_has_focus || replace_has_focus)
            {
                self.term = None;
                self.matches.clear();
                self.current_match = None;
                result.closed = true;
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

        let pattern = if self.find.whole_word {
            format!(r"\b(?:{})\b", term)
        } else {
            term.to_string()
        };

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
        let before_ok = byte_start == 0
            || !text[..byte_start]
                .chars()
                .next_back()
                .map_or(false, |c| c.is_alphanumeric() || c == '_');
        let after_ok = byte_end >= text.len()
            || !text[byte_end..]
                .chars()
                .next()
                .map_or(false, |c| c.is_alphanumeric() || c == '_');
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
                    self.find.matches.iter().position(|m| m.0 >= cursor_pos)
                        .unwrap_or(0)
                }
            }
        } else {
            match self.find.current_match {
                Some(idx) => {
                    if idx == 0 { self.find.matches.len() - 1 } else { idx - 1 }
                }
                None => {
                    // Find the last match before cursor
                    self.find.matches.iter().rposition(|m| m.0 < cursor_pos)
                        .unwrap_or(self.find.matches.len() - 1)
                }
            }
        };

        self.find.current_match = Some(new_idx);
        true
    }
}
