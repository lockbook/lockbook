use egui::{EventFilter, Frame, Id, Key, Label, Margin, Stroke, TextEdit, Ui, Widget as _};
use lb_rs::model::text::buffer::Buffer;
use lb_rs::model::text::offset_types::{DocByteOffset, DocCharOffset, RangeExt as _};

use crate::theme::icons::Icon;
use crate::widgets::IconButton;

use super::super::Editor;

pub struct Find {
    pub id: egui::Id,
    pub term: Option<String>,
    pub match_count: usize,
}

impl Default for Find {
    fn default() -> Self {
        Self { id: Id::new("find"), term: None, match_count: 0 }
    }
}

#[derive(Default)]
pub struct Response {
    pub term: Option<String>,
    pub backwards: bool,
}

impl Find {
    pub fn show(&mut self, buffer: &Buffer, ui: &mut Ui) -> Response {
        let resp = if self.term.is_some() {
            Frame::canvas(ui.style())
                .stroke(Stroke::NONE)
                .inner_margin(Margin::symmetric(10., 10.))
                .show(ui, |ui| self.show_inner(&buffer.current.text, ui))
                .inner
        } else {
            Response::default()
        };

        if ui.input(|i| i.key_pressed(Key::F) && i.modifiers.command && !i.modifiers.shift) {
            if self.term.is_none() {
                self.term = Some(String::from(&buffer[buffer.current.selection]));
                ui.memory_mut(|m| m.request_focus(self.id));
            } else if ui.memory(|m| m.has_focus(self.id)) {
                self.term = None;
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

    pub fn show_inner(&mut self, text: &str, ui: &mut Ui) -> Response {
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
                if term.is_empty() {
                    self.match_count = 0;
                } else {
                    self.match_count = text
                        .to_lowercase()
                        .matches(term.to_lowercase().as_str())
                        .count();
                }
            }
            ui.add_space(5.);

            if IconButton::new(Icon::CHEVRON_LEFT)
                .tooltip("Previous")
                .show(ui)
                .clicked()
                || ui.input(|i| i.key_pressed(Key::Enter) && i.modifiers.shift)
            {
                result.term = Some(term.clone());
                result.backwards = true;
            }
            ui.add_space(5.);
            if IconButton::new(Icon::CHEVRON_RIGHT)
                .tooltip("Next")
                .show(ui)
                .clicked()
                || ui.input(|i| i.key_pressed(Key::Enter) && !i.modifiers.shift)
            {
                result.term = Some(term.clone());
            }
            ui.add_space(5.);

            Label::new(format!("{:?} matches", self.match_count))
                .selectable(false)
                .ui(ui);

            ui.add_space(ui.available_width());

            if ui.input(|i| i.key_pressed(Key::Escape)) && resp.has_focus() {
                self.term = None;
                ui.ctx().request_repaint();
            }

            result
        })
        .inner
    }
}

impl Editor {
    pub fn find(&self, term: String, backwards: bool) -> Option<(DocCharOffset, DocCharOffset)> {
        let buffer = &self.buffer.current;
        let result_start = if !backwards {
            let mut start = buffer.selection.start();
            if start != buffer.segs.last_cursor_position() {
                start += 1;
            }
            let byte_start = buffer.segs.offset_to_byte(start);
            let slice_result = &buffer.text[byte_start.0..]
                .to_lowercase()
                .find(&term.to_lowercase())?;
            slice_result + byte_start.0
        } else {
            let mut end = buffer.selection.end();
            if end != 0 {
                end -= 1;
            }
            buffer.text[..buffer.segs.offset_to_byte(end).0]
                .to_lowercase()
                .rfind(&term.to_lowercase())?
        };
        let result_end = result_start + term.len();
        Some((
            buffer.segs.offset_to_char(DocByteOffset(result_start)),
            buffer.segs.offset_to_char(DocByteOffset(result_end)),
        ))
    }
}
