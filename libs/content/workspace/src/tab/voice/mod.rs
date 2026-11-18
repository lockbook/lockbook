pub mod format;
pub mod host;

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use egui::{
    Align, Align2, Color32, FontId, Galley, Pos2, Rect, Sense, Shape, Stroke, UiBuilder, Vec2,
    pos2, vec2,
};
use lb_rs::Uuid;
use lb_rs::model::file_metadata::DocumentHmac;

use crate::safe_area;
use crate::style::claim;
use crate::theme::palette_v2::{Theme, ThemeExt as _, readable_on};

use format::{Meta, Transcript, VoiceFile};
use host::{Event, Phase, Request, VoiceHost};

const MAX_WIDTH: f32 = 640.0;
const PAD: f32 = 20.0;
const LEVEL_BARS: usize = 48;
const LEVEL_EVERY_MS: u64 = 70;
const WORD_SIZE: f32 = 19.0;

pub struct VoiceNote {
    pub id: Uuid,
    pub hmac: Option<DocumentHmac>,
    pub embedded: bool,
    file: VoiceFile,
    error: Option<String>,
    host: Arc<VoiceHost>,
    engaged: bool,
    audio_path: Option<PathBuf>,
    audio_gen: u32,
    recording: Option<Recording>,
    playing: bool,
    position_ms: u64,
    scrubbing: bool,
    transcribing: Option<Progress>,
    layout: Option<WordLayout>,
    follow_word: Option<usize>,
    changed: bool,
}

struct Recording {
    started: bool,
    elapsed_ms: u64,
    levels: VecDeque<f32>,
    next_level_ms: u64,
    stopping: bool,
}

struct Progress {
    phase: Phase,
    fraction: f32,
    preview: Option<String>,
    started: Instant,
}

struct WordLayout {
    width: f32,
    height: f32,
    words: Vec<PlacedWord>,
}

struct PlacedWord {
    galley: Arc<Galley>,
    rect: Rect,
    start_ms: u64,
}

impl VoiceNote {
    pub fn new(ctx: &egui::Context, id: Uuid, hmac: Option<DocumentHmac>, bytes: &[u8]) -> Self {
        let (file, error) = match VoiceFile::decode(bytes) {
            Ok(file) => (file, None),
            Err(err) => (VoiceFile::default(), Some(err)),
        };
        Self {
            id,
            hmac,
            embedded: false,
            file,
            error,
            host: host::get(ctx),
            engaged: false,
            audio_path: None,
            audio_gen: 0,
            recording: None,
            playing: false,
            position_ms: 0,
            scrubbing: false,
            transcribing: None,
            layout: None,
            follow_word: None,
            changed: false,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        self.file.encode()
    }

    pub fn saved(&mut self, hmac: DocumentHmac) {
        self.hmac = Some(hmac);
    }

    pub fn reload(&mut self, bytes: &[u8], hmac: Option<DocumentHmac>) -> bool {
        let Ok(mut incoming) = VoiceFile::decode(bytes) else { return false };
        self.hmac = hmac;
        let same_audio = incoming.audio == self.file.audio;
        if same_audio && incoming.meta.transcript.is_none() && self.file.meta.transcript.is_some() {
            incoming.meta.transcript = self.file.meta.transcript.take();
            self.changed = true;
        }
        if !same_audio {
            if let Some(old) = self.audio_path.take() {
                let _ = std::fs::remove_file(old);
            }
            self.audio_gen += 1;
            self.position_ms = 0;
            if self.playing {
                self.playing = false;
                self.request(Request::Pause { id: self.id });
            }
        }
        self.file = incoming;
        self.layout = None;
        true
    }

    pub fn transcript(&self) -> Option<&Transcript> {
        self.file.meta.transcript.as_ref()
    }

    fn request(&mut self, request: Request) {
        self.engaged = true;
        self.host.request(request);
    }

    fn audio_path(&mut self) -> Option<String> {
        if self.audio_path.is_none() {
            let dir = std::env::temp_dir().join("lockbook-voice");
            let path = dir.join(format!("{}-{}.m4a", self.id, self.audio_gen));
            let written =
                std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(&path, &self.file.audio));
            if let Err(err) = written {
                self.error = Some(format!("Couldn't prepare the audio: {err}"));
                return None;
            }
            self.audio_path = Some(path);
        }
        self.audio_path
            .as_ref()
            .map(|p| p.to_string_lossy().into_owned())
    }

