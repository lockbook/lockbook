use egui::{EventFilter, Frame, Id, Key, Margin, TextEdit, Ui};

#[derive(Default)]
pub struct Find {
    pub term: Option<String>,
}

pub struct Response {}

impl Find {
    pub fn show(&mut self, ui: &mut Ui) -> Response {
        let id = Id::new("find-term");

        if ui.input(|i| i.key_pressed(Key::F) && i.modifiers.command) {
            if self.term.is_none() {
                self.term = Some(String::new());
                ui.memory_mut(|m| m.request_focus(id));
            } else {
                if ui.memory(|m| m.has_focus(id)) {
                    self.term = None;
                } else {
                    ui.memory_mut(|m| m.request_focus(id));
                }
            }
        }
        if ui.memory(|m| m.has_focus(id)) {
            ui.memory_mut(|m| {
                m.set_focus_lock_filter(
                    id,
                    EventFilter {
                        tab: true,
                        horizontal_arrows: true,
                        vertical_arrows: true,
                        escape: true,
                    },
                )
            })
        }

        if self.term.is_some() {
            Frame::default()
                .inner_margin(Margin::symmetric(10., 10.))
                .fill(ui.style().visuals.window_fill)
                .stroke(ui.style().visuals.window_stroke)
                .show(ui, |ui| self.show_inner(ui))
                .inner
        } else {
            Response {}
        }
    }

    pub fn show_inner(&mut self, ui: &mut Ui) -> Response {
        let id = Id::new("find-term");

        let mut search_term_response = None;
        ui.horizontal(|ui| {
            let resp = if let Some(term) = &mut self.term {
                ui.add(
                    TextEdit::singleline(term)
                        .id(id)
                        .desired_width(ui.available_width())
                        .hint_text("Search"),
                )
            } else {
                unreachable!()
            };
            if ui.input(|i| i.key_pressed(Key::Enter) && !i.modifiers.shift) {
                println!("find next");
            }
            if ui.input(|i| i.key_pressed(Key::Enter) && i.modifiers.shift) {
                println!("find previous");
            }
            if ui.input(|i| i.key_pressed(Key::Escape)) && resp.has_focus() {
                self.term = None;
                ui.ctx().request_repaint();
            }

            search_term_response = Some(resp);
        });

        Response {}
    }
}
