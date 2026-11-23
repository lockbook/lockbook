pub mod facts;

use std::sync::{Arc, Mutex, RwLock};

use egui::text::{LayoutJob, TextFormat, TextWrapping};
use egui::{
    Align, Align2, Color32, FontFamily, FontId, Galley, Layout, Rect, Sense, Ui, Vec2, pos2, vec2,
};
use lb_rs::Uuid;
use lb_rs::blocking::Lb;
use lb_rs::model::account::Account;
use web_time::{Instant, SystemTime, UNIX_EPOCH};

pub use facts::{Facts, Go, Row, Section, Tone};

use crate::file_cache::{FileCache, FilesExt};
use crate::style::interact::{interact_fill_response, quiet_canvas_fills};
use crate::style::{
    Radius, Space, Spacer, Theme, ThemeExt, TypeRole, control_height, display_file_name,
    file_row_icon, loading_indicator, message_indicator, phosphor, phosphor_font_id, plate_content,
    sense_click, ui_width,
};
use crate::theme::palette_v2::Palette;

const MAX_W: f32 = 620.0;
const LABEL_W: f32 = 150.0;
const ICON_SIZE: f32 = 36.0;
const COPIED_SECS: f32 = 1.5;

#[derive(Default)]
struct Job {
    running: bool,
    done: Option<(u64, Result<Facts, String>)>,
}

#[derive(Clone)]
struct Linked {
    id: Uuid,
    title: String,
    path: String,
    links: usize,
}

enum Links {
    Indexing,
    Notes(Vec<Linked>),
}

pub enum Click {
    Go(Go, bool),
    Copy(String),
}

pub struct FileInfo {
    pub id: Uuid,
    lb: Lb,
    files: Arc<RwLock<FileCache>>,
    account: Account,
    ctx: egui::Context,
    job: Arc<Mutex<Job>>,
    wanted: u64,
    started: u64,
    shown: Option<Result<Facts, String>>,
    generation: u64,
    links: Option<(u64, Links)>,
    copied: Option<(String, Instant)>,
}

impl FileInfo {
    pub fn new(
        id: Uuid, lb: &Lb, files: Arc<RwLock<FileCache>>, account: Account, ctx: egui::Context,
    ) -> Self {
        let generation = files.read().unwrap().generation;
        Self {
            id,
            lb: lb.clone(),
            files,
            account,
            ctx,
            job: Default::default(),
            wanted: 1,
            started: 0,
            shown: None,
            generation,
            links: None,
            copied: None,
        }
    }

    pub fn refresh(&mut self) {
        self.wanted += 1;
        self.ctx.request_repaint();
    }

    pub fn settled(&self) -> bool {
        self.shown.is_some() && self.wanted == self.started && !self.job.lock().unwrap().running
    }

    fn pump(&mut self) {
        let generation = self.files.read().unwrap().generation;
        if generation != self.generation {
            self.generation = generation;
            self.wanted += 1;
        }
        let mut job = self.job.lock().unwrap();
        if let Some((_, result)) = job.done.take() {
            self.shown = Some(result);
        }
        if self.wanted > self.started && !job.running {
            job.running = true;
            self.started = self.wanted;
            drop(job);
            let (lb, files, account) = (self.lb.clone(), self.files.clone(), self.account.clone());
            let (id, at, job, ctx) = (self.id, self.started, self.job.clone(), self.ctx.clone());
            lb_rs::spawn!({
                let result = facts::gather(&lb, &files, &account, id);
                let mut job = job.lock().unwrap();
                job.running = false;
                job.done = Some((at, result));
                ctx.request_repaint();
            });
        }
    }

