//! The chat tab's face: a transcript column, the live run under it, and a
//! composer that holds its own controls. Everything is placed by hand on the
//! design system's plates, rows, and buttons.

use egui::text::{LayoutJob, TextWrapping};
use egui::{
    Align, Align2, Color32, CursorIcon, FontFamily, FontId, Id, Key, Layout, Modifiers, Rect,
    ScrollArea, Sense, Stroke, StrokeKind, Ui, UiBuilder, Vec2, pos2, vec2,
};
use lb_chat::{Cmd, Place, Provider, prettify};
use lb_rs::Uuid;
use lb_rs::model::chat::{Body, Entry};
use serde_json::Value;

use super::setup::{Key as KeyNeed, OWN, TEMPLATES};
use super::{Chat, Setup, rows};
use crate::file_cache::FilesExt;
use crate::style::chrome::{row_wash_inset, shortcut_enter, shortcut_esc};
use crate::style::file_name;
use crate::style::interact::{ControlFills, interact_fill_response, quiet_canvas_fills};
use crate::style::space::control as control_space;
use crate::style::{
    Button, FG_HOVER, FG_PRESS, Field, Icon, Radius, STROKE_HAIRLINE, Space, Spacer, Theme,
    ThemeExt, TypeRole, claim, context_menu, control_height, control_icon_hit, icon_button_hit,
    measure_file_name, paint_file_name, phosphor, phosphor_ui_font_id, place_at, sense_click,
    tip_text, with_overlay_scroll,
};
use crate::style::{
    SheetFooterOpts, expand_ancestors_of, folder_tree_scroll_key, show_folder_sheet, tree_metrics,
};
use crate::widgets::{GlyphonLabel, TextOverflow};

const COLUMN_W: f32 = 720.0;
const COMPOSER_MAX: f32 = 160.0;
const BUBBLE_FRACTION: f32 = 0.85;
/// The provider mark's size on the empty chat.
const MARK_PX: f32 = 28.0;
const CHIP_MAX_W: f32 = 180.0;
const CAPTION_SIZE: f32 = 12.0;
const CAPTION_LH: f32 = 16.8;
const COPIED_SECS: f64 = 1.2;

#[derive(Clone)]
enum ModelChoice {
    /// A pinned `provider/model` selection.
    Use(String),
    Browse,
    AddProvider,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    User,
    Assistant,
    Tool,
    Error,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Status {
    Running,
    Done,
    Failed,
}

/// What the composer measured for this frame.
struct ComposerGeom {
    /// Text and controls share one row; otherwise the controls sit under it.
    single: bool,
    measured: f32,
    text_w: f32,
    model_w: f32,
    folder_w: f32,
}

fn gap(prev: Kind, next: Kind) -> Space {
    match (prev, next) {
        (Kind::Tool, Kind::Tool) => Space::Xs,
        (Kind::Assistant, Kind::Tool) | (Kind::Tool, Kind::Assistant) => Space::Sm,
        _ => Space::Md,
    }
}

fn kind(entry: &Entry) -> Option<Kind> {
    match entry.body {
        Body::User { .. } => Some(Kind::User),
        Body::Assistant { .. } => Some(Kind::Assistant),
        Body::Tool { .. } => Some(Kind::Tool),
        Body::Error { .. } => Some(Kind::Error),
        Body::Other(_) => None,
    }
}

impl Chat {
    /// Returns (composer text rect, composer selection changed, composer text changed).
    pub fn show(&mut self, ui: &mut Ui) -> (Rect, bool, bool) {
        self.pump();
        let t = ui.ctx().get_lb_theme();
        ui.spacing_mut().item_spacing = Vec2::ZERO;
        let full = ui.max_rect();
        let col_w = (full.width() - Space::Lg.pts() * 2.0).clamp(160.0, COLUMN_W);
        let col_x = (full.center().x - col_w / 2.0).round();
        let ready = self.is_ready();
        let composer_id = Id::new(("chat_composer", self.id));

        let (pad_x, pad_y, gap, hit) =
            (Space::Md.pts(), Space::Sm.pts(), Space::Xs.pts(), control_height());
        let model_w = chip_width(ui, TypeRole::Body.size(), &self.model_label());
        let folder_w = scope_chip_width(ui, &self.folder_label(), self.scope_chosen());
        let trailing = folder_w + gap + model_w + gap + hit;
        let wrap_w = (col_w - pad_x * 2.0).max(1.0);
        let inner_w = (col_w - pad_x * 2.0 - gap - trailing).max(1.0);
        let row = self.composer.row_height();
        let wide_h = self.composer.measure_height(wrap_w);
        let measured =
            if wide_h <= row + 1.0 { self.composer.measure_height(inner_w) } else { wide_h };
        let single = measured <= row + 1.0;
        let geom = ComposerGeom {
            single,
            measured,
            text_w: if single { inner_w } else { wrap_w },
            model_w,
            folder_w,
        };
        let content_h = if single { measured.max(hit) } else { measured + gap + hit };
        let composer_h = if ready {
            (content_h + pad_y * 2.0).clamp(hit + pad_y * 2.0, COMPOSER_MAX)
        } else {
            0.0
        };
        let ask_h = match &self.pending_ask {
            Some((_, prompt)) => self.ask_height(ui, prompt, col_w),
            None => 0.0,
        };
        let (lg, md, sm) = (Space::Lg.pts(), Space::Md.pts(), Space::Sm.pts());
        let bottom_h = if ready {
            md + composer_h + if ask_h > 0.0 { sm + ask_h } else { 0.0 } + lg
        } else {
            0.0
        };

        let transcript_rect =
            Rect::from_min_max(full.min, pos2(full.max.x, (full.max.y - bottom_h).round()));
        let mut command = None;
        let scroll_id = Id::new(("chat_scroll", self.id));
        let mut at_bottom = true;
        ui.scope_builder(UiBuilder::new().max_rect(transcript_rect), |ui| {
            ui.set_clip_rect(transcript_rect.intersect(ui.clip_rect()));
            at_bottom = with_overlay_scroll(ui, scroll_id, |ui| {
                let mut text_areas = Vec::new();
                let out = ScrollArea::vertical()
                    .id_salt(scroll_id)
                    .stick_to_bottom(true)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing = Vec2::ZERO;
                        ui.horizontal_top(|ui| {
                            ui.add_space((col_x - ui.max_rect().left()).max(0.0));
                            ui.vertical(|ui| {
                                ui.set_width(col_w);
                                ui.spacing_mut().item_spacing = Vec2::ZERO;
                                if ready {
                                    ui.add(Spacer::new(Space::Lg));
                                    self.show_transcript(
                                        ui,
                                        &t,
                                        col_w,
                                        transcript_rect.height(),
                                        &mut text_areas,
                                        &mut command,
                                    );
                                    ui.add(Spacer::new(Space::Md));
                                } else {
                                    ui.add(Spacer::new(Space::Xl));
                                    self.show_setup(ui, &t, col_w);
                                    ui.add(Spacer::new(Space::Xl));
                                }
                            });
                        });
                        if std::mem::take(&mut self.scroll_to_bottom) {
                            ui.scroll_to_cursor(Some(Align::BOTTOM));
                        }
                    });
                if !text_areas.is_empty() {
                    ui.painter()
                        .add(egui_wgpu_renderer::egui_wgpu::Callback::new_paint_callback(
                            transcript_rect,
                            crate::GlyphonRendererCallback::new(text_areas),
                        ));
                }
                let at_bottom =
                    out.state.offset.y + out.inner_rect.height() >= out.content_size.y - 1.0;
                (at_bottom, out.state.offset.y, out.id)
            });
        });
        if let Some(cmd) = command {
            self.send_cmd(cmd);
        }
        if !ready {
            return (Rect::NOTHING, false, false);
        }
        if !at_bottom {
            self.show_jump_to_latest(ui, &t, full.center().x, transcript_rect.bottom() - sm);
        }

