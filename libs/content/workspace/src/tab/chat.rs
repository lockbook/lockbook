//! Chat tab — markdown-rendered messages in a scrollable transcript, with a
//! multiline composer at the bottom. The document on disk is
//! newline-delimited JSON (`{from, content, ts}`), merged across devices via
//! `lb_rs::model::chat::Buffer::merge` (symmetric union over timestamp).

use std::sync::{Arc, RwLock};

use chrono::{DateTime, Local, Utc};
use egui::{
    Color32, CornerRadius, Galley, Id, Key, Modifiers, Rect, ScrollArea, Sense, Ui, pos2, vec2,
};
use lb_rs::Uuid;
use lb_rs::model::chat::{Buffer, Message};
use lb_rs::model::file_metadata::DocumentHmac;

use crate::GlyphonRendererCallback;
use crate::file_cache::FileCache;
use crate::resolvers::FileCacheLinkResolver;
use crate::tab::markdown_editor::{MdEdit, MdLabel};
use crate::theme::palette_v2::{Palette, ThemeExt, username_color};

const MAX_WIDTH: f32 = 800.0;
const H_PAD: f32 = 12.0;
const V_PAD: f32 = 10.0;
const H_MARGIN: f32 = 12.0;
const ROW_GAP: f32 = 4.0;
const CORNER: u8 = 10;
const TOP_MARGIN: f32 = 15.0;
const BOTTOM_PAD: f32 = 15.0;
const COMPOSER_MAX_HEIGHT: f32 = 160.0;
const COMPOSER_BOTTOM_INSET: f32 = 16.0;

/// A message paired with the label that renders its markdown body. Labels
/// are per-message so each one's `LayoutCache` can actually memoize across
/// frames — rotating one shared label through every message would invalidate
/// the cache on every rotation.
pub struct Entry {
    pub msg: Message,
    pub label: MdLabel,
}

/// Per-row layout plan produced by the first pass. All rects absolute; pass 2
/// paints with no egui layout involvement.
struct RowPlan {
    bubble_rect: Rect,
    bubble_color: Color32,
    name_galley: Option<Arc<Galley>>,
    name_h: f32,
    ts_galley: Option<Arc<Galley>>,
    content_h: f32,
}

impl Entry {
    fn new(
        msg: Message, ctx: &egui::Context, files: Arc<RwLock<FileCache>>, chat_id: Uuid,
    ) -> Self {
        let mut label = MdLabel::new(ctx.clone());
        label.renderer.files = Arc::clone(&files);
        label.renderer.link_resolver = Box::new(FileCacheLinkResolver::new(files, chat_id));
        Self { msg, label }
    }
}

pub struct Chat {
    pub id: Uuid,
    pub hmac: Option<DocumentHmac>,
    pub entries: Vec<Entry>,
    pub composer: MdEdit,
    pub username: String,
    pub seq: usize,
    pub initialized: bool,
    ctx: egui::Context,
}

impl Chat {
    pub fn new(
        bytes: &[u8], id: Uuid, hmac: Option<DocumentHmac>, username: String, ctx: egui::Context,
        files: Arc<RwLock<FileCache>>,
    ) -> Self {
        let entries = Buffer::new(bytes)
            .messages
            .into_iter()
            .map(|m| Entry::new(m, &ctx, Arc::clone(&files), id))
            .collect();
        let mut composer = MdEdit::empty(ctx.clone());
        composer.renderer.files = Arc::clone(&files);
        composer.renderer.link_resolver =
            Box::new(FileCacheLinkResolver::new(Arc::clone(&files), id));
        composer.file_id = id;
        Self { id, hmac, entries, composer, username, seq: 0, initialized: false, ctx }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        Buffer { messages: self.entries.iter().map(|e| e.msg.clone()).collect() }.serialize()
    }

    /// Merge in a freshly-synced version. `bytes` is the remote; the local
    /// state is `self`. An empty base is sound because deletions aren't a
    /// thing today — symmetric union over timestamps.
    ///
    /// Unchanged entries keep their existing label (and therefore its layout
    /// cache); new entries get a fresh label.
    pub fn reload(&mut self, bytes: &[u8], hmac: Option<DocumentHmac>) {
        let local = self.to_bytes();
        let merged = Buffer::merge(&[], &local, bytes);
        let merged_msgs = Buffer::new(&merged).messages;

        let mut old: Vec<Entry> = std::mem::take(&mut self.entries);
        self.entries = merged_msgs
            .into_iter()
            .map(|msg| match old.iter().position(|e| e.msg == msg) {
                Some(idx) => old.swap_remove(idx),
                None => {
                    Entry::new(msg, &self.ctx, Arc::clone(&self.composer.renderer.files), self.id)
                }
            })
            .collect();

        self.hmac = hmac;
        self.seq += 1;
    }