    fn linked(&mut self) -> Option<&Links> {
        let is_doc = matches!(&self.shown, Some(Ok(facts)) if !facts.is_folder);
        if !is_doc {
            return None;
        }
        let files = self.files.read().unwrap();
        files.notes.want();
        let version = files.notes.version();
        let fresh = matches!(&self.links, Some((at, Links::Notes(_))) if *at == version);
        if !fresh {
            let links = if !files.notes.is_ready() {
                Links::Indexing
            } else {
                let mut notes: Vec<Linked> = vec![];
                for b in files.notes.backlinks(&*files, self.id) {
                    match notes.last_mut() {
                        Some(note) if note.id == b.note => note.links += 1,
                        _ => {
                            let name = files.get_by_id(b.note).map_or("", |f| f.name.as_str());
                            notes.push(Linked {
                                id: b.note,
                                title: crate::file_cache::wiki_title(name).to_string(),
                                path: files.path(b.note),
                                links: 1,
                            });
                        }
                    }
                }
                Links::Notes(notes)
            };
            self.links = Some((version, links));
        }
        drop(files);
        self.links.as_ref().map(|(_, links)| links)
    }

    pub fn show(&mut self, ui: &mut Ui) -> Option<(Go, bool)> {
        self.pump();
        let t = ui.ctx().get_lb_theme();
        match &self.shown {
            None => {
                loading_indicator(ui);
                return None;
            }
            Some(Err(msg)) => {
                message_indicator(ui, phosphor::WARNING_CIRCLE, msg);
                return None;
            }
            Some(Ok(_)) => {}
        }
        let links_section = self.linked().map(links_section);
        let Some(Ok(facts)) = &self.shown else { return None };

        if self
            .copied
            .as_ref()
            .is_some_and(|(_, at)| at.elapsed().as_secs_f32() > COPIED_SECS)
        {
            self.copied = None;
        }
        let copied = self.copied.as_ref().map(|(text, _)| text.as_str());
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as i64);

        let mut click = None;
        ui.spacing_mut().item_spacing = Vec2::ZERO;
        egui::ScrollArea::vertical()
            .id_salt("file_info")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.with_layout(Layout::top_down(Align::Min), |ui| {
                    ui.spacing_mut().item_spacing = Vec2::ZERO;
                    let avail = ui_width(ui);
                    let col_w = (avail - 2.0 * Space::Lg.pts()).clamp(0.0, MAX_W);
                    let side = ((avail - col_w) / 2.0).max(0.0);
                    ui.add(Spacer::new(Space::Xl));
                    ui.horizontal_top(|ui| {
                        ui.add_space(side);
                        ui.vertical(|ui| {
                            ui.set_width(col_w);
                            ui.spacing_mut().item_spacing = Vec2::ZERO;
                            header(ui, &t, facts);
                            ui.add(Spacer::new(Space::Lg));
                            let mut sections: Vec<&Section> = facts.sections.iter().collect();
                            if let Some(links) = &links_section {
                                let at = sections
                                    .iter()
                                    .position(|s| s.title == "Storage and sync")
                                    .unwrap_or(sections.len());
                                sections.insert(at, links);
                            }
                            for section in sections {
                                if let Some(c) = show_section(ui, &t, section, copied, now) {
                                    click = Some(c);
                                }
                            }
                        });
                    });
                    ui.add(Spacer::new(Space::Xl));
                });
            });

        if self.copied.is_some() {
            ui.ctx().request_repaint_after_secs(COPIED_SECS);
        }
        match click? {
            Click::Copy(text) => {
                ui.ctx().copy_text(text.clone());
                self.copied = Some((text, Instant::now()));
                None
            }
            Click::Go(go, new_tab) => Some((go, new_tab)),
        }
    }
}

fn links_section(links: &Links) -> Section {
    let mut section = Section { title: "Linked from", rows: vec![] };
    match links {
        Links::Indexing => section
            .rows
            .push(Row { tone: Tone::Muted, ..Row::new("Backlinks", "Reading your notes…") }),
        Links::Notes(notes) if notes.is_empty() => section
            .rows
            .push(Row { tone: Tone::Muted, ..Row::new("Backlinks", "No notes link here") }),
        Links::Notes(notes) => {
            for note in notes {
                let count = facts::count(note.links, "link", "links");
                section.rows.push(Row {
                    go: Some(Go::Open(note.id)),
                    ..Row::new(note.title.clone(), format!("{count} · {}", note.path))
                });
            }
        }
    }
    section
}