        let mut y = full.max.y - lg;
        let composer_rect =
            Rect::from_min_max(pos2(col_x, (y - composer_h).round()), pos2(col_x + col_w, y));
        y -= composer_h + sm;
        let ask_rect = Rect::from_min_max(pos2(col_x, y - ask_h), pos2(col_x + col_w, y));
        if ask_h > 0.0 {
            self.show_ask(ui, &t, ask_rect);
        }
        let column = |top: f32, bottom: f32| {
            Rect::from_min_max(pos2(col_x, top), pos2(col_x + col_w, bottom))
        };
        Spacer::paint_at(
            ui,
            Space::Md,
            column(transcript_rect.bottom(), transcript_rect.bottom() + md),
        );
        if ask_h > 0.0 {
            Spacer::paint_at(ui, Space::Sm, column(ask_rect.bottom(), composer_rect.top()));
        }
        Spacer::paint_at(ui, Space::Lg, column(composer_rect.bottom(), full.max.y));
        let out = self.show_composer(ui, &t, composer_rect, composer_id, &geom);
        claim(ui, Rect::from_min_max(pos2(full.min.x, transcript_rect.max.y), full.max));
        out
    }

    fn model_label(&self) -> String {
        if let Some(m) = self.listed_model() {
            return m.label();
        }
        match &self.provider {
            Some(Ok(p)) if !p.model.is_empty() => prettify(&p.model),
            _ => "model".into(),
        }
    }

    fn provider_name(&self) -> String {
        match &self.provider {
            Some(Ok(p)) => p.label(),
            _ => "the provider".into(),
        }
    }

    /// The current provider's mark at `px`, tinted when drawn.
    fn provider_mark(&mut self, ctx: &egui::Context, px: f32) -> Icon {
        let name = match &self.provider {
            Some(Ok(p)) => p.name.clone(),
            _ => String::new(),
        };
        self.mark(ctx, &name, px)
    }

    /// The folder the agent works in, by name; a count when grants widened it.
    fn folder_label(&self) -> String {
        let scope = self.scope();
        let name = folder_name(&scope);
        match self.settings().include.len() {
            0 | 1 => name,
            n => format!("{name} +{}", n - 1),
        }
    }

    fn sheet_open(&self) -> bool {
        self.scope_open || self.models_open
    }

    /// Whether a folder other than the chat's own was chosen.
    fn scope_chosen(&self) -> bool {
        !self.settings().include.is_empty()
    }

    fn scope_id(&self) -> Uuid {
        let files = self.files.read().unwrap();
        files
            .by_path(&self.scope())
            .map(|f| f.id)
            .unwrap_or_else(|| files.root().id)
    }

    fn open_scope_sheet(&mut self, ctx: &egui::Context) {
        let id = self.scope_id();
        {
            let files = self.files.read().unwrap();
            expand_ancestors_of(&*files, id, &mut self.scope_expanded);
        }
        ctx.data_mut(|d| {
            d.remove::<bool>(folder_tree_scroll_key("chat_scope"));
        });
        self.scope_dest = Some(id);
        self.scope_open = true;
    }

    fn close_scope_sheet(&mut self) {
        self.scope_open = false;
        self.scope_dest = None;
    }

    /// Makes `id` the chat's folder; the chat's own folder clears the choice.
    fn choose_scope(&mut self, id: Uuid) {
        let path = self.files.read().unwrap().path(id);
        let mut s = self.settings();
        s.include = if path == self.working_dir { Vec::new() } else { vec![path] };
        self.set_settings(s);
    }

    /// The folder sheet while it is open. Returns whether it consumed the
    /// frame's Enter.
    fn show_scope_sheet(&mut self, ui: &mut Ui, t: &Theme, opened_now: bool) {
        if !self.scope_open {
            return;
        }
        let (enter, esc) = ui.ctx().input_mut(|i| {
            (
                i.consume_key(Modifiers::NONE, Key::Enter)
                    || i.consume_key(Modifiers::COMMAND, Key::Enter),
                i.consume_key(Modifiers::NONE, Key::Escape),
            )
        });
        let dest = self.scope_dest.or_else(|| Some(self.scope_id()));
        let files = self.files.read().unwrap();
        let mut out = show_folder_sheet(
            ui.ctx(),
            t,
            &*files,
            &mut self.scope_expanded,
            dest,
            &[],
            "chat_scope",
            "Folder",
            "Choose the folder this chat can read and edit.",
            "Done",
            SheetFooterOpts::default()
                .divider(false)
                .quiet_primary(true)
                .primary_shortcut(shortcut_enter()),
            |_| {},
        );
        drop(files);
        if enter && dest.is_some() {
            out.confirm = true;
        }
        if let Some(id) = out.picked {
            self.scope_dest = Some(id);
        }
        if (out.dismiss && !opened_now) || esc {
            self.close_scope_sheet();
        } else if out.confirm {
            if let Some(id) = self.scope_dest.or(dest) {
                self.choose_scope(id);
            }
            self.close_scope_sheet();
        }
    }

    fn show_transcript(
        &mut self, ui: &mut Ui, t: &Theme, col_w: f32, transcript_h: f32,
        text_areas: &mut Vec<crate::TextBufferArea>, command: &mut Option<Cmd>,
    ) {
        let entries = self.visible_entries();
        if entries.is_empty() && !self.busy {
            self.show_empty(ui, t, col_w, transcript_h);
            return;
        }
        let me = self.account.username.clone();
        let last = entries.last().map(|e| e.id);
        let mut prev_kind = None;
        let mut prev_day = None;
        for entry in &entries {
            let Some(kind) = kind(entry) else { continue };
            let day = local_day(entry.ts);
            if day != prev_day {
                if prev_day.is_some() {
                    ui.add(Spacer::new(Space::Md));
                }
                if let Some(day) = day {
                    caption_centered(ui, t, col_w, &day_label(day));
                    ui.add(Spacer::new(Space::Sm));
                }
                prev_day = day;
                prev_kind = None;
            }
            if let Some(prev) = prev_kind {
                ui.add(Spacer::new(gap(prev, kind)));
            }
            let mine = entry.from == me;
            match &entry.body {
                Body::User { text, .. } => self.show_user(ui, t, col_w, entry, text, text_areas),
                Body::Assistant { text, interrupted, .. } => {
                    if !mine {
                        caption(ui, t, col_w, &format!("{}'s assistant", entry.from));
                    }
                    self.show_assistant(ui, t, col_w, entry.id, text, text_areas);
                    if *interrupted {
                        caption(ui, t, col_w, "stopped");
                    }
                }
                Body::Tool { name, args, result, ok, .. } => {
                    let status = if *ok { Status::Done } else { Status::Failed };
                    if self.tool_row(ui, t, col_w, entry.id, name, args, status) {
                        let detail = rows::detail(name, args, result);
                        self.show_tool_detail(ui, col_w, entry.id, &detail, text_areas);
                    }
                }
                Body::Error { text } => {
                    if !mine {
                        caption(ui, t, col_w, &format!("{}'s assistant", entry.from));
                    }
                    let retry = mine && Some(entry.id) == last && !self.busy;
                    if self.show_notice(ui, t, col_w, Notice::error(text, retry)) {
                        *command = Some(Cmd::Regenerate);
                    }
                }
                Body::Other(_) => {}
            }
            // Rows stack: the cursor ends under everything laid out so far.
            debug_assert!(
                ui.cursor().top() >= ui.min_rect().bottom() - 0.5,
                "a row left the cursor {} above what it drew",
                ui.min_rect().bottom() - ui.cursor().top()
            );
            prev_kind = Some(kind);
        }

        if let Some(call) = self.running_tool.clone() {
            ui.add(Spacer::new(gap(prev_kind.unwrap_or(Kind::User), Kind::Tool)));
            self.tool_row(ui, t, col_w, Uuid::nil(), &call.name, &call.args, Status::Running);
        } else if !self.streaming.is_empty() {
            ui.add(Spacer::new(Space::Md));
            let streaming = self.streaming.clone();
            text_areas.extend(self.streaming_label.show(ui, &streaming, col_w));
        } else if self.busy && self.pending_ask.is_none() {
            ui.add(Spacer::new(Space::Md));
            caption_pulsing(ui, t, col_w, "Thinking…");
        } else if !self.busy {
            let unanswered = entries
                .last()
                .is_some_and(|e| e.from == me && matches!(e.body, Body::User { .. }));
            if unanswered {
                ui.add(Spacer::new(Space::Md));
                if self.show_notice(ui, t, col_w, Notice::unfinished()) {
                    *command = Some(Cmd::Regenerate);
                }
            }
        }
    }

    /// Who you are about to talk to: the provider's mark, the model, and
    /// where messages go.
    fn show_empty(&mut self, ui: &mut Ui, t: &Theme, col_w: f32, transcript_h: f32) {
        let (heading_lh, body_lh) = (TypeRole::Heading.line_height(), TypeRole::Body.line_height());
        let privacy = privacy_line(self.provider.as_ref().and_then(|p| p.as_ref().ok()));
        let model = self.model_label();
        let mark = self.provider_mark(ui.ctx(), MARK_PX);

        let block_h = MARK_PX + Space::Md.pts() + heading_lh + Space::Xs.pts() + body_lh;
        let slot_h = (transcript_h - Space::Lg.pts() - Space::Md.pts()).max(block_h);
        let (rect, _) = ui.allocate_exact_size(vec2(col_w, slot_h), Sense::hover());
        let mut y = rect.center().y - block_h / 2.0;
        let mark_rect =
            Rect::from_min_size(pos2(rect.center().x - MARK_PX / 2.0, y), vec2(MARK_PX, MARK_PX));
        mark.paint(ui.painter(), mark_rect.center(), MARK_PX, t.neutral_fg());
        y += MARK_PX;
        let gap = Rect::from_min_size(pos2(rect.left(), y), vec2(col_w, Space::Md.pts()));
        Spacer::paint_at(ui, Space::Md, gap);
        y += Space::Md.pts();
        let title = Rect::from_min_size(pos2(rect.left(), y), vec2(col_w, heading_lh));
        place_at(ui, title, Layout::top_down(Align::Center), |ui| {
            ui.add(
                GlyphonLabel::new(&model, t.neutral_fg())
                    .font_size(TypeRole::Heading.size())
                    .line_height(heading_lh)
                    .max_width(col_w)
                    .text_overflow(TextOverflow::EndEllipsis),
            );
        });
        y += heading_lh;
        let gap = Rect::from_min_size(pos2(rect.left(), y), vec2(col_w, Space::Xs.pts()));
        Spacer::paint_at(ui, Space::Xs, gap);
        y += Space::Xs.pts();
        let sub = Rect::from_min_size(pos2(rect.left(), y), vec2(col_w, body_lh));
        place_at(ui, sub, Layout::top_down(Align::Center), |ui| {
            ui.add(
                GlyphonLabel::new(&privacy, t.neutral_fg_secondary())
                    .font_size(TypeRole::Body.size())
                    .line_height(body_lh)
                    .max_width(col_w)
                    .text_overflow(TextOverflow::EndEllipsis),
            );
        });
    }

    fn show_user(
        &mut self, ui: &mut Ui, t: &Theme, col_w: f32, entry: &Entry, text: &str,
        text_areas: &mut Vec<crate::TextBufferArea>,
    ) {
        let mine = entry.from == self.account.username;
        if !mine {
            caption(ui, t, col_w, &entry.from);
        }
        let pad = Space::Sm.pts();
        let max_w = col_w * BUBBLE_FRACTION;
        let inner_max = max_w - pad * 2.0;
        // Shrink to fit: the text's wrapped width at the markdown's own
        // metrics (its font size is its row height), plus a hair for rounding.
        let md = self.composer.row_height();
        let natural = GlyphonLabel::new(text, Color32::PLACEHOLDER)
            .font_size(md)
            .line_height(md)
            .max_width(inner_max)
            .measure(ui)
            .x;
        let bubble_w = (natural + pad * 2.0 + 2.0).clamp(control_height() * 2.0, max_w);
        let inner_w = bubble_w - pad * 2.0;
        let h = self.label(entry.id).height(text, inner_w) + pad * 2.0;
        let (row, _) = ui.allocate_exact_size(vec2(col_w, h), Sense::hover());
        let bubble = if mine {
            Rect::from_min_max(pos2(row.right() - bubble_w, row.top()), row.max)
        } else {
            Rect::from_min_size(row.min, vec2(bubble_w, h))
        };
        ui.painter()
            .rect_filled(bubble, Radius::Surface.corner(), t.neutral_bg_secondary());
        paint_inset_bands(ui, bubble, Space::Sm, Space::Sm);
        let (areas, painted) =
            self.label(entry.id)
                .paint_at(ui, text, bubble.min + vec2(pad, pad), inner_w);
        debug_assert!(
            painted.bottom() <= bubble.bottom() - pad + 0.5,
            "bubble {h} short of its text {}",
            painted.height()
        );
        text_areas.extend(areas);

        if mine && ui.rect_contains_pointer(row) && !self.busy {
            let slot = action_rect(bubble.left(), bubble.top() + pad, md);
            let resp = action_button(ui, t, slot, bubble.left(), phosphor::PENCIL, false);
            tip_text(ui.ctx(), &resp, "Edit and restart from here");
            if resp.clicked() {
                self.start_editing(ui, entry.id, text);
            }
        }
    }

    fn start_editing(&mut self, ui: &Ui, id: Uuid, text: &str) {
        self.editing = Some(id);
        self.composer.set_text(text);
        self.composer_text_seq += 1;
        ui.ctx()
            .memory_mut(|m| m.request_focus(Id::new(("chat_composer", self.id))));
    }

    fn show_assistant(
        &mut self, ui: &mut Ui, t: &Theme, col_w: f32, id: Uuid, text: &str,
        text_areas: &mut Vec<crate::TextBufferArea>,
    ) {
        if text.is_empty() {
            return;
        }
        let h = self.label(id).height(text, col_w);
        let (rect, _) = ui.allocate_exact_size(vec2(col_w, h), Sense::hover());
        let (areas, _) = self.label(id).paint_at(ui, text, rect.min, col_w);
        text_areas.extend(areas);

        // Copy shows while the pointer is over the reply, and a check for a
        // moment after it was used.
        let btn = action_rect(rect.left(), rect.top(), self.composer.row_height());
        let fid = Id::new(("chat_copied", id));
        let now = ui.input(|i| i.time);
        let copied = ui
            .ctx()
            .data(|d| d.get_temp::<f64>(fid))
            .is_some_and(|until| now < until);
        if copied {
            ui.ctx().request_repaint();
        }
        if !copied && !ui.rect_contains_pointer(rect.union(btn)) {
            return;
        }
        let glyph = if copied { phosphor::CHECK } else { phosphor::COPY };
        let resp = action_button(ui, t, btn, rect.left(), glyph, copied);
        tip_text(ui.ctx(), &resp, "Copy");
        if resp.clicked() {
            ui.ctx().copy_text(text.to_string());
            ui.ctx().data_mut(|d| d.insert_temp(fid, now + COPIED_SECS));
        }
    }

    /// One row for a tool call. Returns whether its detail is open.
    #[allow(clippy::too_many_arguments)]
    fn tool_row(
        &mut self, ui: &mut Ui, t: &Theme, col_w: f32, id: Uuid, name: &str, args: &Value,
        status: Status,
    ) -> bool {
        let row = rows::row(name, args);
        let settled = status != Status::Running;
        let open = settled && self.expanded.contains(&id);
        let (rect, _) = ui.allocate_exact_size(vec2(col_w, control_height()), Sense::hover());
        let sense = if settled { sense_click() } else { Sense::hover() };
        let resp = ui.interact(rect, Id::new(("chat_tool", id)), sense);
        if settled {
            let fill = interact_fill_response(ui.ctx(), &resp, quiet_canvas_fills(t));
            ui.painter()
                .rect_filled(rect.shrink(row_wash_inset()), Radius::Control.corner(), fill);
            if resp.hovered() {
                ui.output_mut(|o| o.cursor_icon = CursorIcon::PointingHand);
            }
            tip_text(ui.ctx(), &resp, if open { "Hide details" } else { "Show details" });
            if resp.clicked() && !self.expanded.remove(&id) {
                self.expanded.insert(id);
            }
        }

        let muted = t.neutral_fg_secondary();
        let ink = if status == Status::Failed { t.danger() } else { t.neutral_fg() };
        let cy = rect.center().y;
        let mut x = rect.left() + control_space::PAD_X.pts();
        if settled {
            let caret = if open { phosphor::CARET_DOWN } else { phosphor::CARET_RIGHT };
            x += paint_glyph(ui, caret, muted, x, cy) + control_space::ICON_GAP.pts();
        }
        x += paint_glyph(ui, row.icon, muted, x, cy) + control_space::ICON_GAP.pts();

        let (status_text, status_ink) = match status {
            Status::Running => ("…", pulse(ui, muted)),
            Status::Done => ("", muted),
            Status::Failed => ("failed", t.danger()),
        };
        let mut right = rect.right() - control_space::PAD_X.pts();
        if !status_text.is_empty() {
            let g = ui.painter().layout_no_wrap(
                status_text.into(),
                TypeRole::Body.font_id(),
                status_ink,
            );
            right -= g.size().x;
            ui.painter()
                .galley(pos2(right, cy - g.size().y / 2.0), g, status_ink);
            right -= Space::Sm.pts();
        }

        let verb = truncated(ui, &row.verb, TypeRole::Body.font_id(), ink, (right - x).max(24.0));
        let baseline = cy + TypeRole::Body.line_height() / 2.0;
        ui.painter()
            .galley(pos2(x, baseline - verb.size().y), verb.clone(), ink);
        x += verb.size().x;
        if let Some(path) = &row.path {
            x += Space::Xs.pts();
            let mono = FontId::new(TypeRole::Mono.size(), FontFamily::Monospace);
            let g = truncated(ui, path, mono, ink, (right - x).max(24.0));
            ui.painter().galley(pos2(x, baseline - g.size().y), g, ink);
        }
        open
    }

    /// The opened row's markdown, indented to the row's text.
    fn show_tool_detail(
        &mut self, ui: &mut Ui, col_w: f32, id: Uuid, detail: &str,
        text_areas: &mut Vec<crate::TextBufferArea>,
    ) {
        if detail.is_empty() {
            return;
        }
        ui.add(Spacer::new(Space::Xxs));
        let glyph_w = ui
            .painter()
            .layout_no_wrap(
                phosphor::CARET_RIGHT.into(),
                phosphor_ui_font_id(),
                Color32::PLACEHOLDER,
            )
            .size()
            .x;
        let indent = control_space::PAD_X.pts() + (glyph_w + control_space::ICON_GAP.pts()) * 2.0;
        let inner_w = (col_w - indent).max(24.0);
        let h = self.label(id).height(detail, inner_w);
        let (rect, _) = ui.allocate_exact_size(vec2(col_w, h), Sense::hover());
        let origin = pos2((rect.left() + indent).round(), rect.top());
        let (areas, _) = self.label(id).paint_at(ui, detail, origin, inner_w);
        text_areas.extend(areas);
    }

    /// A plate with an icon, a title, optional detail, and optionally Retry.
    /// Returns whether Retry was clicked.
    fn show_notice(&mut self, ui: &mut Ui, t: &Theme, col_w: f32, notice: Notice) -> bool {
        let pad = Space::Sm.pts();
        let lh = TypeRole::Body.line_height();
        let glyph_w = glyph_width(ui, notice.icon);
        let text_x = pad + glyph_w + control_space::ICON_GAP.pts();
        let text_w = (col_w - text_x - pad).max(40.0);
        let detail = notice.detail.as_deref().map(|d| {
            GlyphonLabel::new(d, t.neutral_fg_secondary())
                .font_size(TypeRole::Body.size())
                .line_height(lh)
                .max_width(text_w)
        });
        let detail_h = detail.as_ref().map_or(0.0, |l| l.measure(ui).y);
        let retry_h = if notice.retry { Space::Xs.pts() + control_height() } else { 0.0 };
        let h = pad + lh + detail_h + retry_h + pad;
        let (rect, _) = ui.allocate_exact_size(vec2(col_w, h), Sense::hover());
        let (fill, ink) = if notice.danger {
            (t.danger().gamma_multiply(0.12), t.danger())
        } else {
            (t.neutral_bg_secondary(), t.neutral_fg())
        };
        ui.painter().rect(
            rect,
            Radius::Control.corner(),
            fill,
            Stroke::new(STROKE_HAIRLINE, t.neutral()),
            StrokeKind::Inside,
        );
        paint_inset_bands(ui, rect, Space::Sm, Space::Sm);
        paint_glyph(ui, notice.icon, ink, rect.left() + pad, rect.top() + pad + lh / 2.0);
        let icon_gap = Rect::from_min_size(
            pos2(rect.left() + pad + glyph_w, rect.top() + pad),
            vec2(control_space::ICON_GAP.pts(), lh),
        );
        Spacer::paint_at(ui, control_space::ICON_GAP, icon_gap);
        let title =
            Rect::from_min_size(pos2(rect.left() + text_x, rect.top() + pad), vec2(text_w, lh));
        place_at(ui, title, Layout::top_down(Align::Min), |ui| {
            ui.add(
                GlyphonLabel::new(&notice.title, ink)
                    .font_size(TypeRole::Body.size())
                    .line_height(lh)
                    .max_width(text_w)
                    .text_overflow(TextOverflow::EndEllipsis),
            );
        });
        if let Some(detail) = detail {
            let slot =
                Rect::from_min_size(pos2(title.left(), title.bottom()), vec2(text_w, detail_h));
            place_at(ui, slot, Layout::top_down(Align::Min), |ui| {
                ui.add(detail);
            });
        }
        let mut clicked = false;
        if notice.retry {
            let slot = Rect::from_min_size(
                pos2(title.left(), rect.bottom() - pad - control_height()),
                vec2(text_w, control_height()),
            );
            clicked = place_at(ui, slot, Layout::left_to_right(Align::Center), |ui| {
                Button::quiet(t, "Retry")
                    .icon(phosphor::ARROW_COUNTER_CLOCKWISE)
                    .show(ui)
                    .clicked()
            })
            .0;
        }
        clicked
    }

    fn show_jump_to_latest(&mut self, ui: &mut Ui, t: &Theme, cx: f32, bottom: f32) {
        let d = control_height();
        let rect = Rect::from_center_size(pos2(cx, bottom - d / 2.0), vec2(d, d));
        let resp = ui.interact(rect, Id::new(("chat_jump", self.id)), sense_click());
        let fill = interact_fill_response(ui.ctx(), &resp, quiet_canvas_fills(t));
        ui.painter().circle(
            rect.center(),
            d / 2.0,
            fill,
            Stroke::new(STROKE_HAIRLINE, t.neutral()),
        );
        let g = ui.painter().layout_no_wrap(
            phosphor::ARROW_LINE_DOWN.into(),
            phosphor_ui_font_id(),
            t.neutral_fg(),
        );
        ui.painter()
            .galley(rect.center() - g.size() / 2.0, g, t.neutral_fg());
        if resp.hovered() {
            ui.output_mut(|o| o.cursor_icon = CursorIcon::PointingHand);
        }
        tip_text(ui.ctx(), &resp, "Jump to latest");
        if resp.clicked() {
            self.scroll_to_bottom = true;
        }
    }

    fn ask_height(&self, ui: &Ui, prompt: &str, col_w: f32) -> f32 {
        let pad = Space::Sm.pts();
        let text_w = col_w
            - pad * 2.0
            - glyph_width(ui, phosphor::LOCK_SIMPLE_OPEN)
            - control_space::ICON_GAP.pts();
        let text_h = GlyphonLabel::new(prompt, Color32::PLACEHOLDER)
            .font_size(TypeRole::Body.size())
            .line_height(TypeRole::Body.line_height())
            .max_width(text_w.max(40.0))
            .measure(ui)
            .y
            .max(TypeRole::Body.line_height());
        pad + text_h + Space::Sm.pts() + control_height() + pad
    }

    /// A tool wants permission: the prompt, Deny (esc) on the left, Allow
    /// (return) on the right.
    fn show_ask(&mut self, ui: &mut Ui, t: &Theme, rect: Rect) {
        let Some((_, prompt)) = self.pending_ask.clone() else { return };
        ui.painter().rect(
            rect,
            Radius::Surface.corner(),
            t.neutral_bg_secondary(),
            Stroke::new(STROKE_HAIRLINE, t.neutral()),
            StrokeKind::Inside,
        );
        let pad = Space::Sm.pts();
        let lh = TypeRole::Body.line_height();
        let glyph_w = paint_glyph(
            ui,
            phosphor::LOCK_SIMPLE_OPEN,
            t.accent(),
            rect.left() + pad,
            rect.top() + pad + lh / 2.0,
        );
        paint_inset_bands(ui, rect, Space::Sm, Space::Sm);
        let icon_gap = Rect::from_min_size(
            pos2(rect.left() + pad + glyph_w, rect.top() + pad),
            vec2(control_space::ICON_GAP.pts(), lh),
        );
        Spacer::paint_at(ui, control_space::ICON_GAP, icon_gap);
        let text_x = rect.left() + pad + glyph_w + control_space::ICON_GAP.pts();
        let text_rect = Rect::from_min_max(
            pos2(text_x, rect.top() + pad),
            pos2(rect.right() - pad, rect.bottom() - pad - control_height() - Space::Sm.pts()),
        );
        place_at(ui, text_rect, Layout::top_down(Align::Min), |ui| {
            ui.add(
                GlyphonLabel::new(&prompt, t.neutral_fg())
                    .font_size(TypeRole::Body.size())
                    .line_height(lh)
                    .max_width(text_rect.width()),
            );
        });

        let approve = ui
            .ctx()
            .input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter));
        let deny = ui
            .ctx()
            .input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape));
        let mut decision = if approve {
            Some(Cmd::Approve)
        } else if deny {
            Some(Cmd::Deny)
        } else {
            None
        };
        let footer = Rect::from_min_size(
            pos2(text_x, rect.bottom() - pad - control_height()),
            vec2(rect.right() - pad - text_x, control_height()),
        );
        place_at(ui, footer, Layout::left_to_right(Align::Center), |ui| {
            if Button::quiet(t, "Deny")
                .shortcut(shortcut_esc())
                .show(ui)
                .clicked()
            {
                decision = Some(Cmd::Deny);
            }
        });
        place_at(ui, footer, Layout::right_to_left(Align::Center), |ui| {
            if Button::primary(t, "Allow")
                .shortcut(shortcut_enter())
                .show(ui)
                .clicked()
            {
                decision = Some(Cmd::Approve);
            }
        });
        claim(ui, rect);
        if let Some(cmd) = decision {
            self.pending_ask = None;
            self.send_cmd(cmd);
        }
    }

    fn show_composer(
        &mut self, ui: &mut Ui, t: &Theme, rect: Rect, composer_id: Id, geom: &ComposerGeom,
    ) -> (Rect, bool, bool) {
        let (pad_x, pad_y, gap, hit) =
            (Space::Md.pts(), Space::Sm.pts(), Space::Xs.pts(), control_height());
        let focused = !self.sheet_open() && ui.ctx().memory(|m| m.has_focus(composer_id));
        let (fill, hairline) = if focused {
            (t.neutral_bg(), t.neutral_fg())
        } else {
            (t.neutral_bg_secondary(), t.neutral())
        };
        ui.painter().rect(
            rect,
            Radius::Control.corner(),
            fill,
            Stroke::new(STROKE_HAIRLINE, hairline),
            StrokeKind::Inside,
        );
        let band = rect.shrink2(vec2(pad_x, pad_y));
        paint_inset_bands(ui, rect, Space::Md, Space::Sm);
        let row = self.composer.row_height();
        let text_h = if geom.single {
            geom.measured.min(band.height()).max(row)
        } else {
            (band.height() - gap - hit).max(row)
        };
        let text_top = if geom.single { band.center().y - text_h / 2.0 } else { band.min.y };
        let text_rect = Rect::from_min_size(pos2(band.min.x, text_top), vec2(geom.text_w, text_h));
        self.composer_rect = text_rect;

        // A sheet owns the keyboard while it is open. Otherwise, with nothing
        // else focused, the composer takes it, so typing always lands.
        if self.sheet_open() {
            ui.ctx().memory_mut(|m| m.surrender_focus(composer_id));
        } else if !self.initialized || ui.memory(|m| m.focused().is_none()) {
            ui.ctx().memory_mut(|m| m.request_focus(composer_id));
            self.initialized = true;
        }
        let text_before = self.composer.renderer.buffer.current.text.clone();
        let completions_open =
            self.composer.link_completions.active || self.composer.emoji_completions.active;
        let send_requested = focused
            && !self.busy
            && ui.ctx().input_mut(|i| {
                i.consume_key(Modifiers::COMMAND, Key::Enter)
                    || (!completions_open
                        && !i.modifiers.shift
                        && i.consume_key(Modifiers::NONE, Key::Enter))
            });
        // Up in an empty composer: edit the last message, like a shell.
        let edit_last = focused
            && text_before.is_empty()
            && self.editing.is_none()
            && !self.busy
            && ui
                .ctx()
                .input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowUp));
        if edit_last {
            let me = self.account.username.clone();
            let last = self
                .transcript
                .entries
                .iter()
                .rev()
                .find(|e| e.from == me && matches!(e.body, Body::User { .. }))
                .map(|e| (e.id, e.text().to_string()));
            if let Some((id, text)) = last {
                self.start_editing(ui, id, &text);
            }
        }

        let selection_before = self.composer.renderer.buffer.current.selection;
        let input = self.composer.handle_input(ui.ctx(), composer_id);
        self.composer.show(ui, text_rect, composer_id);

        let text = self.composer.renderer.buffer.current.text.clone();
        if text.trim().is_empty() {
            let hint =
                if self.editing.is_some() { "Edit and restart from here" } else { "Message" };
            ui.painter().text(
                pos2(text_rect.left(), text_rect.top() + row / 2.0),
                Align2::LEFT_CENTER,
                hint,
                TypeRole::Body.font_id(),
                t.neutral_fg_secondary(),
            );
        }

        let controls_cy = if geom.single { band.center().y } else { band.max.y - hit / 2.0 };
        let send_rect =
            Rect::from_center_size(pos2(band.max.x - hit / 2.0, controls_cy), vec2(hit, hit));
        let model_rect = Rect::from_center_size(
            pos2(send_rect.left() - gap - geom.model_w / 2.0, controls_cy),
            vec2(geom.model_w, hit),
        );
        let folder_rect = Rect::from_center_size(
            pos2(model_rect.left() - gap - geom.folder_w / 2.0, controls_cy),
            vec2(geom.folder_w, hit),
        );
        for (left, right) in [(model_rect, send_rect), (folder_rect, model_rect)] {
            let gap_rect = Rect::from_min_max(
                pos2(left.right(), right.top()),
                pos2(right.left(), right.bottom()),
            );
            Spacer::paint_at(ui, Space::Xs, gap_rect);
        }
        if geom.single {
            let gap_rect = Rect::from_min_max(
                pos2(text_rect.right(), folder_rect.top()),
                pos2(folder_rect.left(), folder_rect.bottom()),
            );
            Spacer::paint_at(ui, Space::Xs, gap_rect);
        } else {
            let gap_rect = Rect::from_min_max(
                pos2(band.min.x, text_rect.bottom()),
                pos2(band.max.x, send_rect.top()),
            );
            Spacer::paint_at(ui, Space::Xs, gap_rect);
        }
        let menu_open = self.show_chips(ui, t, focused, model_rect, folder_rect);

        let active = self.busy || !text.trim().is_empty();
        let glyph = if self.busy { phosphor::SQUARE } else { phosphor::PAPER_PLANE_TILT };
        let resp = send_button(ui, t, send_rect, glyph, active, next_fill(t, focused));
        tip_text(ui.ctx(), &resp, if self.busy { "Stop · esc" } else { "Send · return" });

        let esc = focused
            && !completions_open
            && !menu_open
            && ui
                .ctx()
                .input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape));
        let mut sent = false;
        if self.busy && (resp.clicked() || esc) {
            self.send_cmd(Cmd::Stop);
        } else if esc && self.editing.is_some() {
            self.editing = None;
            self.composer.clear();
            self.composer_text_seq += 1;
        } else if (send_requested || resp.clicked()) && !text.trim().is_empty() && !self.busy {
            let cmd = match self.editing.take() {
                Some(id) => Cmd::Edit { id, text },
                None => Cmd::Say { text, mentions: Vec::new() },
            };
            self.send_cmd(cmd);
            self.composer.clear();
            self.composer_text_seq += 1;
            self.scroll_to_bottom = true;
            sent = true;
        }
        self.composer.show_completions(ui);

        let text_changed = input.text_updated || sent || edit_last;
        let selection_changed = self.composer.renderer.buffer.current.selection != selection_before;
        if text_changed {
            self.composer_seq += 1;
        }
        (text_rect, selection_changed, text_changed)
    }

    /// The model and folder pickers inside the composer. Returns whether a
    /// menu or sheet is open.
    fn show_chips(
        &mut self, ui: &mut Ui, t: &Theme, focused: bool, model_rect: Rect, folder_rect: Rect,
    ) -> bool {
        let fills = chip_fills(t, focused);
        let settings = self.settings();

        let model_label = self.model_label();
        let mark = self.provider_mark(ui.ctx(), TypeRole::Body.size());
        let resp = chip(ui, t, model_rect, mark, &model_label, fills);
        tip_text(ui.ctx(), &resp, format!("{} · {}", self.provider_name(), model_label));
        let favorites: Vec<(String, String, Icon)> = self
            .favorites
            .clone()
            .into_iter()
            .map(|sel| {
                let provider = sel.split_once('/').map_or("", |(p, _)| p).to_string();
                let label = self.selection_label(&sel);
                let mark = self.mark(ui.ctx(), &provider, TypeRole::Body.size());
                (sel, label, mark)
            })
            .collect();
        let choice = context_menu::show_click(&resp, t, |e| {
            for (sel, label, mark) in &favorites {
                e.item_icon(*mark, label.clone(), ModelChoice::Use(sel.clone()));
            }
            e.separator();
            e.item(phosphor::LIST, "Browse models…", ModelChoice::Browse);
            e.item(phosphor::FILE_PLUS, "Add a provider…", ModelChoice::AddProvider);
        });
        let menu_open = context_menu::is_open(&resp);
        let mut models_opened_now = false;
        match choice {
            Some(ModelChoice::Use(selection)) => self.select(selection),
            Some(ModelChoice::Browse) => {
                self.open_model_sheet();
                models_opened_now = true;
            }
            Some(ModelChoice::AddProvider) => {
                self.begin_connect(None);
            }
            None => {}
        }

        let folder_label = self.folder_label();
        let chosen = self.scope_chosen();
        let (resp, cleared) =
            scope_chip(ui, t, folder_rect, &folder_label, self.scope_open, chosen, fills);
        tip_text(ui.ctx(), &resp, format!("Reads and edits notes in {}", self.scope()));
        let mut opened_now = false;
        if cleared {
            self.close_scope_sheet();
            let mut s = settings.clone();
            s.include.clear();
            self.set_settings(s);
        } else if resp.clicked() {
            if self.scope_open {
                self.close_scope_sheet();
            } else {
                self.open_scope_sheet(ui.ctx());
                opened_now = true;
            }
        }
        self.show_scope_sheet(ui, t, opened_now);
        self.show_model_sheet(ui, t, models_opened_now);
        menu_open || self.sheet_open()
    }

    fn show_setup(&mut self, ui: &mut Ui, t: &Theme, col_w: f32) {
        ui.add(
            GlyphonLabel::new("Chat with an AI assistant", t.neutral_fg())
                .font_size(TypeRole::Heading.size())
                .line_height(TypeRole::Heading.line_height())
                .max_width(col_w)
                .text_overflow(TextOverflow::EndEllipsis),
        );
        ui.add(Spacer::new(Space::Xs));
        ui.add(
            GlyphonLabel::new(
                "Pick who runs the model. Messages, and the notes you let this chat read, go to that provider; everything else stays encrypted.",
                t.neutral_fg_secondary(),
            )
            .font_size(TypeRole::Body.size())
            .line_height(TypeRole::Body.line_height())
            .max_width(col_w),
        );
        let trouble = match &self.provider {
            _ if self.adding_provider => None,
            Some(Ok(p)) if p.needs_key => Some(format!("{} needs an API key.", p.label())),
            Some(Err(err)) => Some(err.clone()),
            _ => None,
        };
        if let Some(trouble) = trouble {
            ui.add(Spacer::new(Space::Xs));
            ui.add(
                GlyphonLabel::new(&trouble, t.neutral_fg_secondary())
                    .font_size(TypeRole::Body.size())
                    .max_width(col_w),
            );
        }
        ui.add(Spacer::new(Space::Md));
        let mut picked = None;
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(Space::Xs.pts(), Space::Xs.pts());
            for template in TEMPLATES {
                let selected = self.setup.picked.is_some_and(|p| p.name == template.name);
                let mark = self
                    .glyphs
                    .get(ui.ctx(), template.name, TypeRole::Body.size());
                if template_button(t, template.label, selected)
                    .mark(mark)
                    .show(ui)
                    .clicked()
                {
                    picked = Some(template);
                }
            }
        });
        ui.add(Spacer::new(Space::Md));
        ui.add(
            GlyphonLabel::new(
                "Or run the model yourself, on this device or another machine of yours.",
                t.neutral_fg_secondary(),
            )
            .font_size(TypeRole::Body.size())
            .line_height(TypeRole::Body.line_height())
            .max_width(col_w),
        );
        ui.add(Spacer::new(Space::Xs));
        if template_button(t, OWN.label, self.setup.own())
            .icon(phosphor::HARD_DRIVES)
            .show(ui)
            .clicked()
        {
            picked = Some(&OWN);
        }
        if let Some(template) = picked {
            self.setup.pick(template);
        }
        // Esc leaves the form when there is a working provider to go back to.
        let can_cancel = self.adding_provider && self.usable();
        let mut cancel = can_cancel
            && ui
                .ctx()
                .input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape));
        if let Some(template) = self.setup.picked {
            ui.add(Spacer::new(Space::Md));
            // A server of the user's own is its address first; a hosted
            // provider is its key first.
            let own = template.key == KeyNeed::Optional;
            if own {
                Field::new(t, &mut self.setup.base_url)
                    .hint("address, such as linux-box:11434")
                    .width(col_w)
                    .id(Id::new(("chat_setup_url", self.id)))
                    .show(ui);
                ui.add(Spacer::new(Space::Xs));
            }
            if template.key == KeyNeed::Required {
                Field::new(t, &mut self.setup.key)
                    .hint(format!("{} API key", template.label))
                    .password(true)
                    .width(col_w)
                    .id(Id::new(("chat_setup_key", self.id)))
                    .show(ui);
                ui.add(Spacer::new(Space::Xs));
            }
            Field::new(t, &mut self.setup.model)
                .hint(if own {
                    "model, or blank for the first the server lists"
                } else {
                    "model id"
                })
                .width(col_w)
                .id(Id::new(("chat_setup_model", self.id)))
                .show(ui);
            if own {
                ui.add(Spacer::new(Space::Xs));
                Field::new(t, &mut self.setup.key)
                    .hint("API key, if the server wants one")
                    .password(true)
                    .width(col_w)
                    .id(Id::new(("chat_setup_key", self.id)))
                    .show(ui);
                ui.add(Spacer::new(Space::Xs));
                ui.add(
                    GlyphonLabel::new(OWN_HELP, t.neutral_fg_secondary())
                        .font_size(TypeRole::Body.size())
                        .line_height(TypeRole::Body.line_height())
                        .max_width(col_w),
                );
            }
        }
        if self.setup.picked.is_some() || can_cancel {
            ui.add(Spacer::new(Space::Sm));
            let (footer, _) = ui.allocate_exact_size(vec2(col_w, control_height()), Sense::hover());
            if can_cancel {
                place_at(ui, footer, Layout::left_to_right(Align::Center), |ui| {
                    cancel |= Button::quiet(t, "Cancel")
                        .shortcut(shortcut_esc())
                        .show(ui)
                        .clicked();
                });
            }
            if self.setup.picked.is_some() {
                let asking = self.setup.asking.is_some();
                let label = if asking { "Connecting…" } else { "Connect" };
                let connect = place_at(ui, footer, Layout::right_to_left(Align::Center), |ui| {
                    Button::primary(t, label)
                        .enabled(!asking)
                        .show(ui)
                        .clicked()
                })
                .0;
                if connect {
                    self.connect();
                }
            }
        }
        if cancel {
            self.adding_provider = false;
            self.setup = Setup::default();
        }
        if let Some(err) = self.setup.error.clone() {
            ui.add(Spacer::new(Space::Xs));
            ui.add(
                GlyphonLabel::new(&err, t.danger())
                    .font_size(TypeRole::Body.size())
                    .max_width(col_w),
            );
        }
    }
}