    fn replace_audio(&mut self, audio: Vec<u8>, duration_ms: u64) {
        if let Some(old) = self.audio_path.take() {
            let _ = std::fs::remove_file(old);
        }
        self.audio_gen += 1;
        self.file = VoiceFile {
            meta: Meta { audio_type: "m4a".into(), duration_ms, transcript: None },
            audio,
        };
        self.layout = None;
        self.position_ms = 0;
        self.playing = false;
        self.changed = true;
    }

    fn drain(&mut self) {
        for event in self.host.take_events(self.id) {
            match event {
                Event::Recording { elapsed_ms, level, .. } => {
                    if let Some(rec) = &mut self.recording {
                        rec.started = true;
                        rec.elapsed_ms = elapsed_ms;
                        while elapsed_ms >= rec.next_level_ms {
                            rec.levels.push_back(level.clamp(0.0, 1.0));
                            if rec.levels.len() > LEVEL_BARS {
                                rec.levels.pop_front();
                            }
                            rec.next_level_ms += LEVEL_EVERY_MS;
                        }
                    }
                }
                Event::Recorded { path, duration_ms, .. } => {
                    self.recording = None;
                    match std::fs::read(&path) {
                        Ok(audio) => {
                            let _ = std::fs::remove_file(&path);
                            self.replace_audio(audio, duration_ms);
                        }
                        Err(err) => {
                            self.error = Some(format!("Couldn't read the recording: {err}"))
                        }
                    }
                }
                Event::RecordFailed { message, .. } => {
                    self.recording = None;
                    self.error = Some(message);
                }
                Event::Playback { position_ms, playing, .. } => {
                    self.playing = playing;
                    if !self.scrubbing {
                        self.position_ms = position_ms.min(self.file.meta.duration_ms.max(1));
                    }
                }
                Event::PlaybackFailed { message, .. } => {
                    self.playing = false;
                    self.error = Some(message);
                }
                Event::Transcribing { phase, fraction, preview, .. } => {
                    let started = self
                        .transcribing
                        .as_ref()
                        .map(|p| p.started)
                        .unwrap_or_else(Instant::now);
                    let fraction = match &self.transcribing {
                        Some(p) if p.phase == phase => fraction.max(p.fraction),
                        _ => fraction,
                    };
                    let preview = preview
                        .or_else(|| self.transcribing.as_mut().and_then(|p| p.preview.take()));
                    self.transcribing = Some(Progress { phase, fraction, preview, started });
                }
                Event::Transcribed { transcript, .. } => {
                    self.transcribing = None;
                    self.file.meta.transcript = Some(transcript);
                    self.layout = None;
                    self.changed = true;
                }
                Event::TranscribeFailed { message, .. } => {
                    self.transcribing = None;
                    self.error = Some(message);
                }
            }
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) -> bool {
        self.drain();

        let ctx = ui.ctx().clone();
        let t = ctx.get_lb_theme();
        let host_rect = ui.available_rect_before_wrap();
        ui.painter().rect_filled(host_rect, 0., t.neutral_bg());

        let inset = safe_area::inset_rect(&ctx, host_rect).shrink(PAD);
        let width = inset.width().min(MAX_WIDTH);
        let area = Rect::from_min_size(
            pos2(inset.center().x - width / 2.0, inset.min.y),
            vec2(width, inset.height().max(0.0)),
        );

        if self.recording.is_some() {
            self.show_recording(ui, area, &t);
        } else if !self.file.has_audio() {
            self.show_empty(ui, area, &t);
        } else {
            self.show_recorded(ui, area, &t);
        }

        if self.recording.is_some() || self.playing || self.transcribing.is_some() {
            ctx.request_repaint_after(Duration::from_millis(33));
        }

        claim(ui, host_rect);
        std::mem::take(&mut self.changed)
    }

    fn show_error(&mut self, ui: &mut egui::Ui, pos: Pos2, width: f32, t: &Theme) -> f32 {
        let Some(error) = &self.error else { return 0.0 };
        let galley =
            ui.painter()
                .layout(error.clone(), FontId::proportional(14.0), t.danger(), width);
        let height = galley.size().y;
        ui.painter().galley(pos, galley, t.danger());
        let rect = Rect::from_min_size(pos, vec2(width, height));
        if ui
            .interact(rect, ui.scope_id().with("voice_error"), Sense::click())
            .clicked()
        {
            self.error = None;
        }
        height + 12.0
    }

    fn show_empty(&mut self, ui: &mut egui::Ui, area: Rect, t: &Theme) {
        let center = pos2(area.center().x, area.min.y + area.height() * 0.38);
        let available = VoiceHost::available();

        let radius = 44.0;
        let button = Rect::from_center_size(center, Vec2::splat(radius * 2.0));
        let resp = ui.interact(button, ui.scope_id().with("voice_record"), Sense::click());
        let ring =
            if resp.is_pointer_button_down_on() { t.neutral_fg_secondary() } else { t.neutral() };
        let painter = ui.painter();
        painter.circle_stroke(center, radius, Stroke::new(3.0, ring));
        let red = if available { t.danger() } else { t.neutral() };
        painter.circle_filled(center, radius - 8.0, red);

        let label =
            if available { "Tap to record" } else { "Recording needs the iPhone app for now" };
        painter.text(
            center + vec2(0.0, radius + 26.0),
            Align2::CENTER_CENTER,
            label,
            FontId::proportional(16.0),
            t.neutral_fg(),
        );
        painter.text(
            center + vec2(0.0, radius + 50.0),
            Align2::CENTER_CENTER,
            "Transcribe it on this device when you're done",
            FontId::proportional(13.0),
            t.neutral_fg_secondary(),
        );

        let error_top = center.y + radius + 76.0;
        self.show_error(ui, pos2(area.min.x, error_top), area.width(), t);

        if resp.clicked() && available {
            self.error = None;
            self.recording = Some(Recording {
                started: false,
                elapsed_ms: 0,
                levels: VecDeque::new(),
                next_level_ms: 0,
                stopping: false,
            });
            self.request(Request::Record { id: self.id });
        }
    }

    fn show_recording(&mut self, ui: &mut egui::Ui, area: Rect, t: &Theme) {
        let Some(rec) = &self.recording else { return };
        let painter = ui.painter().clone();

        let timer_center = pos2(area.center().x, area.min.y + area.height() * 0.2);
        painter.text(
            timer_center,
            Align2::CENTER_CENTER,
            clock(rec.elapsed_ms, true),
            FontId::proportional(52.0),
            t.neutral_fg(),
        );
        painter.text(
            timer_center + vec2(0.0, 44.0),
            Align2::CENTER_CENTER,
            if rec.stopping {
                "Saving…"
            } else if rec.started {
                "Recording"
            } else {
                "Starting…"
            },
            FontId::proportional(14.0),
            t.danger(),
        );

        let meter = Rect::from_center_size(
            pos2(area.center().x, timer_center.y + 130.0),
            vec2(area.width(), 90.0),
        );
        let gap = 3.0;
        let bar_w =
            ((meter.width() - gap * (LEVEL_BARS as f32 - 1.0)) / LEVEL_BARS as f32).max(1.0);
        let offset = LEVEL_BARS.saturating_sub(rec.levels.len());
        for i in 0..LEVEL_BARS {
            let level = if i >= offset { rec.levels[i - offset] } else { 0.0 };
            let h = (level * meter.height()).max(4.0);
            let x = meter.min.x + i as f32 * (bar_w + gap);
            let bar =
                Rect::from_center_size(pos2(x + bar_w / 2.0, meter.center().y), vec2(bar_w, h));
            let color = if i >= offset { t.danger() } else { t.neutral_bg_tertiary() };
            painter.rect_filled(bar, bar_w / 2.0, color);
        }

        let center = pos2(area.center().x, meter.max.y + 110.0);
        let radius = 44.0;
        let button = Rect::from_center_size(center, Vec2::splat(radius * 2.0));
        let resp = ui.interact(button, ui.scope_id().with("voice_stop"), Sense::click());
        painter.circle_stroke(center, radius, Stroke::new(3.0, t.neutral()));
        painter.rect_filled(Rect::from_center_size(center, Vec2::splat(30.0)), 6.0, t.danger());
        painter.text(
            center + vec2(0.0, radius + 26.0),
            Align2::CENTER_CENTER,
            "Tap to stop",
            FontId::proportional(14.0),
            t.neutral_fg_secondary(),
        );

        if resp.clicked() {
            if let Some(rec) = &mut self.recording {
                if !rec.stopping {
                    rec.stopping = true;
                    self.request(Request::Stop { id: self.id });
                }
            }
        }
    }

    fn show_recorded(&mut self, ui: &mut egui::Ui, area: Rect, t: &Theme) {
        let mut y = area.min.y;
        y += self.show_player(
            ui,
            Rect::from_min_size(pos2(area.min.x, y), vec2(area.width(), 56.0)),
            t,
        );
        y += 16.0;
        y += self.show_error(ui, pos2(area.min.x, y), area.width(), t);

        let rest = Rect::from_min_max(pos2(area.min.x, y), area.max);
        if self.transcribing.is_some() {
            self.show_progress(ui, rest, t);
        } else if self.file.meta.transcript.is_some() {
            self.show_transcript(ui, rest, t);
        } else {
            self.show_transcribe_button(ui, rest, t);
        }
    }

    fn show_player(&mut self, ui: &mut egui::Ui, row: Rect, t: &Theme) -> f32 {
        let duration = self.file.meta.duration_ms.max(1);
        let painter = ui.painter().clone();

        let card = row;
        painter.rect_filled(card, 14.0, t.neutral_bg_secondary());

        let button_center = pos2(card.min.x + 30.0, card.center().y);
        let button = Rect::from_center_size(button_center, Vec2::splat(40.0));
        let play_resp =
            ui.interact(button.expand(6.0), ui.scope_id().with("voice_play"), Sense::click());
        painter.circle_filled(button_center, 20.0, t.accent());
        let ink = readable_on(Color32::WHITE, t.accent());
        if self.playing {
            for dx in [-5.0, 5.0] {
                painter.rect_filled(
                    Rect::from_center_size(button_center + vec2(dx, 0.0), vec2(5.0, 16.0)),
                    1.5,
                    ink,
                );
            }
        } else {
            let c = button_center + vec2(2.0, 0.0);
            painter.add(Shape::convex_polygon(
                vec![c + vec2(-6.0, -9.0), c + vec2(9.0, 0.0), c + vec2(-6.0, 9.0)],
                ink,
                Stroke::NONE,
            ));
        }

        let time_text = format!("{} / {}", clock(self.position_ms, false), clock(duration, false));
        let time_galley =
            painter.layout_no_wrap(time_text, FontId::proportional(13.0), t.neutral_fg_secondary());
        let time_w = time_galley.size().x;
        let time_pos =
            pos2(card.max.x - 14.0 - time_w, card.center().y - time_galley.size().y / 2.0);
        painter.galley(time_pos, time_galley, t.neutral_fg_secondary());

        let track = Rect::from_min_max(
            pos2(button.max.x + 16.0, card.center().y - 2.0),
            pos2(time_pos.x - 14.0, card.center().y + 2.0),
        );
        let scrub = ui.interact(
            track.expand2(vec2(10.0, 22.0)),
            ui.scope_id().with("voice_scrub"),
            Sense::click_and_drag(),
        );
        if let Some(pointer) = scrub.interact_pointer_pos() {
            if scrub.dragged() || scrub.clicked() || scrub.is_pointer_button_down_on() {
                let frac = ((pointer.x - track.min.x) / track.width().max(1.0)).clamp(0.0, 1.0);
                self.position_ms = (frac * duration as f32) as u64;
                self.scrubbing = true;
            }
        }
        if self.scrubbing
            && (scrub.drag_stopped() || scrub.clicked() || !scrub.is_pointer_button_down_on())
        {
            self.scrubbing = false;
            if self.playing {
                self.request(Request::Seek { id: self.id, ms: self.position_ms });
            }
        }

        let frac = (self.position_ms as f32 / duration as f32).clamp(0.0, 1.0);
        painter.rect_filled(track, 2.0, t.neutral_bg_tertiary());
        let filled =
            Rect::from_min_max(track.min, pos2(track.min.x + track.width() * frac, track.max.y));
        painter.rect_filled(filled, 2.0, t.accent());
        let knob = if self.scrubbing { 9.0 } else { 7.0 };
        painter.circle_filled(pos2(filled.max.x, track.center().y), knob, t.accent());

        if play_resp.clicked() {
            self.toggle_play();
        }
        row.height()
    }

    fn toggle_play(&mut self) {
        if self.playing {
            self.playing = false;
            self.request(Request::Pause { id: self.id });
        } else {
            let from_ms = if self.position_ms + 100 >= self.file.meta.duration_ms {
                0
            } else {
                self.position_ms
            };
            self.play_from(from_ms);
        }
    }

    fn play_from(&mut self, from_ms: u64) {
        let Some(path) = self.audio_path() else { return };
        self.error = None;
        self.position_ms = from_ms;
        self.playing = true;
        self.request(Request::Play { id: self.id, path, from_ms });
    }

    fn start_transcription(&mut self) {
        let Some(path) = self.audio_path() else { return };
        self.error = None;
        self.transcribing = Some(Progress {
            phase: Phase::Preparing,
            fraction: 0.0,
            preview: None,
            started: Instant::now(),
        });
        self.request(Request::Transcribe { id: self.id, path });
    }

    fn show_transcribe_button(&mut self, ui: &mut egui::Ui, rest: Rect, t: &Theme) {
        let available = VoiceHost::available();
        let size = vec2(rest.width().min(320.0), 50.0);
        let button =
            Rect::from_min_size(pos2(rest.center().x - size.x / 2.0, rest.min.y + 24.0), size);
        let resp = ui.interact(button, ui.scope_id().with("voice_transcribe"), Sense::click());
        let fill = if !available {
            t.neutral()
        } else if resp.is_pointer_button_down_on() {
            t.wash_toward_neutral_fg(t.accent(), Theme::PRESS_WASH)
        } else {
            t.accent()
        };
        let painter = ui.painter();
        painter.rect_filled(button, 12.0, fill);
        painter.text(
            button.center(),
            Align2::CENTER_CENTER,
            "Transcribe",
            FontId::proportional(17.0),
            readable_on(Color32::WHITE, fill),
        );
        painter.text(
            pos2(button.center().x, button.max.y + 22.0),
            Align2::CENTER_CENTER,
            if available {
                "Runs on this device with Apple's speech model"
            } else {
                "Transcription needs the iPhone app for now"
            },
            FontId::proportional(13.0),
            t.neutral_fg_secondary(),
        );
        if resp.clicked() && available {
            self.start_transcription();
        }
    }

    fn show_progress(&mut self, ui: &mut egui::Ui, rest: Rect, t: &Theme) {
        let Some(progress) = &self.transcribing else { return };
        let painter = ui.painter().clone();
        let mut y = rest.min.y + 8.0;

        let pct = (progress.fraction.clamp(0.0, 1.0) * 100.0).round() as u32;
        let label = match progress.phase {
            Phase::Preparing => "Getting the speech model ready…".to_string(),
            Phase::Downloading => format!("Downloading the speech model · {pct}%"),
            Phase::Transcribing => format!("Transcribing · {pct}%"),
        };
        painter.text(
            pos2(rest.min.x, y),
            Align2::LEFT_TOP,
            label,
            FontId::proportional(16.0),
            t.neutral_fg(),
        );
        let elapsed = progress.started.elapsed().as_secs_f32();
        painter.text(
            pos2(rest.max.x, y + 2.0),
            Align2::RIGHT_TOP,
            format!("{elapsed:.1} s"),
            FontId::proportional(13.0),
            t.neutral_fg_secondary(),
        );
        y += 30.0;

        let bar = Rect::from_min_size(pos2(rest.min.x, y), vec2(rest.width(), 6.0));
        painter.rect_filled(bar, 3.0, t.neutral_bg_tertiary());
        let indeterminate = progress.phase == Phase::Preparing;
        if indeterminate {
            let phase = (elapsed * 0.8).fract();
            let w = bar.width() * 0.25;
            let x = bar.min.x + (bar.width() + w) * phase - w;
            let seg = Rect::from_min_max(
                pos2(x.max(bar.min.x), bar.min.y),
                pos2((x + w).min(bar.max.x), bar.max.y),
            );
            painter.rect_filled(seg, 3.0, t.accent());
        } else {
            let fill = Rect::from_min_size(
                bar.min,
                vec2(bar.width() * progress.fraction.clamp(0.0, 1.0), bar.height()),
            );
            painter.rect_filled(fill, 3.0, t.accent());
        }
        y += 24.0;

        if let Some(preview) = &progress.preview {
            let galley = painter.layout(
                preview.trim().to_string(),
                FontId::proportional(WORD_SIZE),
                t.neutral_fg_secondary(),
                rest.width(),
            );
            let clip = Rect::from_min_max(pos2(rest.min.x, y), rest.max);
            let top = (clip.max.y - galley.size().y).min(y);
            painter.with_clip_rect(clip).galley(
                pos2(rest.min.x, top),
                galley,
                t.neutral_fg_secondary(),
            );
        }
    }

    fn show_transcript(&mut self, ui: &mut egui::Ui, rest: Rect, t: &Theme) {
        let Some(transcript) = self.file.meta.transcript.clone() else { return };
        let painter = ui.painter().clone();

        let audio_s = self.file.meta.duration_ms as f32 / 1000.0;
        let took_s = (transcript.took_ms as f32 / 1000.0).max(0.001);
        let stats = if transcript.took_ms > 0 {
            format!(
                "{:.1} s of audio in {:.2} s ({:.0}× real time) · {}",
                audio_s,
                took_s,
                audio_s / took_s,
                transcript.locale
            )
        } else {
            transcript.locale.clone()
        };
        painter.text(
            rest.min,
            Align2::LEFT_TOP,
            "Transcript",
            FontId::proportional(16.0),
            t.neutral_fg(),
        );
        let again = painter.layout_no_wrap(
            "Transcribe again".into(),
            FontId::proportional(14.0),
            t.accent(),
        );
        let again_rect =
            Rect::from_min_size(pos2(rest.max.x - again.size().x, rest.min.y + 1.0), again.size());
        let again_resp =
            ui.interact(again_rect.expand(8.0), ui.scope_id().with("voice_again"), Sense::click());
        painter.galley(again_rect.min, again, t.accent());
        painter.text(
            rest.min + vec2(0.0, 24.0),
            Align2::LEFT_TOP,
            stats,
            FontId::proportional(12.0),
            t.neutral_fg_secondary(),
        );

        let words_rect = Rect::from_min_max(rest.min + vec2(0.0, 52.0), rest.max);
        if words_rect.height() > 0.0 {
            self.show_words(ui, words_rect, &transcript, t);
        }

        if again_resp.clicked() && VoiceHost::available() {
            self.start_transcription();
        }
    }

    fn layout_words(&mut self, ui: &egui::Ui, transcript: &Transcript, width: f32) {
        if self.layout.as_ref().is_some_and(|l| l.width == width) {
            return;
        }
        let font = FontId::proportional(WORD_SIZE);
        let painter = ui.painter();
        let space = painter
            .layout_no_wrap(" ".into(), font.clone(), Color32::PLACEHOLDER)
            .size()
            .x;
        let line_h = painter
            .layout_no_wrap("Ag".into(), font.clone(), Color32::PLACEHOLDER)
            .size()
            .y
            * 1.45;

        let mut words = Vec::new();
        let mut x = 0.0f32;
        let mut y = 0.0f32;
        let mut last_end: Option<u64> = None;
        for segment in &transcript.segments {
            let mut first = true;
            for word in &segment.words {
                let text = word.text.trim();
                if text.is_empty() {
                    continue;
                }
                let paragraph =
                    first && last_end.is_some_and(|end| word.start_ms.saturating_sub(end) >= 1500);
                let spaced = word.text.starts_with(char::is_whitespace) || (first && x > 0.0);
                first = false;
                if paragraph {
                    x = 0.0;
                    y += line_h * 1.5;
                } else if spaced && x > 0.0 {
                    x += space;
                }
                let galley =
                    painter.layout_no_wrap(text.to_string(), font.clone(), Color32::PLACEHOLDER);
                let size = galley.size();
                if x > 0.0 && x + size.x > width {
                    x = 0.0;
                    y += line_h;
                }
                let rect = Rect::from_min_size(pos2(x, y), vec2(size.x, line_h));
                x += size.x;
                words.push(PlacedWord { galley, rect, start_ms: word.start_ms });
                last_end = Some(word.end_ms);
            }
        }
        self.layout = Some(WordLayout { width, height: y + line_h, words });
    }

    fn current_word(&self) -> Option<usize> {
        if !self.playing && self.position_ms == 0 {
            return None;
        }
        let words = &self.layout.as_ref()?.words;
        let i = words.partition_point(|w| w.start_ms <= self.position_ms);
        i.checked_sub(1)
    }

    fn show_words(&mut self, ui: &mut egui::Ui, rect: Rect, transcript: &Transcript, t: &Theme) {
        let width = rect.width();
        self.layout_words(ui, transcript, width);
        let current = self.current_word();
        let follow = self.playing && current.is_some() && current != self.follow_word;
        if follow {
            self.follow_word = current;
        }

        let mut tapped: Option<u64> = None;
        let mut child = ui.new_child(UiBuilder::new().max_rect(rect));
        let layout = self.layout.as_ref().unwrap();
        egui::ScrollArea::vertical()
            .id_salt(("voice_words", self.id))
            .auto_shrink([false, false])
            .show_viewport(&mut child, |ui, viewport| {
                ui.set_min_size(vec2(width, layout.height + 120.0));
                let origin = ui.min_rect().min;
                let painter = ui.painter();
                for (i, word) in layout.words.iter().enumerate() {
                    if word.rect.max.y < viewport.min.y || word.rect.min.y > viewport.max.y {
                        continue;
                    }
                    let screen = word.rect.translate(origin.to_vec2());
                    let color = match current {
                        Some(c) if i == c => {
                            painter.rect_filled(
                                screen.expand2(vec2(3.0, -2.0)),
                                5.0,
                                t.accent_bg(),
                            );
                            t.neutral_fg()
                        }
                        Some(c) if i < c => t.neutral_fg(),
                        Some(_) => t.neutral_fg_secondary(),
                        None => t.neutral_fg(),
                    };
                    let text_y = screen.center().y - word.galley.size().y / 2.0;
                    painter.galley(pos2(screen.min.x, text_y), word.galley.clone(), color);
                }
                if follow {
                    if let Some(word) = current.and_then(|c| layout.words.get(c)) {
                        if !viewport.contains_rect(word.rect) {
                            ui.scroll_to_rect(
                                word.rect.translate(origin.to_vec2()),
                                Some(Align::Center),
                            );
                        }
                    }
                }
                let area = Rect::from_min_size(origin, vec2(width, layout.height));
                let resp = ui.interact(area, ui.scope_id().with("voice_words_tap"), Sense::click());
                if resp.clicked() {
                    if let Some(pos) = resp.interact_pointer_pos() {
                        let local = pos - origin.to_vec2();
                        tapped = layout
                            .words
                            .iter()
                            .find(|w| w.rect.expand2(vec2(4.0, 0.0)).contains(local))
                            .map(|w| w.start_ms);
                    }
                }
            });

        if let Some(ms) = tapped {
            if self.playing {
                self.position_ms = ms;
                self.request(Request::Seek { id: self.id, ms });
            } else {
                self.play_from(ms);
            }
        }
    }
}

impl Drop for VoiceNote {
    fn drop(&mut self) {
        if self.engaged {
            self.host.request(Request::Release { id: self.id });
        }
    }
}

fn clock(ms: u64, tenths: bool) -> String {
    let total = ms / 1000;
    let (m, s) = (total / 60, total % 60);
    if tenths { format!("{m}:{s:02}.{}", (ms % 1000) / 100) } else { format!("{m}:{s:02}") }
}

#[cfg(test)]
mod tests {
    use super::format::{Segment, Word};
    use super::*;

