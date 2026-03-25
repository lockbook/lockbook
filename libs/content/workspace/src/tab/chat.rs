use std::sync::mpsc;

use chrono::{DateTime, Local, Utc};
use egui::{
    CornerRadius, Key, Rect, ScrollArea, TextEdit, Ui, pos2,
    text::{LayoutJob, TextFormat},
    vec2,
};
use lb_rs::model::chat::{Buffer, Message};
use lb_rs::service::ai::{ApiMessage, ToolRequest};
use lb_rs::{Uuid, model::file_metadata::DocumentHmac};

use crate::theme::palette_v2::{Palette, ThemeExt, username_color};

pub enum AgentStatus {
    Idle,
    Thinking,
    ToolsPending { requests: Vec<ToolRequest>, approve_tx: mpsc::Sender<bool> },
    ToolRunning { name: String },
}

pub struct Chat {
    pub id: Uuid,
    pub hmac: Option<DocumentHmac>,
    pub messages: Vec<Message>,
    pub input: String,
    pub username: String,
    pub seq: usize,
    pub initialized: bool,
    pub is_agent: bool,
    pub agent_pending: bool,
    pub agent_status: AgentStatus,
    /// Full API message history including tool_use/tool_result blocks.
    /// Kept in memory for the session so the agent retains tool context across turns.
    pub api_messages: Vec<ApiMessage>,
}