struct Notice {
    icon: &'static str,
    title: String,
    detail: Option<String>,
    danger: bool,
    retry: bool,
}

impl Notice {
    fn error(text: &str, retry: bool) -> Self {
        Self {
            icon: phosphor::WARNING_CIRCLE,
            title: "Couldn't finish that turn".into(),
            detail: Some(text.to_string()),
            danger: true,
            retry,
        }
    }

    fn unfinished() -> Self {
        Self {
            icon: phosphor::INFO,
            title: "This turn didn't finish".into(),
            detail: None,
            danger: false,
            retry: true,
        }
    }
}

/// The fill one level under `on_canvas` ground: a control that reads as a
/// button by its plate alone.
fn next_fill(t: &Theme, on_canvas: bool) -> Color32 {
    if on_canvas { t.neutral_bg_secondary() } else { t.neutral_bg_tertiary() }
}

fn chip_fills(t: &Theme, on_canvas: bool) -> ControlFills {
    let rest = next_fill(t, on_canvas);
    ControlFills {
        rest,
        hover: t.wash_toward_neutral_fg(rest, FG_HOVER),
        press: t.wash_toward_neutral_fg(rest, FG_PRESS),
    }
}

/// A picker: a brand mark tinted like text, and a name (through glyphon so
/// emoji render) on a plate.
fn chip(
    ui: &mut Ui, t: &Theme, rect: Rect, mark: Icon, label: &str, fills: ControlFills,
) -> egui::Response {
    let resp = ui.interact(rect, Id::new(("chat_chip", "model")), sense_click());
    let fill = interact_fill_response(ui.ctx(), &resp, fills);
    ui.painter()
        .rect_filled(rect, Radius::Control.corner(), fill);
    if resp.hovered() {
        ui.output_mut(|o| o.cursor_icon = CursorIcon::PointingHand);
    }
    let cy = rect.center().y;
    let x = rect.left() + control_space::PAD_X.pts();
    let side = TypeRole::Body.size();
    mark.paint(ui.painter(), pos2(x + side / 2.0, cy), side, t.neutral_fg());
    let x = x + side + control_space::ICON_GAP.pts();
    let lh = TypeRole::Body.line_height();
    let right = rect.right() - control_space::PAD_X.pts();
    let slot = Rect::from_min_max(pos2(x, cy - lh / 2.0), pos2(right.max(x), cy + lh / 2.0));
    paint_file_name(ui, label, t.neutral_fg(), slot);
    resp
}