    /// Mark saved after a successful write — called from the workspace save
    /// completion path.
    pub fn saved(&mut self, hmac: DocumentHmac) {
        self.hmac = Some(hmac);
    }

    /// Returns true if the user sent a message this frame.
    pub fn show(&mut self, ui: &mut Ui) -> bool {
        let mut sent = false;
        let theme = ui.ctx().get_lb_theme();
        let available_width = ui.available_width();
        let col_width = available_width.min(MAX_WIDTH);
        let max_bubble_content_w = (col_width * 0.72 - H_PAD * 2.0).max(120.0);
        let text_color = theme.neutral_fg();
        let secondary_color = theme.neutral_fg_secondary();

        ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
        let full_rect = ui.available_rect_before_wrap();

        let composer_id = Id::new("chat_composer");
        if !self.initialized {
            ui.memory_mut(|m| m.request_focus(composer_id));
            self.initialized = true;
        }

        // Cmd/Ctrl+Enter → send. Consume before handle_input so the composer
        // doesn't translate the Enter into a Newline.
        let composer_focused = ui.memory(|m| m.has_focus(composer_id));
        let send_requested = composer_focused
            && ui
                .ctx()
                .input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::Enter));

        // Composer input phase — keyboard / completions / internal events.
        let _ = self.composer.handle_input(ui.ctx(), composer_id);

        // Measure at the exact render width so the composer bubble grows
        // same-frame. The re-parse inside `show` below hits the layout cache.
        // `48.0` and `H_PAD` mirror the h_inset / shrink geometry below.
        let composer_inner_w = (col_width - 2.0 * 48.0 - 2.0 * H_PAD).max(0.0);
        let measured_h = self.composer.measure_height(composer_inner_w);

        // Autogrow with a max cap, no lower floor — a lower floor makes a
        // single-line composer bottom-heavy (content is top-anchored).
        let composer_height = (measured_h + V_PAD * 2.0).min(COMPOSER_MAX_HEIGHT);
        let transcript_rect = Rect::from_min_max(
            full_rect.min,
            pos2(full_rect.max.x, full_rect.max.y - composer_height - COMPOSER_BOTTOM_INSET),
        );
        let mut text_areas = Vec::new();

        ui.scope_builder(egui::UiBuilder::new().max_rect(transcript_rect), |ui| {
            ui.set_clip_rect(transcript_rect.intersect(ui.clip_rect()));
            ScrollArea::vertical()
                .id_salt("chat_messages")
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    let origin = ui.cursor().min;
                    let col_pad = (available_width - col_width) / 2.0;
                    let col_left = origin.x + col_pad;
                    let col_right = col_left + col_width;

                    // pass 1: measure each message and compute its bubble rect
                    // against a running y. This populates each label's layout
                    // cache so pass 2's paint is near-free.
                    let n = self.entries.len();
                    let mut plans: Vec<RowPlan> = Vec::with_capacity(n);
                    let mut y = origin.y + TOP_MARGIN;
                    for i in 0..n {
                        let from = self.entries[i].msg.from.clone();
                        let ts = self.entries[i].msg.ts;
                        let is_mine = from == self.username;
                        let first_in_run = i == 0 || self.entries[i - 1].msg.from != from;
                        let last_in_run = i + 1 >= n || self.entries[i + 1].msg.from != from;

                        let name_galley = if !is_mine && first_in_run {
                            let name_color = theme.fg().get_color(username_color(&from));
                            Some(ui.fonts(|f| {
                                f.layout_no_wrap(
                                    from.clone(),
                                    egui::FontId::proportional(11.0),
                                    name_color,
                                )
                            }))
                        } else {
                            None
                        };

                        let ts_galley = if last_in_run {
                            Some(ui.fonts(|f| {
                                f.layout_no_wrap(
                                    format_ts(ts),
                                    egui::FontId::proportional(11.0),
                                    secondary_color,
                                )
                            }))
                        } else {
                            None
                        };

                        let entry = &mut self.entries[i];
                        let content_h =
                            entry.label.height(&entry.msg.content, max_bubble_content_w);

                        let name_h = name_galley.as_ref().map_or(0.0, |g| g.rect.height());
                        let ts_h = ts_galley.as_ref().map_or(0.0, |g| g.rect.height());

                        let bubble_w = max_bubble_content_w + H_PAD * 2.0;
                        let bubble_h = name_h + content_h + ts_h + V_PAD * 2.0;
                        let bubble_x = if is_mine {
                            col_right - H_MARGIN - bubble_w
                        } else {
                            col_left + H_MARGIN
                        };
                        let bubble_rect =
                            Rect::from_min_size(pos2(bubble_x, y), vec2(bubble_w, bubble_h));

                        let bubble_color = if is_mine {
                            theme
                                .bg()
                                .get_color(Palette::Blue)
                                .lerp_to_gamma(theme.neutral_bg(), 0.5)
                        } else {
                            theme.neutral_bg_secondary()
                        };

                        plans.push(RowPlan {
                            bubble_rect,
                            bubble_color,
                            name_galley,
                            name_h,
                            ts_galley,
                            content_h,
                        });
                        y += bubble_h + ROW_GAP;
                    }

                    // Allocate total footprint so ScrollArea sees the right
                    // height (stick-to-bottom depends on this).
                    let total_h = (y - origin.y) + BOTTOM_PAD;
                    let _ = ui.allocate_exact_size(vec2(available_width, total_h), Sense::hover());

                    // pass 2: paint absolute. No egui layout calls.
                    for (i, plan) in plans.into_iter().enumerate() {
                        ui.painter().rect_filled(
                            plan.bubble_rect,
                            CornerRadius::same(CORNER),
                            plan.bubble_color,
                        );

                        let mut text_y = plan.bubble_rect.min.y + V_PAD;
                        if let Some(ng) = plan.name_galley {
                            ui.painter().galley(
                                pos2(plan.bubble_rect.min.x + H_PAD, text_y),
                                ng,
                                text_color,
                            );
                            text_y += plan.name_h;
                        }

                        let content_top = pos2(plan.bubble_rect.min.x + H_PAD, text_y);
                        let entry = &mut self.entries[i];
                        let (areas, _) = entry.label.paint_at(
                            ui,
                            &entry.msg.content,
                            content_top,
                            max_bubble_content_w,
                        );
                        text_areas.extend(areas);
                        text_y += plan.content_h;

                        if let Some(tg) = plan.ts_galley {
                            let tg_w = tg.rect.width();
                            ui.painter().galley(
                                pos2(plan.bubble_rect.max.x - H_PAD - tg_w, text_y),
                                tg,
                                secondary_color,
                            );
                        }
                    }
                });
        });

        // Transcript text callback. Submit before composer so the composer's
        // own callback (inside show) lands on a later glyphon layer.
        if !text_areas.is_empty() {
            ui.painter()
                .add(egui_wgpu_renderer::egui_wgpu::Callback::new_paint_callback(
                    ui.max_rect(),
                    GlyphonRendererCallback::new(text_areas),
                ));
        }

        // Composer bubble + body.
        let composer_rect = Rect::from_min_max(
            pos2(full_rect.min.x, full_rect.max.y - composer_height - COMPOSER_BOTTOM_INSET),
            full_rect.max,
        );
        let col_pad = (available_width - col_width) / 2.0;
        let h_inset = col_pad + 48.0;
        let bubble_rect = Rect::from_min_max(
            pos2(composer_rect.min.x + h_inset, composer_rect.min.y),
            pos2(composer_rect.max.x - h_inset, composer_rect.max.y - COMPOSER_BOTTOM_INSET),
        );
        ui.painter().rect_filled(
            bubble_rect,
            CornerRadius::same(CORNER),
            theme.neutral_bg_secondary(),
        );

        let inner_rect = bubble_rect.shrink2(vec2(H_PAD, V_PAD));

        // Composer draw. Submits its own text callback internally.
        self.composer.show(ui, inner_rect, composer_id);

        if send_requested {
            let content = self
                .composer
                .renderer
                .buffer
                .current
                .text
                .trim()
                .to_string();
            if !content.is_empty() {
                let msg =
                    Message { from: self.username.clone(), content, ts: Utc::now().timestamp() };
                self.entries.push(Entry::new(
                    msg,
                    &self.ctx,
                    Arc::clone(&self.composer.renderer.files),
                    self.id,
                ));
                self.composer.clear();
                self.seq += 1;
                ui.memory_mut(|m| m.request_focus(composer_id));
                sent = true;
            }
        }

        // Popups land last so they composite over composer + transcript.
        self.composer.show_completions(ui);

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