impl Chat {
    pub fn new(
        bytes: &[u8], id: Uuid, hmac: Option<DocumentHmac>, username: String, is_agent: bool,
    ) -> Self {
        let messages = Buffer::new(bytes).messages;
        Self {
            id,
            hmac,
            messages,
            input: String::new(),
            username,
            seq: 0,
            initialized: false,
            is_agent,
            agent_pending: false,
            agent_status: AgentStatus::Idle,
            api_messages: Vec::new(),
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        Buffer { messages: self.messages.clone() }.serialize()
    }

    pub fn reload(&mut self, bytes: &[u8], hmac: Option<DocumentHmac>) {
        let merged = Buffer::merge(&[], &self.to_bytes(), bytes);
        self.messages = Buffer::new(&merged).messages;
        self.hmac = hmac;
        self.seq += 1;
    }

    pub fn show(&mut self, ui: &mut Ui) -> bool {
        let mut sent = false;
        let theme = ui.ctx().get_lb_theme();

        let available_width = ui.available_width();
        let composer_height = 52.0_f32;
        let composer_bottom_inset = 16.0_f32;

        let text_color = theme.neutral_fg();
        let secondary_color = theme.neutral_fg_secondary();

        let h_pad = 10.0_f32;
        let v_pad = 6.0_f32;
        let h_margin = 12.0_f32;
        let row_gap = 4.0_f32;
        let corner = 10_u8;
        let max_inner_w = available_width * 0.72 - h_pad * 2.0;
        let top_margin = 15.0_f32;
        let bottom_pad = 15.0_f32;

        // Pre-compute total content height (outside scroll area) so we can
        // calculate the top padding needed to push short content to the bottom,
        // matching the editor's viewport-fill pattern.
        let n = self.messages.len();
        let content_h: f32 = (0..n)
            .map(|i| {
                let msg = &self.messages[i];
                let is_mine = msg.from == self.username;
                let last_in_run = i + 1 >= n || self.messages[i + 1].from != msg.from;

                let first_in_run = i == 0 || self.messages[i - 1].from != msg.from;
                let name_h = if !is_mine && first_in_run {
                    ui.fonts(|f| {
                        f.layout_no_wrap(
                            msg.from.clone(),
                            egui::FontId::proportional(11.0),
                            egui::Color32::WHITE,
                        )
                        .rect
                        .height()
                    })
                } else {
                    0.0
                };

                let content_h = ui.fonts(|f| {
                    let mut job = LayoutJob::default();
                    job.wrap.max_width = max_inner_w;
                    job.append(
                        &msg.content,
                        0.0,
                        TextFormat {
                            font_id: egui::FontId::proportional(14.0),
                            color: egui::Color32::WHITE,
                            ..Default::default()
                        },
                    );
                    if last_in_run {
                        job.append(
                            &format!("  {}", format_ts(msg.ts)),
                            0.0,
                            TextFormat {
                                font_id: egui::FontId::proportional(11.0),
                                color: egui::Color32::WHITE,
                                valign: egui::Align::BOTTOM,
                                ..Default::default()
                            },
                        );
                    }
                    f.layout_job(job).rect.height()
                });

                name_h + content_h + v_pad * 2.0 + row_gap
            })
            .sum::<f32>();

        ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
        ui.vertical(|ui| {
            // Scroll area: takes all height except the composer, exactly as the
            // editor does with its mobile toolbar.
            let (_, scroll_area_rect) = ui.allocate_space(vec2(
                available_width,
                ui.available_height() - composer_height - composer_bottom_inset,
            ));
            ui.scope_builder(egui::UiBuilder::new().max_rect(scroll_area_rect), |ui| {
                ui.set_clip_rect(scroll_area_rect.intersect(ui.clip_rect()));
                ScrollArea::vertical()
                    .id_salt("chat_messages")
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        const MAX_WIDTH: f32 = 800.0;
                        let col_width = available_width.min(MAX_WIDTH);
                        let padding = (available_width - col_width) / 2.0;

                        ui.allocate_space(vec2(ui.available_width(), 0.0));

                        ui.add_space(top_margin);

                        let n = self.messages.len();
                        for i in 0..n {
                            let msg = &self.messages[i];
                            let is_mine = msg.from == self.username;
                            let last_in_run = i + 1 >= n || self.messages[i + 1].from != msg.from;

                            let bubble_color = if is_mine {
                                theme
                                    .bg()
                                    .get_color(Palette::Blue)
                                    .lerp_to_gamma(theme.neutral_bg(), 0.5)
                            } else {
                                theme.neutral_bg_secondary()
                            };

                            let first_in_run = i == 0 || self.messages[i - 1].from != msg.from;
                            let name_galley = if !is_mine && first_in_run {
                                let name_color = theme.fg().get_color(username_color(&msg.from));
                                Some(ui.fonts(|f| {
                                    f.layout_no_wrap(
                                        msg.from.clone(),
                                        egui::FontId::proportional(11.0),
                                        name_color,
                                    )
                                }))
                            } else {
                                None
                            };

                            let content_galley = ui.fonts(|f| {
                                let mut job = LayoutJob::default();
                                job.wrap.max_width = max_inner_w;
                                job.append(
                                    &msg.content,
                                    0.0,
                                    TextFormat {
                                        font_id: egui::FontId::proportional(14.0),
                                        color: text_color,
                                        ..Default::default()
                                    },
                                );
                                if last_in_run {
                                    job.append(
                                        &format!("  {}", format_ts(msg.ts)),
                                        0.0,
                                        TextFormat {
                                            font_id: egui::FontId::proportional(11.0),
                                            color: secondary_color,
                                            valign: egui::Align::BOTTOM,
                                            ..Default::default()
                                        },
                                    );
                                }
                                f.layout_job(job)
                            });

                            let name_w = name_galley.as_ref().map_or(0.0, |g| g.rect.width());
                            let name_h = name_galley.as_ref().map_or(0.0, |g| g.rect.height());
                            let content_h = content_galley.rect.height();

                            let bubble_inner_w = name_w.max(content_galley.rect.width());
                            let bubble_w = bubble_inner_w + h_pad * 2.0;
                            let bubble_h = name_h + content_h + v_pad * 2.0;

                            let (_id, row_rect) =
                                ui.allocate_space(vec2(ui.available_width(), bubble_h + row_gap));

                            let col_left = row_rect.min.x + padding;
                            let col_right = col_left + col_width;

                            let bubble_x = if is_mine {
                                col_right - h_margin - bubble_w
                            } else {
                                col_left + h_margin
                            };
                            let bubble_rect = Rect::from_min_size(
                                pos2(bubble_x, row_rect.min.y),
                                vec2(bubble_w, bubble_h),
                            );

                            ui.painter().rect_filled(
                                bubble_rect,
                                CornerRadius::same(corner),
                                bubble_color,
                            );

                            let mut text_y = bubble_rect.min.y + v_pad;

                            if let Some(ng) = name_galley {
                                ui.painter().galley(
                                    pos2(bubble_rect.min.x + h_pad, text_y),
                                    ng,
                                    egui::Color32::PLACEHOLDER,
                                );
                                text_y += name_h;
                            }

                            ui.painter().galley(
                                pos2(bubble_rect.min.x + h_pad, text_y),
                                content_galley,
                                text_color,
                            );
                        }

                        match &self.agent_status {
                            AgentStatus::Thinking => {
                                ui.add_space(row_gap);
                                let galley = ui.fonts(|f| {
                                    f.layout_no_wrap(
                                        "Thinking...".into(),
                                        egui::FontId::proportional(13.0),
                                        secondary_color,
                                    )
                                });
                                let (_id, rect) = ui.allocate_space(vec2(
                                    ui.available_width(),
                                    galley.rect.height() + v_pad * 2.0,
                                ));
                                let col_left = rect.min.x + padding;
                                ui.painter().galley(
                                    pos2(col_left + h_margin + h_pad, rect.min.y + v_pad),
                                    galley,
                                    secondary_color,
                                );
                                ui.ctx().request_repaint_after(
                                    std::time::Duration::from_millis(500),
                                );
                            }
                            AgentStatus::ToolsPending { requests, .. } => {
                                ui.add_space(row_gap * 2.0);
                                let col_left = ui.min_rect().min.x + padding;

                                // Tool descriptions
                                for req in requests {
                                    let galley = ui.fonts(|f| {
                                        f.layout_no_wrap(
                                            format!("  {}", req.description),
                                            egui::FontId::proportional(13.0),
                                            text_color,
                                        )
                                    });
                                    let (_id, rect) = ui.allocate_space(vec2(
                                        ui.available_width(),
                                        galley.rect.height() + 4.0,
                                    ));
                                    ui.painter().galley(
                                        pos2(col_left + h_margin + h_pad, rect.min.y + 2.0),
                                        galley,
                                        text_color,
                                    );
                                }

                                // Approve / Deny buttons
                                ui.add_space(4.0);
                                ui.horizontal(|ui| {
                                    ui.add_space(col_left + h_margin + h_pad);
                                    if ui.button("Allow").clicked() {
                                        if let AgentStatus::ToolsPending { approve_tx, .. } =
                                            std::mem::replace(
                                                &mut self.agent_status,
                                                AgentStatus::Thinking,
                                            )
                                        {
                                            let _ = approve_tx.send(true);
                                        }
                                    }
                                    if ui.button("Deny").clicked() {
                                        if let AgentStatus::ToolsPending { approve_tx, .. } =
                                            std::mem::replace(
                                                &mut self.agent_status,
                                                AgentStatus::Idle,
                                            )
                                        {
                                            let _ = approve_tx.send(false);
                                            self.agent_pending = false;
                                        }
                                    }
                                });
                                ui.add_space(row_gap);
                            }
                            AgentStatus::ToolRunning { name } => {
                                ui.add_space(row_gap);
                                let galley = ui.fonts(|f| {
                                    f.layout_no_wrap(
                                        format!("{name}..."),
                                        egui::FontId::proportional(13.0),
                                        secondary_color,
                                    )
                                });
                                let (_id, rect) = ui.allocate_space(vec2(
                                    ui.available_width(),
                                    galley.rect.height() + v_pad * 2.0,
                                ));
                                let col_left = rect.min.x + padding;
                                ui.painter().galley(
                                    pos2(col_left + h_margin + h_pad, rect.min.y + v_pad),
                                    galley,
                                    secondary_color,
                                );
                                ui.ctx().request_repaint_after(
                                    std::time::Duration::from_millis(500),
                                );
                            }
                            AgentStatus::Idle => {}
                        }

                        let viewport_h = ui.max_rect().height();
                        let end_pad = (viewport_h - content_h - top_margin).max(bottom_pad);
                        ui.add_space(end_pad);
                    });
            });

            // Composer: remaining height, same pattern as editor's mobile toolbar.
            let (_, composer_rect) =
                ui.allocate_space(vec2(available_width, composer_height + composer_bottom_inset));
            ui.scope_builder(egui::UiBuilder::new().max_rect(composer_rect), |ui| {
                const MAX_WIDTH: f32 = 800.0;
                let col_width = available_width.min(MAX_WIDTH);
                let padding = (available_width - col_width) / 2.0;
                let h_inset = padding + 48.0;

                let bubble_color = theme.neutral_bg_secondary();
                let v_gap = 6.0_f32;
                let bottom_gap = 22.0_f32;
                let bubble_rect = Rect::from_min_max(
                    pos2(composer_rect.min.x + h_inset, composer_rect.min.y + v_gap),
                    pos2(composer_rect.max.x - h_inset, composer_rect.max.y - bottom_gap),
                );
                ui.painter()
                    .rect_filled(bubble_rect, CornerRadius::same(10_u8), bubble_color);

                let text_rect = Rect::from_center_size(
                    bubble_rect.center(),
                    vec2(bubble_rect.width() - h_pad * 2.0, 20.0),
                );
                let te_id = egui::Id::new("chat_composer");
                let has_focus = ui.ctx().memory(|m| m.has_focus(te_id));
                let enter_pressed =
                    has_focus && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Enter));

                let te = TextEdit::singleline(&mut self.input)
                    .id(te_id)
                    .hint_text("Message...")
                    .frame(false)
                    .font(egui::FontId::proportional(14.0));
                let resp = ui.put(text_rect, te);

                if !self.initialized {
                    resp.request_focus();
                    self.initialized = true;
                }

                if enter_pressed && !self.agent_pending {
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