/// The folder chip, as search draws its scope: an accent folder in the icon
/// slot, the name, and an X to return to this chat's own folder. Returns the
/// chip response and whether the X was clicked.
fn scope_chip(
    ui: &mut Ui, t: &Theme, rect: Rect, name: &str, open: bool, chosen: bool, fills: ControlFills,
) -> (egui::Response, bool) {
    let pad = control_space::PAD_X.pts();
    let clear = control_icon_hit();
    let body = if chosen {
        Rect::from_min_max(rect.min, pos2(rect.right() - pad - clear, rect.max.y))
    } else {
        rect
    };
    let resp = ui.interact(body, Id::new(("chat_chip", "scope")), sense_click());
    let fill = interact_fill_response(ui.ctx(), &resp, fills);
    let fill = if open { fills.hover } else { fill };
    ui.painter()
        .rect_filled(rect, Radius::Control.corner(), fill);
    if resp.hovered() {
        ui.output_mut(|o| o.cursor_icon = CursorIcon::PointingHand);
    }
    let cy = rect.center().y;
    paint_glyph(ui, phosphor::FOLDER, t.accent(), rect.left() + pad, cy);
    let slot = Rect::from_min_max(
        pos2(rect.left() + pad + tree_metrics::ICON_SLOT, rect.top()),
        pos2((body.right() - pad).max(rect.left() + pad), rect.bottom()),
    );
    paint_file_name(ui, name, t.neutral_fg(), slot);
    let mut cleared = false;
    if chosen {
        let x_rect =
            Rect::from_center_size(pos2(rect.right() - pad - clear / 2.0, cy), vec2(clear, clear));
        let (x_resp, _) = place_at(ui, x_rect, Layout::left_to_right(Align::Center), |ui| {
            icon_button_hit(ui, t, phosphor::X, false, fill, clear)
        });
        tip_text(ui.ctx(), &x_resp, "Back to this chat's folder");
        cleared = x_resp.clicked();
    }
    (resp, cleared)
}