fn wrapped(ui: &Ui, parts: &[(&str, FontId, Color32)], max_w: f32) -> Arc<Galley> {
    let mut job = LayoutJob {
        wrap: TextWrapping { max_width: max_w.max(1.0), ..Default::default() },
        ..Default::default()
    };
    for (text, font_id, color) in parts {
        job.append(
            text,
            0.0,
            TextFormat { font_id: font_id.clone(), color: *color, ..Default::default() },
        );
    }
    ui.fonts_mut(|f| f.layout_job(job))
}

fn bold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(Arc::from("Bold")))
}

fn header(ui: &mut Ui, t: &Theme, facts: &Facts) {
    let w = ui_width(ui);
    let gap = Space::Md.pts();
    let text_w = (w - ICON_SIZE - gap).max(1.0);
    let name = wrapped(
        ui,
        &[(display_file_name(&facts.name), bold(TypeRole::Title.size()), t.neutral_fg())],
        text_w,
    );
    let summary = wrapped(
        ui,
        &[(&facts.summary, TypeRole::Body.font_id(), t.neutral_fg_secondary())],
        text_w,
    );
    let path =
        wrapped(ui, &[(&facts.path, TypeRole::Mono.font_id(), t.neutral_fg_secondary())], text_w);
    let text_h =
        name.size().y + Space::Xxs.pts() + summary.size().y + Space::Xxs.pts() + path.size().y;
    let h = text_h.max(ICON_SIZE);
    let (rect, _) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
    let icon = file_row_icon(&facts.name, facts.is_folder);
    ui.painter().text(
        pos2(rect.left() + ICON_SIZE / 2.0, rect.top() + ICON_SIZE / 2.0),
        Align2::CENTER_CENTER,
        icon,
        phosphor_font_id(ICON_SIZE),
        t.accent(),
    );
    let x = rect.left() + ICON_SIZE + gap;
    let mut y = rect.top();
    let name_h = name.size().y;
    ui.painter().galley(pos2(x, y), name, t.neutral_fg());
    y += name_h + Space::Xxs.pts();
    let summary_h = summary.size().y;
    ui.painter()
        .galley(pos2(x, y), summary, t.neutral_fg_secondary());
    y += summary_h + Space::Xxs.pts();
    ui.painter()
        .galley(pos2(x, y), path, t.neutral_fg_secondary());
}

fn show_section(
    ui: &mut Ui, t: &Theme, section: &Section, copied: Option<&str>, now: i64,
) -> Option<Click> {
    if section.rows.is_empty() {
        return None;
    }
    let w = ui_width(ui);
    let caption = wrapped(
        ui,
        &[(
            &TypeRole::caps(section.title),
            bold(TypeRole::Caption.size()),
            t.neutral_fg_secondary(),
        )],
        w,
    );
    let (rect, _) = ui.allocate_exact_size(vec2(w, caption.size().y), Sense::hover());
    ui.painter().galley(
        pos2(rect.left() + Space::Md.pts(), rect.top()),
        caption,
        t.neutral_fg_secondary(),
    );
    ui.add(Spacer::new(Space::Xs));

    let mut click = None;
    plate_content(ui, t.neutral_bg(), t.neutral(), Radius::Control.corner(), |ui| {
        ui.spacing_mut().item_spacing = Vec2::ZERO;
        ui.set_width(w);
        ui.add(Spacer::new(Space::Xxs));
        for row in &section.rows {
            let is_copied = row.copy.is_some() && row.copy.as_deref() == copied;
            if let Some(c) = show_row(ui, t, row, is_copied, now) {
                click = Some(c);
            }
        }
        ui.add(Spacer::new(Space::Xxs));
    });
    ui.add(Spacer::new(Space::Lg));
    click
}

fn tone_ink(t: &Theme, tone: Tone) -> Color32 {
    match tone {
        Tone::Plain => t.neutral_fg(),
        Tone::Muted => t.neutral_fg_secondary(),
        Tone::Warn => t.fg().get_color(Palette::Yellow),
    }
}

