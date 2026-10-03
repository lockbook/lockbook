//! The chat tab's face: a transcript column, the live run under it, and a
//! composer that holds its own controls. Everything is placed by hand on the
//! design system's plates, rows, and buttons.

use egui::os::OperatingSystem;
use egui::scroll_area::ScrollSource;
use egui::{
    Align, Align2, Color32, CornerRadius, CursorIcon, Id, Key, Layout, Modifiers, Rect, ScrollArea,
    Sense, Stroke, StrokeKind, Ui, UiBuilder, Vec2, pos2, vec2,
};
use lb_chat::{Cmd, Kind as ProviderKind, Place, Provider, prettify};
use lb_rs::Uuid;
use lb_rs::model::chat::{Body, Entry};
use serde_json::Value;
use unicode_segmentation::UnicodeSegmentation as _;

use super::diff::Change;
use super::setup::{Key as KeyNeed, OWN, TEMPLATES};
use super::{Chat, IN_FLIGHT, Setup, rows};
use crate::file_cache::FilesExt;
use crate::resolvers::embed::EmbedResolver as _;
use crate::resolvers::image_embed::ImageEmbedResolver;
use crate::style::chrome::{
    display_file_name, file_row_icon, is_touch, shortcut_enter, shortcut_esc,
};
use crate::style::file_name;
use crate::style::interact::{ControlFills, interact_fill_response, quiet_canvas_fills};
use crate::style::layout::paint_control_pads;
use crate::style::space::control as control_space;
use crate::style::{
    Button, FG_HOVER, FG_PRESS, Field, Icon, Radius, STROKE_HAIRLINE, Space, Spacer, Theme,
    ThemeExt, TypeRole, claim, context_menu, control_height, control_icon_hit, icon_button_hit,
    measure_file_name, paint_file_name, phosphor, phosphor_ui_font_id, place_at, sense_click,
    tip_text, with_overlay_scroll,
};
use crate::style::{FileRow, parent_crumbs};
use crate::style::{
    SheetFooterOpts, expand_ancestors_of, folder_tree_scroll_key, show_folder_sheet, tree_metrics,
};
use crate::tab::ExtendedOutput as _;
use crate::tab::markdown_editor::input::{Event as Edit, Location, Region};
use crate::voice;
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
/// How strongly a diff tints the words that went and the words that came:
/// enough to find at a glance, 1.3 to 1.7 against the page in either mode.
const DIFF_WASH: f32 = 0.32;
/// The longest piece of a diff that still reads well on one line with the rest.
const DIFF_INLINE: usize = 40;
/// After a scroll to the newest line is asked for, wheel motion is dropped
/// until the wheel has rested this long: momentum still in flight would
/// cancel the scroll.
const WHEEL_REST_SECS: f64 = 0.15;
/// How far above the composer the transcript fades out. The transcript ends
/// on as much blank, so at the end of a chat nothing sits under the fade.
const FADE: Space = Space::Lg;
/// The least width of text that shares a row with the composer's controls.
const MIN_BESIDE: f32 = 140.0;
/// The tallest a picture is drawn in a card.
const PICTURE_H: f32 = 360.0;
/// The stop square's side: the weight of a glyph at body size.
const STOP_SIDE: f32 = 10.0;

#[derive(Clone)]
enum ModelChoice {
    /// A pinned `provider/model` selection.
    Use(String),
    /// How hard the model thinks; none is its provider's default.
    Effort(Option<String>),
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

/// What a row's context menu can do.
#[derive(Clone)]
enum RowAction {
    Open(Uuid),
    /// Point the file tree at a folder.
    Show(Uuid),
    Copy(String),
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
    /// One button stands for the folder and model chips, and the text
    /// grows beside the controls.
    compact: bool,
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
        self.view = full;
        let col_w = (full.width() - Space::Lg.pts() * 2.0).clamp(160.0, COLUMN_W);
        let col_x = (full.center().x - col_w / 2.0).round();
        let ready = self.is_ready();
        let composer_id = Id::new(("chat_composer", self.id));