fn scope_chip_width(ui: &Ui, name: &str, chosen: bool) -> f32 {
    let pad = control_space::PAD_X.pts() * 2.0;
    let icon = tree_metrics::ICON_SLOT;
    let clear = if chosen { control_space::PAD_X.pts() + control_icon_hit() } else { 0.0 };
    let inner_max = (CHIP_MAX_W - pad - icon - clear).max(1.0);
    let inner = file_name::measure_sized(
        ui,
        name,
        file_name::body_font_size(),
        file_name::body_line_height(),
        inner_max,
    );
    (pad + icon + inner + clear).clamp(control_height(), CHIP_MAX_W)
}

fn chip_width(ui: &Ui, lead_w: f32, label: &str) -> f32 {
    let name_w = measure_file_name(ui, label).min(CHIP_MAX_W);
    (control_space::PAD_X.pts() * 2.0 + lead_w + control_space::ICON_GAP.pts() + name_w)
        .max(control_height())
}

/// What the form says under a server of the user's own.
const OWN_HELP: &str = "Any OpenAI-compatible server works. Ollama answers on port 11434, LM Studio on 1234, llama.cpp on 8080. For another machine, put its name or address in place of localhost.";

/// A provider in the setup form: solid when it is the one picked.
fn template_button<'a>(t: &'a Theme, label: &str, selected: bool) -> Button<'a> {
    if selected { Button::primary(t, label) } else { Button::secondary(t, label) }
}

