use egui::{EventFilter, Frame, Id, Key, Label, Margin, Stroke, TextEdit, Ui, Widget as _};
use lb_rs::model::text::buffer::Buffer;
use lb_rs::model::text::offset_types::{DocByteOffset, DocCharOffset};

use crate::theme::icons::Icon;
use crate::widgets::IconButton;

use super::super::Editor;

pub struct Find {
    pub id: egui::Id,
    pub term: Option<String>,
    /// All match ranges in the document for the current search term.
    pub matches: Vec<(DocCharOffset, DocCharOffset)>,
    /// Index into `matches` for the currently focused match, if any.
    pub current_match: Option<usize>,
}

impl Default for Find {
    fn default() -> Self {
        Self { id: Id::new("find"), term: None, matches: Vec::new(), current_match: None }
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
        if ui.memory(|m| m.has_focus(self.id)) {
            ui.memory_mut(|m| {
                m.set_focus_lock_filter(
                    self.id,
                    EventFilter {
                        tab: true,
                        horizontal_arrows: true,
                        vertical_arrows: true,
                        escape: true,
                    },
                )
            })
        }

        resp
    }

    pub fn show_inner(&mut self, ui: &mut Ui) -> Response {
        ui.horizontal(|ui| {
            let mut result = Response::default();
            let Some(term) = &mut self.term else {
                return result;
            };

            ui.spacing_mut().button_padding = egui::vec2(5., 5.);
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
                result.navigate = Some(false); // previous
            }
            ui.add_space(5.);
            if IconButton::new(Icon::CHEVRON_RIGHT)
                .tooltip("Next")
                .show(ui)
                .clicked()
                || ui.input(|i| i.key_pressed(Key::Enter) && !i.modifiers.shift)
            {
                result.navigate = Some(true); // next
            }
            ui.add_space(5.);

            let match_label = if let Some(idx) = self.current_match {
                format!("{} of {}", idx + 1, self.matches.len())
            } else if self.matches.is_empty() {
                "No results".to_string()
            } else {
                format!("{} matches", self.matches.len())
            };
            Label::new(match_label)
                .selectable(false)
                .ui(ui);

            ui.add_space(ui.available_width());

            if ui.input(|i| i.key_pressed(Key::Escape)) && resp.has_focus() {
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
        let lower_term = term.to_lowercase();
        let lower_text = text.to_lowercase();
        let mut matches = Vec::new();
        let mut byte_start = 0;
        while let Some(pos) = lower_text[byte_start..].find(&lower_term) {
            let abs_pos = byte_start + pos;
            let abs_end = abs_pos + lower_term.len();
            matches.push((
                segs.offset_to_char(DocByteOffset(abs_pos)),
                segs.offset_to_char(DocByteOffset(abs_end)),
            ));
            byte_start = abs_pos + 1; // advance by 1 byte to find overlapping matches
        }
        matches
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
