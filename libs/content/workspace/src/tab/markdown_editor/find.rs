use egui::{Button, EventFilter, Frame, Id, Key, Margin, TextEdit, Ui, Widget as _};
use lb_rs::text::offset_types::{DocByteOffset, DocCharOffset, RangeExt};

use super::Editor;

pub struct Find {
    pub id: egui::Id,
    pub term: Option<String>,
}

impl Default for Find {
    fn default() -> Self {
        Self { id: Id::new("find"), term: None }
    }
}

#[derive(Default)]
pub struct Response {
    pub term: Option<String>,
    pub backwards: bool,
}

impl Find {
    pub fn show(&mut self, ui: &mut Ui) -> Response {
        let resp = if self.term.is_some() {
            Frame::default()
                .inner_margin(Margin::symmetric(10., 10.))
                .fill(ui.style().visuals.window_fill)
                .stroke(ui.style().visuals.window_stroke)
                .show(ui, |ui| self.show_inner(ui))
                .inner
        } else {
            Response::default()
        };

        if ui.input(|i| i.key_pressed(Key::F) && i.modifiers.command) {
            if self.term.is_none() {
                self.term = Some(String::new());
                ui.memory_mut(|m| m.request_focus(self.id));
            } else {
                if ui.memory(|m| m.has_focus(self.id)) {
                    self.term = None;
                } else {
                    ui.memory_mut(|m| m.request_focus(self.id));
                }
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
        let mut resp = Response::default();
        ui.horizontal(|ui| {
            let resp = if let Some(term) = &mut self.term {
                if Button::new("<").small().ui(ui).clicked()
                    || ui.input(|i| i.key_pressed(Key::Enter) && i.modifiers.shift)
                {
                    resp.term = Some(term.clone());
                    resp.backwards = true;
                }
                ui.add_space(5.);
                if Button::new(">").small().ui(ui).clicked()
                    || ui.input(|i| i.key_pressed(Key::Enter) && !i.modifiers.shift)
                {
                    resp.term = Some(term.clone());
                }
                ui.add_space(5.);

                let resp = TextEdit::singleline(term)
                    .return_key(None)
                    .id(self.id)
                    .desired_width(ui.available_width())
                    .hint_text("Search")
                    .ui(ui);

                resp
            } else {
                unreachable!()
            };
            if ui.input(|i| i.key_pressed(Key::Escape)) && resp.has_focus() {
                self.term = None;
                ui.ctx().request_repaint();
            }
        });
        resp
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
            let slice_result = &buffer.text[byte_start.0..].find(&term)?;
            slice_result + byte_start.0
        } else {
            let mut end = buffer.selection.end();
            if end != 0 {
                end -= 1;
            }
            buffer.text[..buffer.segs.offset_to_byte(end).0].rfind(&term)?
        };
        let result_end = result_start + term.len();
        Some((
            buffer.segs.offset_to_char(DocByteOffset(result_start)),
            buffer.segs.offset_to_char(DocByteOffset(result_end)),
        ))
    }
}