/// Where a chat's messages go, as the empty chat says it.
fn privacy_line(provider: Option<&Provider>) -> String {
    match provider {
        Some(p) if p.place() == Place::ThisDevice => "Messages stay on this device.".into(),
        Some(p) => format!("Messages you send go to {}.", p.label()),
        None => "Messages you send go to the provider.".into(),
    }
}

/// The primary action: accent plate with canvas ink while there is something
/// to send or stop; the plain next level otherwise.
fn send_button(
    ui: &mut Ui, t: &Theme, rect: Rect, glyph: &'static str, active: bool, idle_fill: Color32,
) -> egui::Response {
    let resp = ui.interact(rect, Id::new(("chat_send", glyph)), sense_click());
    let (fill, ink) = if active {
        let fills = ControlFills {
            rest: t.accent(),
            hover: t.accent().lerp_to_gamma(t.neutral_bg(), 0.16),
            press: t.accent().lerp_to_gamma(t.neutral_bg(), 0.24),
        };
        (interact_fill_response(ui.ctx(), &resp, fills), t.neutral_bg())
    } else {
        (idle_fill, t.neutral_fg_secondary())
    };
    ui.painter()
        .rect_filled(rect, Radius::Control.corner(), fill);
    if active && resp.hovered() {
        ui.output_mut(|o| o.cursor_icon = CursorIcon::PointingHand);
    }
    let g = ui
        .painter()
        .layout_no_wrap(glyph.into(), phosphor_ui_font_id(), ink);
    ui.painter().galley(rect.center() - g.size() / 2.0, g, ink);
    resp
}

