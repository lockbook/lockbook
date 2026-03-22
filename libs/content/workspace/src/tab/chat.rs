use chrono::{DateTime, Local, Utc};
use egui::{
    Color32, CornerRadius, Frame, Key, Layout, Margin, Rect, ScrollArea, Sense, TextEdit, Ui, Vec2,
    pos2,
};
use lb_rs::model::chat::{Buffer, Message};
use lb_rs::{Uuid, model::file_metadata::DocumentHmac};

use crate::theme::palette_v2::{Palette, ThemeExt};

pub struct Chat {
    pub id: Uuid,
    pub hmac: Option<DocumentHmac>,
    pub messages: Vec<Message>,
    pub input: String,
    pub username: String,
    pub seq: usize,
}

impl Chat {
    pub fn new(bytes: &[u8], id: Uuid, hmac: Option<DocumentHmac>, username: String) -> Self {
        let messages = Buffer::new(bytes).messages;
        Self { id, hmac, messages, input: String::new(), username, seq: 0 }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        Buffer { messages: self.messages.clone() }.serialize()
    }

    pub fn reload(&mut self, bytes: &[u8], hmac: Option<DocumentHmac>) {
        // treat current in-memory state as "local", incoming bytes as "remote",
        // with no base (empty) — equivalent to a union merge
        let merged = Buffer::merge(&[], &self.to_bytes(), bytes);
        self.messages = Buffer::new(&merged).messages;
        self.hmac = hmac;
        self.seq += 1;
    }

    pub fn show(&mut self, ui: &mut Ui) -> bool {
        let mut sent = false;
        let theme = ui.ctx().get_lb_theme();
        let composer_height = 52.0;
        let sep_height = 1.0;

        let full_rect = ui.available_rect_before_wrap();
        // Claim the full rect so centered_and_justified doesn't fight us
        ui.allocate_rect(full_rect, Sense::hover());

        let list_rect = Rect::from_min_max(
            full_rect.min,
            pos2(full_rect.max.x, full_rect.max.y - composer_height - sep_height),
        );
        let composer_rect = Rect::from_min_max(
            pos2(full_rect.min.x, full_rect.max.y - composer_height),
            full_rect.max,
        );

        let available_width = full_rect.width();

        ui.allocate_ui_at_rect(list_rect, |ui| {
            ScrollArea::vertical()
                .id_salt("chat_messages")
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    ui.set_min_width(available_width);
                    ui.add_space(8.0);

                    let mut prev_sender: Option<String> = None;

                    for msg in &self.messages {
                        let is_mine = msg.from == self.username;

                        let bubble_color = if is_mine {
                            theme.fg().get_color(Palette::Blue)
                        } else {
                            theme.neutral_bg_tertiary()
                        };
                        let text_color = if is_mine { Color32::WHITE } else { theme.neutral_fg() };

                        let show_sender = prev_sender.as_deref() != Some(msg.from.as_str());
                        if show_sender {
                            let name_layout = if is_mine {
                                Layout::right_to_left(egui::Align::Min)
                            } else {
                                Layout::left_to_right(egui::Align::Min)
                            };
                            ui.with_layout(name_layout, |ui| {
                                ui.add_space(12.0);
                                ui.label(
                                    egui::RichText::new(&msg.from)
                                        .small()
                                        .color(theme.neutral_fg_secondary()),
                                );
                            });
                        }

                        let bubble_layout = if is_mine {
                            Layout::right_to_left(egui::Align::Min)
                        } else {
                            Layout::left_to_right(egui::Align::Min)
                        };
                        ui.with_layout(bubble_layout, |ui| {
                            ui.add_space(12.0);
                            Frame::new()
                                .fill(bubble_color)
                                .corner_radius(CornerRadius::same(10))
                                .inner_margin(Margin::symmetric(10, 6))
                                .show(ui, |ui| {
                                    ui.set_max_width(available_width * 0.72);
                                    ui.label(egui::RichText::new(&msg.content).color(text_color));
                                });
                        });

                        let ts_layout = if is_mine {
                            Layout::right_to_left(egui::Align::Min)
                        } else {
                            Layout::left_to_right(egui::Align::Min)
                        };
                        ui.with_layout(ts_layout, |ui| {
                            ui.add_space(12.0);
                            ui.label(
                                egui::RichText::new(format_ts(msg.ts))
                                    .small()
                                    .color(theme.neutral_fg_secondary()),
                            );
                        });

                        ui.add_space(4.0);
                        prev_sender = Some(msg.from.clone());
                    }

                    ui.add_space(8.0);
                });
        });

        ui.painter().hline(
            full_rect.x_range(),
            full_rect.max.y - composer_height - sep_height,
            ui.visuals().widgets.noninteractive.bg_stroke,
        );

        ui.allocate_ui_at_rect(composer_rect, |ui| {
            ui.horizontal_centered(|ui| {
                ui.add_space(8.0);
                let te = TextEdit::multiline(&mut self.input)
                    .desired_rows(1)
                    .hint_text("Message...")
                    .frame(false);
                let resp =
                    ui.add_sized(Vec2::new(ui.available_width() - 44.0, composer_height - 8.0), te);

                let enter_sends = resp.has_focus()
                    && ui.input(|i| i.key_pressed(Key::Enter) && !i.modifiers.shift);

                if ui.button("→").clicked() || enter_sends {
                    let trimmed = self.input.trim().to_string();
                    if !trimmed.is_empty() {
                        self.messages.push(Message {
                            from: self.username.clone(),
                            content: trimmed,
                            ts: Utc::now().timestamp(),
                        });
                        self.input.clear();
                        self.seq += 1;
                        resp.request_focus();
                        sent = true;
                    }
                }
            });
        });

        sent
    }
}

fn format_ts(ts: i64) -> String {
    let Some(utc) = DateTime::from_timestamp(ts, 0) else { return String::new() };
    let dt: DateTime<Local> = utc.with_timezone(&Local);
    let now = Local::now();
    let days_ago = (now.date_naive() - dt.date_naive()).num_days();
    if days_ago == 0 {
        dt.format("%H:%M").to_string()
    } else if days_ago == 1 {
        format!("Yesterday {}", dt.format("%H:%M"))
    } else {
        dt.format("%b %-d %H:%M").to_string()
    }
}