fn show_row(ui: &mut Ui, t: &Theme, row: &Row, copied: bool, now: i64) -> Option<Click> {
    let w = ui_width(ui);
    let inset = Space::Xxs.pts();
    let pad_l = Space::Md.pts() - inset;
    let pad_r = Space::Sm.pts();
    let gap = Space::Sm.pts();
    let pad_y = Space::Xs.pts();
    let label_w = LABEL_W.min(w * 0.38);
    let glyph = match (&row.go, &row.copy) {
        (Some(_), _) => Some(phosphor::CARET_RIGHT),
        (None, Some(_)) if copied => Some(phosphor::CHECK),
        (None, Some(_)) => Some(phosphor::COPY),
        _ => None,
    };
    let glyph_w = if glyph.is_some() { TypeRole::Body.size() + gap } else { 0.0 };
    let value_w = (w - 2.0 * inset - pad_l - label_w - gap - glyph_w - pad_r).max(1.0);

    let label =
        wrapped(ui, &[(&row.label, TypeRole::Body.font_id(), t.neutral_fg_secondary())], label_w);
    let ink = tone_ink(t, row.tone);
    let relative = row
        .when
        .map(|ms| format!(" · {}", facts::relative(ms, now)));
    let mut parts = vec![(row.value.as_str(), TypeRole::Body.font_id(), ink)];
    if let Some(rel) = &relative {
        parts.push((rel.as_str(), TypeRole::Body.font_id(), t.neutral_fg_secondary()));
    }
    let value = wrapped(ui, &parts, value_w);

    let h = (label.size().y.max(value.size().y) + 2.0 * pad_y).max(control_height());
    let interactive = glyph.is_some();
    let sense = if interactive { sense_click() } else { Sense::hover() };
    let (outer, resp) = ui.allocate_exact_size(vec2(w, h), sense);
    let rect = Rect::from_min_max(
        pos2(outer.left() + inset, outer.top()),
        pos2(outer.right() - inset, outer.bottom()),
    );
    if interactive {
        let fill = interact_fill_response(ui.ctx(), &resp, quiet_canvas_fills(t));
        ui.painter().rect_filled(rect, Radius::Sm.corner(), fill);
    }

    let line_top =
        |g: &Galley| rect.top() + (h - g.size().y.max(TypeRole::Body.line_height())) / 2.0;
    let top = line_top(&label).min(line_top(&value));
    let top = if label.size().y.max(value.size().y) > TypeRole::Body.line_height() * 1.2 {
        rect.top() + pad_y
    } else {
        top
    };
    let x = rect.left() + pad_l;
    ui.painter()
        .galley(pos2(x, top), label, t.neutral_fg_secondary());
    ui.painter()
        .galley(pos2(x + label_w + gap, top), value, ink);
    if let Some(glyph) = glyph {
        let ink = if copied { t.accent() } else { t.neutral_fg_secondary() };
        ui.painter().text(
            pos2(rect.right() - pad_r, rect.center().y),
            Align2::RIGHT_CENTER,
            glyph,
            phosphor_font_id(TypeRole::Body.size()),
            ink,
        );
    }

    if !interactive {
        return None;
    }
    let resp = match (&row.go, copied) {
        (Some(Go::Open(_)), _) => resp.on_hover_text("Open"),
        (Some(Go::Info(_)), _) => resp.on_hover_text("Show info"),
        (None, true) => resp.on_hover_text("Copied"),
        (None, false) => resp.on_hover_text("Copy"),
    };
    if !resp.clicked() && !resp.middle_clicked() {
        return None;
    }
    let new_tab = resp.middle_clicked() || ui.input(|i| i.modifiers.command);
    match (&row.go, &row.copy) {
        (Some(go), _) => Some(Click::Go(*go, new_tab)),
        (None, Some(text)) => Some(Click::Copy(text.clone())),
        _ => None,
    }
}