/// Where a message's action sits: just left of the message, centred on its
/// first line of height `row`.
fn action_rect(message_left: f32, first_line_top: f32, row: f32) -> Rect {
    let hit = control_icon_hit();
    Rect::from_center_size(
        pos2(message_left - Space::Xs.pts() - hit / 2.0, first_line_top + row / 2.0),
        Vec2::splat(hit),
    )
}

/// An action in its [`action_rect`], with the gap to its message.
fn action_button(
    ui: &mut Ui, t: &Theme, slot: Rect, message_left: f32, glyph: &'static str, active: bool,
) -> egui::Response {
    let gap = Rect::from_min_max(slot.right_top(), pos2(message_left, slot.bottom()));
    Spacer::paint_at(ui, Space::Xs, gap);
    place_at(ui, slot, Layout::left_to_right(Align::Center), |ui| {
        icon_button_hit(ui, t, glyph, active, t.neutral_bg(), slot.width())
    })
    .0
}

/// A folder's name as the chip shows it; the root is Home, as in crumbs.
fn folder_name(path: &str) -> String {
    path.trim_end_matches('/')
        .rsplit('/')
        .next()
        .filter(|n| !n.is_empty())
        .unwrap_or("Home")
        .to_string()
}

/// F2-visible pad bands just inside `outer`: top and bottom full width,
/// sides between them.
fn paint_inset_bands(ui: &Ui, outer: Rect, x: Space, y: Space) {
    let (px, py) = (x.pts(), y.pts());
    Spacer::paint_at(ui, y, Rect::from_min_size(outer.min, vec2(outer.width(), py)));
    Spacer::paint_at(ui, y, Rect::from_min_max(pos2(outer.min.x, outer.max.y - py), outer.max));
    let mid = Rect::from_min_max(
        pos2(outer.min.x, outer.min.y + py),
        pos2(outer.max.x, outer.max.y - py),
    );
    Spacer::paint_at(ui, x, Rect::from_min_size(mid.min, vec2(px, mid.height())));
    Spacer::paint_at(ui, x, Rect::from_min_max(pos2(mid.max.x - px, mid.min.y), mid.max));
}