        let (pad_x, pad_y, gap, hit) =
            (Space::Md.pts(), Space::Sm.pts(), Space::Xs.pts(), control_height());
        let model_w = chip_width(ui, TypeRole::Body.size(), &self.model_chip_label());
        let folder_w = scope_chip_width(ui, &self.folder_label(), self.scope_chosen());
        let wrap_w = (col_w - pad_x * 2.0).max(1.0);
        // Where the chips would leave too little room to type beside them,
        // each is its mark alone and the text keeps the row.
        let call_w = if self.calls() { hit + gap } else { 0.0 };
        let compact = wrap_w - gap - call_w - (folder_w + gap + model_w + gap + hit) < MIN_BESIDE;
        let (folder_w, model_w) = if compact { (hit, hit) } else { (folder_w, model_w) };
        let trailing = call_w + folder_w + gap + model_w + gap + hit;
        let inner_w = (wrap_w - gap - trailing).max(1.0);
        let row = self.composer.row_height();
        let wide_h = self.composer.measure_height(wrap_w);
        let measured = if compact || wide_h <= row + 1.0 {
            self.composer.measure_height(inner_w)
        } else {
            wide_h
        };
        let single = compact || measured <= row + 1.0;
        let geom = ComposerGeom {
            single,
            compact,
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
        let lg = Space::Lg.pts();
        let bottom_h = if ready { composer_h + lg } else { 0.0 };

        let transcript_rect =
            Rect::from_min_max(full.min, pos2(full.max.x, (full.max.y - bottom_h).round()));
        let mut command = None;
        let scroll_id = Id::new(("chat_scroll", self.id));
        let mut at_bottom = true;
        let now = ui.input(|i| i.time);
        if std::mem::take(&mut self.scroll_to_bottom) {
            self.to_latest = Some(now);
        }
        self.to_latest = self
            .to_latest
            .filter(|since| now - since <= WHEEL_REST_SECS);
        if self.to_latest.is_some() && ui.rect_contains_pointer(transcript_rect) {
            let coasting = ui
                .ctx()
                .input_mut(|i| std::mem::take(&mut i.smooth_scroll_delta.y) != 0.0);
            if coasting {
                self.to_latest = Some(now);
            }
        }
        ui.scope_builder(UiBuilder::new().max_rect(transcript_rect), |ui| {
            ui.set_clip_rect(transcript_rect.intersect(ui.clip_rect()));
            at_bottom = with_overlay_scroll(ui, scroll_id, |ui| {
                let mut text_areas = Vec::new();
                // A mouse drag selects text; only a finger drags the page.
                let touch =
                    matches!(ui.ctx().os(), OperatingSystem::Android | OperatingSystem::IOS);
                // A place to go back to is taken for a frame without the
                // pull to the end, which would win.
                let place_to = self.place_to.take();
                let mut area = ScrollArea::vertical();
                if let Some(to) = place_to {
                    area = area.vertical_scroll_offset(to);
                }
                let out = area
                    .id_salt(scroll_id)
                    .stick_to_bottom(place_to.is_none())
                    .auto_shrink([false, false])
                    .scroll_source(ScrollSource { drag: touch, ..ScrollSource::ALL })
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing = Vec2::ZERO;
                        ui.horizontal_top(|ui| {
                            ui.add_space((col_x - ui.max_rect().left()).max(0.0));
                            ui.vertical(|ui| {
                                ui.set_width(col_w);
                                ui.spacing_mut().item_spacing = Vec2::ZERO;
                                if ready {
                                    ui.add(Spacer::new(FADE));
                                    self.show_transcript(
                                        ui,
                                        &t,
                                        col_w,
                                        transcript_rect.height(),
                                        &mut text_areas,
                                        &mut command,
                                    );
                                    ui.add(Spacer::new(FADE));
                                } else {
                                    ui.add(Spacer::new(Space::Xl));
                                    self.show_setup(ui, &t, col_w);
                                    ui.add(Spacer::new(Space::Xl));
                                }
                            });
                        });
                        if self.to_latest.is_some() {
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
                if ready {
                    let (bg, clear) = (t.neutral_bg(), Color32::TRANSPARENT);
                    let top = transcript_rect.with_max_y(transcript_rect.top() + FADE.pts());
                    fade(ui.painter(), top, bg, clear);
                    let bottom = transcript_rect.with_min_y(transcript_rect.bottom() - FADE.pts());
                    fade(ui.painter(), bottom, clear, bg);
                }
                let at_bottom =
                    out.state.offset.y + out.inner_rect.height() >= out.content_size.y - 1.0;
                let top = out.inner_rect.top();
                self.place_to = self.keep_place(out.state.offset.y, top, at_bottom);
                if self.place_to.is_some() {
                    ui.ctx().request_repaint();
                }
                (at_bottom, out.state.offset.y, out.id)
            });
        });
        if let Some(cmd) = command {
            self.send_cmd(cmd);
        }
        if !ready {
            // Nothing to type into: a frame that takes no touch and draws no caret.
            return (Rect::from_min_size(full.min, Vec2::ZERO), false, false);
        }
        if !at_bottom {
            let above_fade = transcript_rect.bottom() - FADE.pts();
            self.show_jump_to_latest(ui, &t, full.center().x, above_fade);
        }

        let y = full.max.y - lg;
        let composer_rect =
            Rect::from_min_max(pos2(col_x, (y - composer_h).round()), pos2(col_x + col_w, y));
        let under =
            Rect::from_min_max(composer_rect.left_bottom(), pos2(col_x + col_w, full.max.y));
        Spacer::paint_at(ui, Space::Lg, under);
        let out = self.show_composer(ui, &t, composer_rect, composer_id, &geom);
        claim(ui, Rect::from_min_max(pos2(full.min.x, transcript_rect.max.y), full.max));
        out
    }

    fn model_label(&self) -> String {
        if let Some(m) = self.listed_model() {
            return m.label();
        }
        match &self.provider {
            // The device's one model goes by its provider's name.
            Some(Ok(p)) if p.kind == ProviderKind::Apple => p.label(),
            Some(Ok(p)) if !p.model.is_empty() => prettify(&p.model),
            _ => "model".into(),
        }
    }

    /// The model as its chip names it, with the effort when one is chosen.
    fn model_chip_label(&self) -> String {
        match self.effort() {
            Some(effort) => format!("{} · {}", self.model_label(), effort_name(effort)),
            None => self.model_label(),
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

    /// Where a phone's sheets go: a hair in from the view's sides and top,
    /// down to where the composer ends.
    pub(super) fn sheet_fill(&self) -> Rect {
        let edge = Space::Sm.pts();
        Rect::from_min_max(
            self.view.min + vec2(edge, edge),
            pos2(self.view.max.x - edge, self.view.max.y - Space::Lg.pts()),
        )
    }

    /// The host can hold a call and the chat's provider speaks; a call under
    /// way keeps its hang-up either way.
    fn calls(&self) -> bool {
        voice::offered()
            && (self.voice || matches!(&self.provider, Some(Ok(p)) if p.speaks().is_some()))
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
        ctx.set_virtual_keyboard_shown(false);
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
        let fill = is_touch(ui.ctx()).then(|| self.sheet_fill());
        let files = self.files.read().unwrap();
        let mut out = show_folder_sheet(
            ui.ctx(),
            t,
            &*files,
            &mut self.scope_expanded,
            dest,
            &[],
            "chat_scope",
            fill,
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
        // The last message ended in its action strip, which is most of a gap.
        let mut after_strip = false;
        self.spans.clear();
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
            // A reply that showed its thinking opens on a row for it, which
            // sits among tool rows as one of them.
            let (lead, trail) = match &entry.body {
                Body::Assistant { text, thinking, .. } if !thinking.is_empty() => {
                    (Kind::Tool, if text.is_empty() { Kind::Tool } else { kind })
                }
                _ => (kind, kind),
            };
            if let Some(prev) = prev_kind {
                ui.add(Spacer::new(if after_strip { Space::Xs } else { gap(prev, lead) }));
            }
            let mine = entry.from == me;
            let row_top = ui.cursor().top();
            after_strip = match &entry.body {
                Body::User { text, .. } => self.show_user(ui, t, col_w, entry, text),
                Body::Assistant { text, thinking, interrupted, .. } => {
                    if !mine {
                        caption(ui, t, col_w, &format!("{}'s assistant", entry.from));
                    }
                    if !thinking.is_empty() {
                        let thought = Some((thinking.as_str(), true));
                        let none = Value::Null;
                        self.tool_card(
                            ui, t, col_w, entry.id, "thought", &none, thought, text_areas,
                        );
                        if !text.is_empty() {
                            ui.add(Spacer::new(gap(lead, kind)));
                        }
                    }
                    let strip = self.show_assistant(ui, t, col_w, entry.id, text);
                    // The newest reply cut short gets the notice instead.
                    let noticed = mine && Some(entry.id) == last && !self.busy && !self.voice;
                    if *interrupted && !noticed {
                        caption(ui, t, col_w, "stopped");
                    }
                    strip && (!*interrupted || noticed)
                }
                Body::Tool { name, args, result, ok, .. } => {
                    let outcome = Some((result.as_str(), *ok));
                    self.tool_card(ui, t, col_w, entry.id, name, args, outcome, text_areas);
                    false
                }
                Body::Error { text } => {
                    if !mine {
                        caption(ui, t, col_w, &format!("{}'s assistant", entry.from));
                    }
                    let retry = mine && Some(entry.id) == last && !self.busy;
                    if self.show_notice(ui, t, col_w, Notice::error(text, retry)) {
                        *command = Some(Cmd::Regenerate);
                    }
                    false
                }
                Body::Other(_) => false,
            };
            self.spans.push((entry.id, row_top, ui.cursor().top()));
            // Rows stack: the cursor ends under everything laid out so far.
            debug_assert!(
                ui.cursor().top() >= ui.min_rect().bottom() - 0.5,
                "a row left the cursor {} above what it drew",
                ui.min_rect().bottom() - ui.cursor().top()
            );
            prev_kind = Some(trail);
        }

        if !self.thinking.is_empty() {
            ui.add(Spacer::new(gap(prev_kind.unwrap_or(Kind::User), Kind::Tool)));
            let thinking = self.thinking.clone();
            let so_far = Some((thinking.as_str(), true));
            self.tool_card(ui, t, col_w, IN_FLIGHT, "thinking", &Value::Null, so_far, text_areas);
            prev_kind = Some(Kind::Tool);
        }
        if !self.hearing.is_empty() {
            ui.add(Spacer::new(if after_strip { Space::Xs } else { Space::Md }));
            let hearing = self.hearing.clone();
            self.show_hearing(ui, t, col_w, &hearing);
            prev_kind = Some(Kind::User);
        }
        if let Some(call) = self.running_tool.clone() {
            ui.add(Spacer::new(gap(prev_kind.unwrap_or(Kind::User), Kind::Tool)));
            let (name, args) = (&call.name, &call.args);
            self.tool_card(ui, t, col_w, Uuid::nil(), name, args, None, text_areas);
        } else if !self.streaming.is_empty() {
            ui.add(Spacer::new(gap(prev_kind.unwrap_or(Kind::User), Kind::Assistant)));
            let streaming = self.streaming.clone();
            text_areas.extend(self.streaming_label.show(ui, &streaming, col_w));
        } else if self.busy && self.thinking.is_empty() {
            ui.add(Spacer::new(Space::Md));
            caption_pulsing(ui, t, col_w, "Thinking…");
        } else if !self.busy && !self.voice && unfinished(&entries, &me) {
            ui.add(Spacer::new(Space::Md));
            if self.show_notice(ui, t, col_w, Notice::unfinished()) {
                *command = Some(Cmd::Resume);
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
        let slot_h = (transcript_h - Space::Lg.pts() - FADE.pts()).max(block_h);
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

    /// Returns whether the message ended in its action strip.
    fn show_user(&mut self, ui: &mut Ui, t: &Theme, col_w: f32, entry: &Entry, text: &str) -> bool {
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
        let bubble_w = (natural + pad * 2.0 + 2.0).min(max_w);
        let inner_w = bubble_w - pad * 2.0;
        let text_h = self.reader(entry.id, text).measure_height(inner_w);
        let h = text_h + pad * 2.0;
        let (row, _) = ui.allocate_exact_size(vec2(col_w, h), Sense::hover());
        let bubble = if mine {
            Rect::from_min_max(pos2(row.right() - bubble_w, row.top()), row.max)
        } else {
            Rect::from_min_size(row.min, vec2(bubble_w, h))
        };
        ui.painter()
            .rect_filled(bubble, Radius::Surface.corner(), t.neutral_bg_secondary());
        paint_inset_bands(ui, bubble, Space::Sm, Space::Sm);
        let inner = Rect::from_min_size(bubble.min + vec2(pad, pad), vec2(inner_w, text_h));
        self.show_reader(ui, entry.id, text, inner);

        if !mine {
            return false;
        }
        let strip = action_strip(ui, col_w);
        if self.shows_actions(ui, entry.id, row.union(strip)) && !self.busy {
            // The glyph's right edge on the bubble's; retry to its left.
            let hit = control_icon_hit();
            let edit = strip_slot(strip, bubble.right() - hit + glyph_inset());
            let resp = action_button(ui, t, edit, phosphor::PENCIL, false);
            tip_text(ui.ctx(), &resp, "Edit and restart from here");
            if resp.clicked() {
                self.start_editing(ui, entry.id, text);
            }
            let retry = strip_slot(strip, edit.left() - Space::Xs.pts() - hit);
            let resp = action_button(ui, t, retry, phosphor::ARROW_COUNTER_CLOCKWISE, false);
            tip_text(ui.ctx(), &resp, "Restart from here");
            if resp.clicked() {
                let mentions = match &entry.body {
                    Body::User { mentions, .. } => mentions.clone(),
                    _ => Vec::new(),
                };
                self.send_cmd(Cmd::Edit { id: entry.id, text: text.to_string(), mentions });
                self.scroll_to_bottom = true;
            }
        }
        true
    }

    /// What the user is saying in a call, as a bubble of their own still
    /// filling: secondary ink, since it is not settled.
    fn show_hearing(&mut self, ui: &mut Ui, t: &Theme, col_w: f32, text: &str) {
        let pad = Space::Sm.pts();
        let max_w = col_w * BUBBLE_FRACTION;
        let md = self.composer.row_height();
        let label = |ink| {
            GlyphonLabel::new(text, ink)
                .font_size(md)
                .line_height(md)
                .max_width(max_w - pad * 2.0)
        };
        let size = label(Color32::PLACEHOLDER).measure(ui);
        let bubble_w = (size.x + pad * 2.0 + 2.0).min(max_w);
        let h = size.y + pad * 2.0;
        let (row, _) = ui.allocate_exact_size(vec2(col_w, h), Sense::hover());
        let bubble = Rect::from_min_max(pos2(row.right() - bubble_w, row.top()), row.max);
        ui.painter()
            .rect_filled(bubble, Radius::Surface.corner(), t.neutral_bg_secondary());
        paint_inset_bands(ui, bubble, Space::Sm, Space::Sm);
        place_at(ui, bubble.shrink(pad), Layout::top_down(Align::Min), |ui| {
            ui.add(label(t.neutral_fg_secondary()));
        });
    }

    /// Whether the message `id`, drawn in `rect`, shows its actions: under
    /// the pointer, or where there is none, once tapped and until a tap
    /// lands elsewhere.
    fn shows_actions(&mut self, ui: &Ui, id: Uuid, rect: Rect) -> bool {
        let touch = matches!(ui.ctx().os(), OperatingSystem::Android | OperatingSystem::IOS);
        if !touch {
            return ui.rect_contains_pointer(rect);
        }
        let tap = ui.input(|i| {
            i.pointer
                .any_click()
                .then(|| i.pointer.interact_pos())
                .flatten()
        });
        match tap.map(|at| rect.contains(at) && ui.clip_rect().contains(at)) {
            Some(true) => self.tapped = Some(id),
            Some(false) if self.tapped == Some(id) => self.tapped = None,
            _ => {}
        }
        self.tapped == Some(id)
    }

    fn start_editing(&mut self, ui: &Ui, id: Uuid, text: &str) {
        self.editing = Some(id);
        self.composer.set_text(text);
        self.composer_text_seq += 1;
        ui.ctx()
            .memory_mut(|m| m.request_focus(Id::new(("chat_composer", self.id))));
        ui.ctx().set_virtual_keyboard_shown(true);
    }

    /// A settled message's text in `rect`. It selects with the mouse and
    /// copies; one message holds a selection at a time.
    fn show_reader(&mut self, ui: &mut Ui, id: Uuid, text: &str, rect: Rect) {
        let at = text_id(id);
        let reader = self.reader(id, text);
        if ui.memory(|m| m.has_focus(at)) || !reader.event.internal_events.is_empty() {
            reader.handle_input(ui.ctx(), at);
        }
        // A child of its own: the text's layout leaves the column's cursor
        // alone, and its widgets are not confused with another message's.
        let mut child = ui.new_child(UiBuilder::new().max_rect(rect).id_salt(at));
        reader.show(&mut child, rect, at);
        // The selection goes when the keyboard does.
        let (start, end) = reader.renderer.buffer.current.selection;
        if start != end && !ui.memory(|m| m.has_focus(at)) {
            let region = Region::Location(Location::Grapheme(end));
            reader.event.internal_events.push(Edit::Select { region });
        }
    }

    /// Whether a message holds the keyboard for its selection.
    fn reading(&self, ui: &Ui) -> bool {
        ui.memory(|m| m.focused())
            .is_some_and(|focused| self.readers.keys().any(|id| text_id(*id) == focused))
    }

    /// Returns whether the reply ended in its action strip.
    fn show_assistant(&mut self, ui: &mut Ui, t: &Theme, col_w: f32, id: Uuid, text: &str) -> bool {
        if text.is_empty() {
            return false;
        }
        let h = self.reader(id, text).measure_height(col_w);
        let (rect, _) = ui.allocate_exact_size(vec2(col_w, h), Sense::hover());
        self.show_reader(ui, id, text, rect);

        // Copy shows while the pointer is over the reply, and a check for a
        // moment after it was used.
        let strip = action_strip(ui, col_w);
        // The glyph's left edge on the text's.
        let btn = strip_slot(strip, strip.left() - glyph_inset());
        let fid = Id::new(("chat_copied", id));
        let now = ui.input(|i| i.time);
        let copied = ui
            .ctx()
            .data(|d| d.get_temp::<f64>(fid))
            .is_some_and(|until| now < until);
        if copied {
            ui.ctx().request_repaint();
        }
        if !copied && !self.shows_actions(ui, id, rect.union(strip)) {
            return true;
        }
        let glyph = if copied { phosphor::CHECK } else { phosphor::COPY };
        let resp = action_button(ui, t, btn, glyph, copied);
        tip_text(ui.ctx(), &resp, "Copy");
        if resp.clicked() {
            ui.ctx().copy_text(text.to_string());
            ui.ctx().data_mut(|d| d.insert_temp(fid, now + COPIED_SECS));
        }
        true
    }

    /// A tool call as a card. Its bar holds the icon, the statement, and how
    /// the call went; `outcome` is its result and whether it worked, once
    /// it has one. A settled card opens on a click, and then the bar and
    /// what the call returned share one border.
    #[allow(clippy::too_many_arguments)]
    fn tool_card(
        &mut self, ui: &mut Ui, t: &Theme, col_w: f32, id: Uuid, name: &str, args: &Value,
        outcome: Option<(&str, bool)>, text_areas: &mut Vec<crate::TextBufferArea>,
    ) {
        let row = rows::row(name, args);
        let open = outcome.is_some() && self.expanded.contains(&id);
        let pad = control_space::PAD_X.pts();
        let lead = tool_text_inset(ui);
        let lh = TypeRole::Body.line_height();
        let status_w = glyph_width(ui, phosphor::CHECK);
        let spans = fit_statement(ui, row.words, col_w - lead - Space::Sm.pts() - status_w - pad);

        let (bar, _) = ui.allocate_exact_size(vec2(col_w, control_height()), Sense::hover());
        let fills = chip_fills(t, true);
        let mut fill = fills.rest;
        if outcome.is_some() {
            let resp = ui.interact(bar, Id::new(("chat_tool", id)), sense_click());
            fill = interact_fill_response(ui.ctx(), &resp, fills);
            if resp.hovered() {
                ui.output_mut(|o| o.cursor_icon = CursorIcon::PointingHand);
            }
            tip_text(ui.ctx(), &resp, if open { "Hide details" } else { "Show details" });
            if resp.clicked() && !self.expanded.remove(&id) {
                self.expanded.insert(id);
            }
            self.row_menu(ui, t, &resp, name, args, outcome.map_or("", |(result, _)| result));
        }
        if !open {
            self.bodies.remove(&id);
        }
        let radius = Radius::Control.corner();
        let corners = if open { CornerRadius { sw: 0, se: 0, ..radius } } else { radius };
        ui.painter().rect_filled(bar, corners, fill);
        paint_control_pads(ui, bar, control_space::PAD_X, control_space::PAD_Y);

        let cy = bar.center().y;
        let icon_w = paint_glyph(ui, row.icon, t.neutral_fg(), bar.left() + pad, cy);
        let icon_gap = Rect::from_min_max(
            pos2(bar.left() + pad + icon_w, cy - lh / 2.0),
            pos2(bar.left() + lead, cy + lh / 2.0),
        );
        Spacer::paint_at(ui, control_space::ICON_GAP, icon_gap);
        let text = Rect::from_min_max(
            pos2(bar.left() + lead, cy - lh / 2.0),
            pos2(bar.right() - pad - status_w, cy + lh / 2.0),
        );
        place_at(ui, text, Layout::left_to_right(Align::Center), |ui| {
            ui.add(statement(span_refs(&spans), t.neutral_fg()));
        });
        // A thought still arriving opens like a settled card.
        let arriving = name == "thinking";
        let status = match outcome {
            None => Status::Running,
            Some(_) if arriving => Status::Running,
            Some((_, true)) => Status::Done,
            Some((_, false)) => Status::Failed,
        };
        paint_status(ui, t, status, bar.right() - pad, cy);

        let Some((result, ok)) = outcome.filter(|_| open) else { return };
        let query = args
            .get("query")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let parts = match self.bodies.remove(&id) {
            Some(parts) => parts,
            None => self.card_body(name, args, result, ok),
        };
        // A quoted note's links are relative to where the note is.
        let quoted = args.get("path").and_then(Value::as_str);
        let quoted = quoted.and_then(|path| self.files.read().unwrap().by_path(path).map(|f| f.id));
        let from = quoted.unwrap_or(self.id);
        // File rows are padded already; text is not, above or below.
        let mut after_files = false;
        for part in &parts {
            let files = matches!(part, rows::Part::Files(_));
            if !files {
                ui.add(Spacer::new(if after_files { Space::Xs } else { Space::Sm }));
            }
            self.tool_part(ui, t, col_w, (id, from), part, query, text_areas);
            after_files = files;
        }
        if !after_files {
            ui.add(Spacer::new(Space::Sm));
        }
        if !arriving {
            self.bodies.insert(id, parts);
        }
        let card = Rect::from_min_max(bar.min, pos2(bar.right(), ui.cursor().top()));
        ui.painter().rect_stroke(
            card,
            radius,
            Stroke::new(STROKE_HAIRLINE, t.neutral()),
            StrokeKind::Inside,
        );
    }

    /// A row's context menu: go to what the call was about, or copy it.
    fn row_menu(
        &self, ui: &Ui, t: &Theme, resp: &egui::Response, name: &str, args: &Value, result: &str,
    ) {
        let arg = |key: &str| args.get(key).and_then(Value::as_str);
        // A moved note is where it went.
        let path = arg("to").or(arg("path")).map(str::to_string);
        let found = path.as_deref().and_then(|path| {
            let files = self.files.read().unwrap();
            files.by_path(path).map(|f| (f.id, f.parent, f.is_folder()))
        });
        let text = match name {
            "search" => arg("query"),
            "read" | "thought" | "thinking" => Some(result),
            _ => None,
        };
        let chosen = context_menu::show(resp, t, |e| {
            match found {
                Some((id, _, true)) => {
                    e.item(phosphor::FOLDER, "Show in files", RowAction::Show(id))
                }
                Some((id, parent, false)) => {
                    e.item(phosphor::ARROW_SQUARE_OUT, "Open", RowAction::Open(id));
                    e.item(phosphor::FOLDER, "Show in files", RowAction::Show(parent));
                }
                None => {}
            }
            e.separator();
            if let Some(path) = &path {
                e.item(phosphor::LINK, "Copy path", RowAction::Copy(path.clone()));
            }
            if let Some(text) = text.filter(|text| !text.is_empty()) {
                let label = if name == "search" { "Copy query" } else { "Copy text" };
                e.item(phosphor::COPY, label, RowAction::Copy(text.to_string()));
            }
        });
        match chosen {
            Some(RowAction::Open(id)) => ui.ctx().open_file(id, false),
            Some(RowAction::Show(id)) => ui.ctx().focus_folder(id),
            Some(RowAction::Copy(text)) => ui.ctx().copy_text(text),
            None => {}
        }
    }

    /// What a card opens onto, with each file it lists looked up once.
    fn card_body(&self, name: &str, args: &Value, result: &str, ok: bool) -> Vec<rows::Part> {
        let mut parts = rows::body(name, args, result, ok, &self.scope());
        let files = self.files.read().unwrap();
        for part in &mut parts {
            let rows::Part::Files(found) = part else { continue };
            for file in found.iter_mut() {
                file.target = files.by_path(&file.path).map(|f| f.id);
            }
        }
        parts
    }

    /// One piece of an opened card, edge to edge under the bar.
    #[allow(clippy::too_many_arguments)]
    fn tool_part(
        &mut self, ui: &mut Ui, t: &Theme, col_w: f32, (id, from): (Uuid, Uuid), part: &rows::Part,
        query: &str, text_areas: &mut Vec<crate::TextBufferArea>,
    ) {
        let pad = Space::Sm.pts();
        let inner_w = (col_w - pad * 2.0).max(24.0);
        let plain = |ui: &mut Ui, text: &str, ink: Color32| {
            let label = GlyphonLabel::new(text, ink)
                .font_size(TypeRole::Body.size())
                .line_height(TypeRole::Body.line_height())
                .max_width(inner_w);
            let h = label.measure(ui).y;
            let (rect, _) = ui.allocate_exact_size(vec2(col_w, h), Sense::hover());
            place_at(ui, rect.shrink2(vec2(pad, 0.0)), Layout::top_down(Align::Min), |ui| {
                ui.add(label);
            });
        };
        match part {
            rows::Part::Files(files) => {
                for (i, file) in files.iter().enumerate() {
                    self.tool_file(ui, t, (id, i), file, query);
                }
            }
            rows::Part::Note(text) => {
                let h = self.label(id, from).height(text, inner_w);
                let (rect, _) = ui.allocate_exact_size(vec2(col_w, h), Sense::hover());
                let origin = pos2((rect.left() + pad).round(), rect.top());
                let (areas, _) = self.label(id, from).paint_at(ui, text, origin, inner_w);
                text_areas.extend(areas);
            }
            rows::Part::Text(text) => plain(ui, text, t.neutral_fg()),
            rows::Part::Line(text) => plain(ui, text, t.neutral_fg_secondary()),
            rows::Part::Diff(changes) => tool_diff(ui, t, col_w, changes, text_areas),
            rows::Part::Picture(path) => {
                let Some(images) = self.images.clone() else {
                    return plain(ui, path, t.neutral_fg_secondary());
                };
                let drawn = ImageEmbedResolver::new(images, self.id);
                let url = path.replace(' ', "%20");
                // Whole, within the card's width and a screenful's height.
                let size = drawn.size(&url);
                let fit = (inner_w / size.x).min(PICTURE_H / size.y).min(1.0);
                let (rect, _) = ui.allocate_exact_size(vec2(col_w, size.y * fit), Sense::hover());
                let at = Rect::from_min_size(rect.min + vec2(pad, 0.0), size * fit);
                drawn.show(ui, &url, at, CornerRadius::ZERO);
            }
        }
    }

    /// A file a search or a listing found. A click opens a note or points
    /// the file tree at a folder; one that is gone only reads.
    fn tool_file(
        &mut self, ui: &mut Ui, t: &Theme, id: (Uuid, usize), file: &rows::FileRef, query: &str,
    ) {
        let folder = file.path.ends_with('/');
        let path = file.path.trim_end_matches('/');
        let name = path.rsplit('/').next().unwrap_or(path);
        let target = file.target;
        let mut row = FileRow::new(t, display_file_name(name))
            .icon(file_row_icon(name, folder))
            .interactive(target.is_some());
        if !file.snippet.is_empty() {
            row = row.caption_spans(rows::marked(&file.snippet, query));
        } else if !query.is_empty() {
            row = row.subtitle(parent_crumbs(path));
        }
        let resp = row.show(ui, Id::new(("chat_tool_file", id)));
        if let Some(target) = target {
            if resp.hovered() {
                ui.output_mut(|o| o.cursor_icon = CursorIcon::PointingHand);
            }
            if resp.clicked() && folder {
                ui.ctx().focus_folder(target);
            } else if resp.clicked() {
                ui.ctx()
                    .open_file(target, ui.input(|i| i.modifiers.command));
            }
        }
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
        let retry_h =
            if notice.action.is_some() { Space::Xs.pts() + control_height() } else { 0.0 };
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
        if let Some((label, icon)) = notice.action {
            let slot = Rect::from_min_size(
                pos2(title.left(), rect.bottom() - pad - control_height()),
                vec2(text_w, control_height()),
            );
            clicked = place_at(ui, slot, Layout::left_to_right(Align::Center), |ui| {
                Button::quiet(t, label).icon(icon).show(ui).clicked()
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

        // A sheet owns the keyboard while it is open. Otherwise the composer
        // takes it when nothing else has it, and takes it back from a
        // message's selection on a keystroke, so typing always lands.
        let reading = self.reading(ui);
        let typed = ui.input(|i| {
            i.events
                .iter()
                .any(|e| matches!(e, egui::Event::Text(_) | egui::Event::Paste(_)))
        });
        if self.sheet_open() {
            ui.ctx().memory_mut(|m| m.surrender_focus(composer_id));
        } else if !self.initialized || ui.memory(|m| m.focused().is_none()) || (reading && typed) {
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
        // What the workspace sends an editor, such as the link to a picture
        // it just imported, is the composer's while a chat is the tab.
        let sent = self.composer.drain_workspace_events(ui.ctx());
        self.composer.event.internal_events.extend(sent);
        let input = self.composer.handle_input(ui.ctx(), composer_id);
        self.composer.show(ui, text_rect, composer_id);

        let text = self.composer.renderer.buffer.current.text.clone();
        if text.trim().is_empty() {
            let hint = match (self.voice, self.busy, self.editing.is_some()) {
                (true, true, _) => "Speaking",
                (true, false, _) => "Listening",
                (false, _, true) => "Edit and restart from here",
                (false, _, false) => "Message",
            };
            ui.painter().text(
                pos2(text_rect.left(), text_rect.top() + row / 2.0),
                Align2::LEFT_CENTER,
                hint,
                TypeRole::Body.font_id(),
                t.neutral_fg_secondary(),
            );
        }

        let centered = geom.single && !geom.compact;
        let controls_cy = if centered { band.center().y } else { band.max.y - hit / 2.0 };
        let send_rect =
            Rect::from_center_size(pos2(band.max.x - hit / 2.0, controls_cy), vec2(hit, hit));
        let call_rect = self.calls().then(|| {
            Rect::from_center_size(
                pos2(send_rect.left() - gap - hit / 2.0, controls_cy),
                vec2(hit, hit),
            )
        });
        let before_model = call_rect.unwrap_or(send_rect);
        let model_rect = Rect::from_center_size(
            pos2(before_model.left() - gap - geom.model_w / 2.0, controls_cy),
            vec2(geom.model_w, hit),
        );
        let folder_rect = Rect::from_center_size(
            pos2(model_rect.left() - gap - geom.folder_w / 2.0, controls_cy),
            vec2(geom.folder_w, hit),
        );
        for (left, right) in [(model_rect, before_model), (folder_rect, model_rect)]
            .into_iter()
            .chain(call_rect.map(|call| (call, send_rect)))
        {
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
        let menu_open = self.show_chips(ui, t, focused, geom.compact, model_rect, folder_rect);

        if let Some(call_rect) = call_rect {
            let resp = call_button(ui, t, call_rect, self.voice, focused);
            tip_text(ui.ctx(), &resp, if self.voice { "Hang up" } else { "Call" });
            if resp.clicked() {
                self.send_cmd(if self.voice { Cmd::Stop } else { Cmd::StartVoice });
                self.scroll_to_bottom = true;
            }
        }
        let action = match (self.voice, self.busy, text.trim().is_empty()) {
            (true, _, _) | (false, false, true) => Action::Idle,
            (false, true, _) => Action::Stop,
            (false, false, false) => Action::Send,
        };
        let resp = send_button(ui, t, send_rect, action, focused);
        tip_text(ui.ctx(), &resp, if self.busy { "Stop · esc" } else { "Send · return" });

        let esc = (focused || reading)
            && !completions_open
            && !menu_open
            && ui
                .ctx()
                .input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape));
        if esc && reading {
            ui.ctx().memory_mut(|m| m.request_focus(composer_id));
        }
        let mut sent = false;
        if self.voice {
            if esc {
                self.send_cmd(Cmd::Stop);
            }
        } else if self.busy && (resp.clicked() || esc) {
            self.send_cmd(Cmd::Stop);
        } else if esc && self.editing.is_some() {
            self.editing = None;
            self.composer.clear();
            self.composer_text_seq += 1;
        } else if (send_requested || resp.clicked())
            && !text.trim().is_empty()
            && !self.busy
            && !self.voice
        {
            let mentions = self.linked(&text);
            let cmd = match self.editing.take() {
                Some(id) => Cmd::Edit { id, text, mentions },
                None => Cmd::Say { text, mentions },
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
        // While a menu or a sheet is up, the platform's text view takes no
        // touch and draws no caret, so a tap lands on what is up.
        let text_rect = if self.sheet_open() || menu_open {
            Rect::from_min_size(text_rect.min, Vec2::ZERO)
        } else {
            text_rect
        };
        (text_rect, selection_changed, text_changed)
    }

    /// The model and folder pickers inside the composer, each its mark alone
    /// when `compact`. Returns whether a menu or sheet is open.
    fn show_chips(
        &mut self, ui: &mut Ui, t: &Theme, focused: bool, compact: bool, model_rect: Rect,
        folder_rect: Rect,
    ) -> bool {
        let fills = chip_fills(t, focused);
        let settings = self.settings();
        let touch = is_touch(ui.ctx());

        let model_label = self.model_chip_label();
        let mark = self.provider_mark(ui.ctx(), TypeRole::Body.size());
        let label = if compact { "" } else { model_label.as_str() };
        let resp = chip(ui, t, model_rect, mark, label, fills);
        tip_text(ui.ctx(), &resp, format!("{} · {}", self.provider_name(), model_label));
        let mut models_opened_now = false;
        // A finger goes straight to the sheet; a pointer has the quick menu.
        let choice = if touch {
            if resp.clicked() {
                self.open_model_sheet();
                models_opened_now = true;
            }
            None
        } else {
            let (efforts, effort) = (self.efforts(), self.effort().map(str::to_string));
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
            context_menu::show_click(&resp, t, |e| {
                for (sel, label, mark) in &favorites {
                    e.item_icon(*mark, label.clone(), ModelChoice::Use(sel.clone()));
                }
                e.separator();
                // Offered only for a model that has been shown to take it.
                if !efforts.is_empty() {
                    let default = ModelChoice::Effort(None);
                    e.item_checked(effort.is_none(), "Thinking: default", default);
                }
                for value in efforts {
                    let chosen = effort.as_deref() == Some(*value);
                    let pick = ModelChoice::Effort(Some(value.to_string()));
                    e.item_checked(chosen, format!("Thinking: {}", effort_name(value)), pick);
                }
                e.separator();
                e.item(phosphor::LIST, "Browse models…", ModelChoice::Browse);
                e.item(phosphor::FILE_PLUS, "Add a provider…", ModelChoice::AddProvider);
            })
        };
        let menu_open = context_menu::is_open(&resp);
        let mut opened_now = false;
        match choice {
            Some(ModelChoice::Use(selection)) => self.select(selection),
            Some(ModelChoice::Effort(effort)) => self.set_effort(effort),
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
        let name = if compact { "" } else { folder_label.as_str() };
        let (resp, cleared) =
            scope_chip(ui, t, folder_rect, name, self.scope_open, chosen && !compact, fills);
        tip_text(ui.ctx(), &resp, format!("Reads and edits notes in {}", self.scope()));
        let toggled = resp.clicked();
        if cleared {
            self.close_scope_sheet();
            let mut s = settings.clone();
            s.include.clear();
            self.set_settings(s);
        } else if toggled {
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
            // A phone runs no model server, so "this device" is no address
            // to offer there.
            let phone = matches!(ui.ctx().os(), OperatingSystem::Android | OperatingSystem::IOS);
            if phone && std::ptr::eq(template, &OWN) {
                self.setup.base_url.clear();
            }
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
                if Field::new(t, &mut self.setup.base_url)
                    .hint("address, such as linux-box:11434")
                    .width(col_w)
                    .id(Id::new(("chat_setup_url", self.id)))
                    .show(ui)
                    .clicked()
                {
                    ui.ctx().set_virtual_keyboard_shown(true);
                }
                ui.add(Spacer::new(Space::Xs));
            }
            if template.key == KeyNeed::Required {
                if Field::new(t, &mut self.setup.key)
                    .hint(format!("{} API key", template.label))
                    .password(true)
                    .width(col_w)
                    .id(Id::new(("chat_setup_key", self.id)))
                    .show(ui)
                    .clicked()
                {
                    ui.ctx().set_virtual_keyboard_shown(true);
                }
                ui.add(Spacer::new(Space::Xs));
            }
            if template.key != KeyNeed::None
                && Field::new(t, &mut self.setup.model)
                    .hint(if own {
                        "model, or blank for the first the server lists"
                    } else {
                        "model id"
                    })
                    .width(col_w)
                    .id(Id::new(("chat_setup_model", self.id)))
                    .show(ui)
                    .clicked()
            {
                ui.ctx().set_virtual_keyboard_shown(true);
            }
            if own {
                ui.add(Spacer::new(Space::Xs));
                if Field::new(t, &mut self.setup.key)
                    .hint("API key, if the server wants one")
                    .password(true)
                    .width(col_w)
                    .id(Id::new(("chat_setup_key", self.id)))
                    .show(ui)
                    .clicked()
                {
                    ui.ctx().set_virtual_keyboard_shown(true);
                }
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
    /// The button's label and icon, when there is one.
    action: Option<(&'static str, &'static str)>,
}

impl Notice {
    fn error(text: &str, retry: bool) -> Self {
        Self {
            icon: phosphor::WARNING_CIRCLE,
            title: "Couldn't finish that turn".into(),
            detail: Some(text.to_string()),
            danger: true,
            action: retry.then_some(("Retry", phosphor::ARROW_COUNTER_CLOCKWISE)),
        }
    }

    /// A turn stopped short, by a stop, by leaving the chat, or by the app
    /// closing; it goes on from where it was.
    fn unfinished() -> Self {
        Self {
            icon: phosphor::INFO,
            title: "This turn didn't finish".into(),
            detail: None,
            danger: false,
            action: Some(("Resume", phosphor::PLAY)),
        }
    }
}

/// Whether the newest turn of `me`'s stopped short: a message unanswered,
/// a reply cut off, or a round of calls left without its reply. A spoken
/// reply cut off was spoken over, which is no stop.
fn unfinished(entries: &[Entry], me: &str) -> bool {
    let last = entries.iter().rev().find(|e| e.from == me);
    last.is_some_and(|e| match &e.body {
        Body::User { .. } | Body::Tool { .. } => true,
        Body::Assistant { interrupted, spoken, .. } => *interrupted && !*spoken,
        Body::Error { .. } | Body::Other(_) => false,
    })
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
    // With no label the mark is the whole button.
    if label.is_empty() {
        mark.paint(ui.painter(), rect.center(), side, t.neutral_fg());
        return resp;
    }
    mark.paint(ui.painter(), pos2(x + side / 2.0, cy), side, t.neutral_fg());
    let x = x + side + control_space::ICON_GAP.pts();
    let lh = TypeRole::Body.line_height();
    let right = rect.right() - control_space::PAD_X.pts();
    let slot = Rect::from_min_max(pos2(x, cy - lh / 2.0), pos2(right.max(x), cy + lh / 2.0));
    paint_file_name(ui, label, t.neutral_fg(), slot);
    resp
}

/// The folder chip, as search draws its scope: an accent folder in the icon
/// slot, the name, and an X to return to this chat's own folder. With no
/// name the folder is the whole button. Returns the chip response and
/// whether the X was clicked.
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
    if name.is_empty() {
        let half = TypeRole::Body.size() / 2.0;
        paint_glyph(ui, phosphor::FOLDER, t.accent(), rect.center().x - half, cy);
        return (resp, false);
    }
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

/// Where a tool capsule's statement starts, from its left edge: the pad,
/// the icon, the gap. Its opened detail lines up with it.
fn tool_text_inset(ui: &Ui) -> f32 {
    control_space::PAD_X.pts()
        + glyph_width(ui, phosphor::FILE_TEXT)
        + control_space::ICON_GAP.pts()
}

/// A tool statement, or part of one, at body size.
fn statement(spans: Vec<(&str, bool)>, ink: Color32) -> GlyphonLabel<'_> {
    GlyphonLabel::new_rich(spans, ink)
        .font_size(TypeRole::Body.size())
        .line_height(TypeRole::Body.line_height())
}

fn span_refs(spans: &[(String, bool)]) -> Vec<(&str, bool)> {
    spans
        .iter()
        .map(|(text, bold)| (text.as_str(), *bold))
        .collect()
}

fn statement_width(ui: &Ui, spans: &[(String, bool)]) -> f32 {
    statement(span_refs(spans), Color32::PLACEHOLDER)
        .measure(ui)
        .x
}

/// A statement's words as one line of spans no wider than `max_w`. Paths
/// give up their folders first, a letter each from the outside in; after
/// that the longest variable gives up its end.
fn fit_statement(ui: &Ui, words: Vec<(String, bool)>, max_w: f32) -> Vec<(String, bool)> {
    let mut spans: Vec<(String, bool)> = Vec::new();
    for (i, word) in words.into_iter().enumerate() {
        if i > 0 {
            spans.push((" ".into(), false));
        }
        spans.push(word);
    }
    while statement_width(ui, &spans) > max_w {
        let shorter = spans
            .iter_mut()
            .filter(|(_, variable)| *variable)
            .filter_map(|(text, _)| rows::abbreviate(text).map(|shorter| (text, shorter)))
            .max_by_key(|(text, _)| text.len());
        match shorter {
            Some((text, shorter)) => *text = shorter,
            None => break,
        }
    }
    for _ in 0..8 {
        let over = statement_width(ui, &spans) - max_w;
        if over <= 0.0 {
            break;
        }
        let Some((text, _)) = spans
            .iter_mut()
            .filter(|(_, variable)| *variable)
            .max_by_key(|(text, _)| text.len())
        else {
            break;
        };
        let kept: Vec<&str> = text.trim_end_matches('…').graphemes(true).collect();
        if kept.is_empty() {
            break;
        }
        let width = statement(vec![(text.as_str(), true)], Color32::PLACEHOLDER)
            .measure(ui)
            .x;
        let cut = (over * kept.len() as f32 / width).ceil() as usize + 1;
        let keep = kept.len().saturating_sub(cut).max(1);
        *text = format!("{}…", kept[..keep].concat());
    }
    spans
}

/// How a call went, at the right end of its bar: a spinner while it runs, a
/// quiet check, a red x.
fn paint_status(ui: &Ui, t: &Theme, status: Status, right: f32, cy: f32) {
    let (glyph, ink) = match status {
        Status::Running => (phosphor::SPINNER_GAP, t.neutral_fg_secondary()),
        Status::Done => (phosphor::CHECK, t.neutral_fg_secondary()),
        Status::Failed => (phosphor::X, t.danger()),
    };
    let g = ui
        .painter()
        .layout_no_wrap(glyph.into(), phosphor_ui_font_id(), ink);
    let pos = pos2(right - g.size().x, cy - g.size().y / 2.0);
    let mut shape = egui::epaint::TextShape::new(pos, g, ink);
    if status == Status::Running && ui.ctx().style().animation_time >= 0.01 {
        ui.ctx().request_repaint();
        let angle = (ui.input(|i| i.time) * std::f64::consts::TAU) as f32;
        shape = shape.with_angle_and_anchor(angle, Align2::CENTER_CENTER);
    }
    ui.painter().add(shape);
}

/// What an edit changed, as one flow of text: what went is struck through on
/// a red wash, what came is on a green one, and the rest reads as it is.
/// Color is never the only cue: what went is also struck and quieter.
fn tool_diff(
    ui: &mut Ui, t: &Theme, col_w: f32, changes: &[Change],
    text_areas: &mut Vec<crate::TextBufferArea>,
) {
    let long = |text: &str| text.len() > DIFF_INLINE || text.trim_end().contains('\n');
    // Whether the line ends where the piece at `i` does.
    let ends_line = |i: usize| {
        changes[i].text().ends_with('\n')
            || changes
                .get(i + 1)
                .is_none_or(|next| next.text().starts_with('\n'))
    };
    // Quieter than the text around it, and still 4.5 to 1 on its wash.
    let gone_ink = t.neutral_fg_secondary().lerp_to_gamma(t.neutral_fg(), 0.5);
    let mut spans: Vec<(&str, Option<Color32>)> = Vec::new();
    let mut kinds: Vec<Option<bool>> = Vec::new();
    // Whether the flow so far ends a line.
    let mut fresh = true;
    for (i, change) in changes.iter().enumerate() {
        let (ink, kind) = match change {
            Change::Same(_) => (None, None),
            Change::Gone(_) => (Some(gone_ink), Some(false)),
            Change::New(_) => (None, Some(true)),
        };
        spans.push((change.text(), ink));
        kinds.push(kind);
        // Whole lines replaced: the new text starts its own line when
        // either is long, and sits a space away when both are short.
        if let (Change::Gone(went), Some(Change::New(came))) = (change, changes.get(i + 1)) {
            if fresh && ends_line(i + 1) && !went.ends_with('\n') {
                spans.push((if long(went) || long(came) { "\n" } else { " " }, None));
                kinds.push(None);
            }
        }
        fresh = spans.last().is_some_and(|(text, _)| text.ends_with('\n'));
    }
    let pad = Space::Sm.pts();
    let shaped = GlyphonLabel::new_colored(spans, t.neutral_fg())
        .font_size(TypeRole::Body.size())
        .line_height(TypeRole::Body.line_height())
        .max_width((col_w - pad * 2.0).max(24.0))
        .build(ui.ctx());
    let (slot, _) = ui.allocate_exact_size(vec2(col_w, shaped.size.y), Sense::hover());
    let origin = pos2(slot.left() + pad, slot.top());
    if !ui.is_rect_visible(slot) {
        return;
    }
    // The saturated red and green, whichever variant holds them.
    let hues = if t.light() { t.fg() } else { t.bg() };
    let (gone_wash, new_wash) =
        (hues.red.gamma_multiply(DIFF_WASH), hues.green.gamma_multiply(DIFF_WASH));
    for (span, rect) in shaped.span_rects(ui.ctx()) {
        let Some(came) = kinds.get(span).copied().flatten() else { continue };
        // A hair of air on every side keeps neighboring marks apart.
        let rect = rect.translate(origin.to_vec2()).shrink(0.5);
        let wash = if came { new_wash } else { gone_wash };
        ui.painter().rect_filled(rect, Radius::Sm.corner(), wash);
        if !came {
            let y = rect.center().y.round() + 0.5;
            ui.painter()
                .hline(rect.x_range(), y, Stroke::new(STROKE_HAIRLINE, gone_ink));
        }
    }
    let rect = Rect::from_min_size(origin, shaped.size);
    text_areas.push(shaped.text_area(rect, ui.ctx(), ui.clip_rect()));
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

/// What the composer's button does right now.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Action {
    Idle,
    Send,
    Stop,
}

/// The composer's one button. Sending is the accent plate with canvas ink
/// once there is something to send; stopping is a solid red square on the
/// plain plate, so a live run never reads as "go".
fn send_button(
    ui: &mut Ui, t: &Theme, rect: Rect, action: Action, on_canvas: bool,
) -> egui::Response {
    let resp = ui.interact(rect, Id::new(("chat_send", action)), sense_click());
    let plain = chip_fills(t, on_canvas);
    let accent = ControlFills {
        rest: t.accent(),
        hover: t.accent().lerp_to_gamma(t.neutral_bg(), 0.16),
        press: t.accent().lerp_to_gamma(t.neutral_bg(), 0.24),
    };
    let fill = match action {
        Action::Idle => plain.rest,
        Action::Send => interact_fill_response(ui.ctx(), &resp, accent),
        Action::Stop => interact_fill_response(ui.ctx(), &resp, plain),
    };
    ui.painter()
        .rect_filled(rect, Radius::Control.corner(), fill);
    if action != Action::Idle && resp.hovered() {
        ui.output_mut(|o| o.cursor_icon = CursorIcon::PointingHand);
    }
    if action == Action::Stop {
        let square = Rect::from_center_size(rect.center(), Vec2::splat(STOP_SIDE));
        ui.painter()
            .rect_filled(square, CornerRadius::same(2), t.danger());
        return resp;
    }
    let ink = if action == Action::Send { t.neutral_bg() } else { t.neutral_fg_secondary() };
    let g =
        ui.painter()
            .layout_no_wrap(phosphor::PAPER_PLANE_TILT.into(), phosphor_ui_font_id(), ink);
    ui.painter().galley(rect.center() - g.size() / 2.0, g, ink);
    resp
}

/// The call button: a phone on the plain plate, and the phone crossed in
/// red while a call is on.
fn call_button(ui: &mut Ui, t: &Theme, rect: Rect, live: bool, on_canvas: bool) -> egui::Response {
    let resp = ui.interact(rect, Id::new(("chat_call", live)), sense_click());
    let fill = interact_fill_response(ui.ctx(), &resp, chip_fills(t, on_canvas));
    ui.painter()
        .rect_filled(rect, Radius::Control.corner(), fill);
    if resp.hovered() {
        ui.output_mut(|o| o.cursor_icon = CursorIcon::PointingHand);
    }
    let (icon, ink) = if live {
        (phosphor::PHONE_SLASH, t.danger())
    } else {
        (phosphor::PHONE, t.neutral_fg_secondary())
    };
    let g = ui
        .painter()
        .layout_no_wrap(icon.into(), phosphor_ui_font_id(), ink);
    ui.painter().galley(rect.center() - g.size() / 2.0, g, ink);
    resp
}

/// An effort as the interface says it: a provider's "none" is thinking off.
pub(super) fn effort_name(effort: &str) -> &str {
    match effort {
        "none" => "off",
        "xhigh" => "extra high",
        other => other,
    }
}

/// Paints `rect` from `top` at its top edge to `bottom` at its bottom edge.
fn fade(painter: &egui::Painter, rect: Rect, top: Color32, bottom: Color32) {
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(rect.left_top(), top);
    mesh.colored_vertex(rect.right_top(), top);
    mesh.colored_vertex(rect.left_bottom(), bottom);
    mesh.colored_vertex(rect.right_bottom(), bottom);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(1, 2, 3);
    painter.add(egui::Shape::mesh(mesh));
}

/// The widget holding a settled message's text.
fn text_id(id: Uuid) -> Id {
    Id::new(("chat_text", id))
}

/// The row under a message where its actions sit: one icon tall with a
/// little air above and below, kept whether or not the actions show so
/// nothing moves when they do.
fn action_strip(ui: &mut Ui, col_w: f32) -> Rect {
    let h = Space::Xs.pts() + control_icon_hit() + Space::Xs.pts();
    let (strip, _) = ui.allocate_exact_size(vec2(col_w, h), Sense::hover());
    Spacer::paint_at(ui, Space::Xs, Rect::from_min_size(strip.min, vec2(col_w, Space::Xs.pts())));
    Spacer::paint_at(
        ui,
        Space::Xs,
        Rect::from_min_size(
            pos2(strip.left(), strip.bottom() - Space::Xs.pts()),
            vec2(col_w, Space::Xs.pts()),
        ),
    );
    strip
}

/// The icon-sized slot of the strip at `left`.
fn strip_slot(strip: Rect, left: f32) -> Rect {
    Rect::from_min_size(pos2(left, strip.top() + Space::Xs.pts()), Vec2::splat(control_icon_hit()))
}

/// How far a glyph sits inside its hit box, so a slot can put the glyph's
/// edge where the message's is.
fn glyph_inset() -> f32 {
    (control_icon_hit() - TypeRole::Body.size()) / 2.0
}

/// An action in its slot of the strip.
fn action_button(
    ui: &mut Ui, t: &Theme, slot: Rect, glyph: &'static str, active: bool,
) -> egui::Response {
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

    /// A statement too wide for its line: its path gives up folders first,
    /// keeping the note's name, and only then its end. The verb stays.
    #[test]
    fn a_statement_is_cut_to_fit() {
        let ctx = super::super::tests::context();
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let words =
                    |path: &str| vec![("delete".to_string(), false), (path.to_string(), true)];
                let long = format!("/projects/{}note.md", "deeper/".repeat(30));
                let natural = statement_width(ui, &fit_statement(ui, words(&long), f32::MAX));

                let cut = fit_statement(ui, words(&long), natural / 2.0);
                assert!(statement_width(ui, &cut) <= natural / 2.0);
                assert_eq!(cut[0].0, "delete");
                assert!(cut[2].0.starts_with("/p/d/") && cut[2].0.ends_with("/note.md"), "{cut:?}");

                let cut = fit_statement(ui, words(&long), 120.0);
                assert!(statement_width(ui, &cut) <= 120.0);
                assert!(cut[2].0.starts_with("/p/d/") && cut[2].0.ends_with('…'), "{cut:?}");

                assert_eq!(fit_statement(ui, words("…"), 1.0)[2].0, "…");
            });
        });
    }

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