    fn file(audio: &[u8], transcript: Option<&str>) -> Vec<u8> {
        VoiceFile {
            meta: Meta {
                audio_type: "m4a".into(),
                duration_ms: 1000,
                transcript: transcript.map(|text| Transcript {
                    engine: "test".into(),
                    locale: "en_US".into(),
                    took_ms: 1,
                    segments: vec![Segment {
                        words: vec![Word { text: text.into(), start_ms: 0, end_ms: 500 }],
                    }],
                }),
            },
            audio: audio.to_vec(),
        }
        .encode()
    }

    #[test]
    fn reload_keeps_a_local_transcript_the_newer_version_lacks() {
        let ctx = egui::Context::default();
        let mut note = VoiceNote::new(&ctx, Uuid::new_v4(), None, &file(b"audio", Some("mine")));
        assert!(note.reload(&file(b"audio", None), None));
        assert_eq!(note.transcript().unwrap().text(), "mine");
        assert!(note.changed);
    }

    #[test]
    fn reload_takes_new_audio_and_its_transcript() {
        let ctx = egui::Context::default();
        let mut note = VoiceNote::new(&ctx, Uuid::new_v4(), None, &file(b"old", Some("old words")));
        assert!(note.reload(&file(b"new", Some("new words")), None));
        assert_eq!(note.transcript().unwrap().text(), "new words");
        assert_eq!(note.file.audio, b"new");
        assert!(!note.changed);
    }

    #[test]
    fn reload_refuses_unreadable_bytes() {
        let ctx = egui::Context::default();
        let mut note = VoiceNote::new(&ctx, Uuid::new_v4(), None, &file(b"audio", None));
        assert!(!note.reload(b"not a voice file", None));
    }

    #[test]
    fn clock_formats() {
        assert_eq!(clock(83_450, false), "1:23");
        assert_eq!(clock(83_450, true), "1:23.4");
    }
}