fn caption_label<'a>(t: &Theme, col_w: f32, text: &'a str) -> GlyphonLabel<'a> {
    GlyphonLabel::new(text, t.neutral_fg_secondary())
        .font_size(CAPTION_SIZE)
        .line_height(CAPTION_LH)
        .max_width(col_w)
        .text_overflow(TextOverflow::EndEllipsis)
}

fn caption(ui: &mut Ui, t: &Theme, col_w: f32, text: &str) {
    ui.add(caption_label(t, col_w, text));
}

fn caption_centered(ui: &mut Ui, t: &Theme, col_w: f32, text: &str) {
    let (rect, _) = ui.allocate_exact_size(vec2(col_w, CAPTION_LH), Sense::hover());
    place_at(ui, rect, Layout::top_down(Align::Center), |ui| {
        ui.add(caption_label(t, col_w, text));
    });
}

fn caption_pulsing(ui: &mut Ui, t: &Theme, col_w: f32, text: &str) {
    let (rect, _) = ui.allocate_exact_size(vec2(col_w, CAPTION_LH), Sense::hover());
    let ink = pulse(ui, t.neutral_fg_secondary());
    place_at(ui, rect, Layout::top_down(Align::Min), |ui| {
        ui.add(
            GlyphonLabel::new(text, ink)
                .font_size(CAPTION_SIZE)
                .line_height(CAPTION_LH)
                .max_width(col_w),
        );
    });
}

/// Breathes between 40% and full over 1.4 s; steady when motion is off.
fn pulse(ui: &Ui, base: Color32) -> Color32 {
    if ui.ctx().style().animation_time < 0.01 {
        return base;
    }
    ui.ctx().request_repaint();
    let s = ((ui.input(|i| i.time) * std::f64::consts::TAU / 1.4).sin() * 0.5 + 0.5) as f32;
    base.gamma_multiply(0.4 + 0.6 * s)
}

fn glyph_width(ui: &Ui, glyph: &str) -> f32 {
    ui.painter()
        .layout_no_wrap(glyph.into(), phosphor_ui_font_id(), Color32::PLACEHOLDER)
        .size()
        .x
}

fn paint_glyph(ui: &Ui, glyph: &str, color: Color32, x: f32, cy: f32) -> f32 {
    let g = ui
        .painter()
        .layout_no_wrap(glyph.into(), phosphor_ui_font_id(), color);
    let w = g.size().x;
    ui.painter()
        .galley(pos2(x, cy - g.size().y / 2.0), g, color);
    w
}

fn truncated(
    ui: &Ui, text: &str, font: FontId, color: Color32, max_w: f32,
) -> std::sync::Arc<egui::Galley> {
    let mut job = LayoutJob::simple_singleline(text.to_string(), font, color);
    job.wrap = TextWrapping::truncate_at_width(max_w);
    ui.painter().layout_job(job)
}

fn local_day(ts: i64) -> Option<chrono::NaiveDate> {
    chrono::DateTime::from_timestamp_millis(ts)
        .map(|d| d.with_timezone(&chrono::Local).date_naive())
}

fn day_label(day: chrono::NaiveDate) -> String {
    use chrono::Datelike;
    let today = chrono::Local::now().date_naive();
    if day == today {
        "Today".into()
    } else if day.year() == today.year() {
        day.format("%A, %B %-d").to_string()
    } else {
        day.format("%B %-d, %Y").to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The line follows the address: a host that only starts with
    /// "localhost" is somewhere else.
    #[test]
    fn the_privacy_line_says_where_messages_go() {
        let at = |name: &str, file: Value| Provider::parse(name, "m", file.to_string().as_bytes());
        let line = |name: &str, file: Value| privacy_line(Some(&at(name, file).unwrap()));
        assert_eq!(
            line("localhost", serde_json::json!({ "base_url": "http://localhost:11434/v1" })),
            "Messages stay on this device."
        );
        assert_eq!(
            line(
                "linux-box",
                serde_json::json!({ "display_name": "linux-box", "base_url": "http://linux-box:11434/v1" })
            ),
            "Messages you send go to linux-box."
        );
        assert_eq!(
            line("sneaky", serde_json::json!({ "base_url": "https://localhost.example.com/v1" })),
            "Messages you send go to Sneaky."
        );
    }
}
