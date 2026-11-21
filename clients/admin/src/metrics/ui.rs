use super::history::{DAY, History};
use super::queries::{self, Cohort, Retention, SeriesPoint};
use super::{Result, format_bytes, read_cache, source};
use eframe::egui::{
    self, Align2, Color32, Context, FontId, Frame, Pos2, Rect, RichText, Stroke, Ui, Vec2,
};
use std::path::PathBuf;
use std::sync::{Arc, mpsc};
use time::OffsetDateTime;
use time::macros::format_description;
use workspace_rs::theme::icons::Icon;
use workspace_rs::widgets::Button;

const DATE: &[time::format_description::BorrowedFormatItem<'static>] =
    format_description!("[year]-[month]-[day]");
const TIME: &[time::format_description::BorrowedFormatItem<'static>] =
    format_description!("[year]-[month]-[day] [hour]:[minute] UTC");
const COLORS: [Color32; 4] = [
    Color32::from_rgb(90, 125, 241),
    Color32::from_rgb(33, 155, 128),
    Color32::from_rgb(196, 118, 62),
    Color32::from_rgb(163, 109, 208),
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Overview,
    Retention,
    Accounts,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Job {
    Open,
    Refresh,
    Cached,
}

enum Update {
    Progress(String),
    Loaded(Result<Option<History>>, Job, Option<u64>),
    Analysis((i64, i64), Arc<History>, Analysis),
}

struct Analysis {
    overview: Vec<SeriesPoint>,
    retention: Retention,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Sort {
    Storage,
    Egress,
    Activity,
    Age,
    Name,
}

pub struct Metrics {
    directory: PathBuf,
    remote: String,
    path: String,
    history: Option<Arc<History>>,
    tab: Tab,
    range: Option<(i64, i64)>,
    preset_days: Option<i64>,
    draft_start: String,
    draft_end: String,
    range_error: Option<String>,
    at: Option<i64>,
    sequence_text: String,
    exact: Option<u64>,
    source_status: Option<String>,
    progress: Option<String>,
    query: Option<Analysis>,
    sort: Sort,
    search: String,
    selected_account: Option<usize>,
    sender: mpsc::Sender<Update>,
    updates: mpsc::Receiver<Update>,
    busy: bool,
}

impl Metrics {
    pub fn new(directory: PathBuf, remote: String, logs: Option<PathBuf>, ctx: &Context) -> Self {
        let (sender, updates) = mpsc::channel();
        let mut app = Self {
            directory,
            remote,
            path: String::new(),
            history: None,
            tab: Tab::Overview,
            range: None,
            preset_days: Some(30),
            draft_start: String::new(),
            draft_end: String::new(),
            range_error: None,
            at: None,
            sequence_text: String::new(),
            exact: None,
            source_status: None,
            progress: None,
            query: None,
            sort: Sort::Storage,
            search: String::new(),
            selected_account: None,
            sender,
            updates,
            busy: false,
        };
        let job = if let Some(path) = logs {
            app.path = path.to_string_lossy().into_owned();
            Job::Open
        } else {
            Job::Cached
        };
        app.load(job, ctx);
        app
    }

    fn load(&mut self, job: Job, ctx: &Context) {
        if self.busy {
            return;
        }
        self.busy = true;
        self.progress = Some("Opening logs…".into());
        self.source_status = None;
        let sender = self.sender.clone();
        let ctx = ctx.clone();
        let directory = self.directory.clone();
        let remote = self.remote.clone();
        let path = PathBuf::from(&self.path);
        std::thread::spawn(move || {
            let mut transferred = None;
            let result = match job {
                Job::Open => source::local(&path).map(Some),
                Job::Cached => read_cache(&directory),
                Job::Refresh => source::refresh(&directory, &remote, |text| {
                    let _ = sender.send(Update::Progress(text));
                    ctx.request_repaint();
                })
                .map(|(history, bytes)| {
                    transferred = Some(bytes);
                    Some(history)
                }),
            };
            let _ = sender.send(Update::Loaded(result, job, transferred));
            ctx.request_repaint();
        });
    }

    fn updates(&mut self, ctx: &Context) {
        while let Ok(update) = self.updates.try_recv() {
            match update {
                Update::Progress(progress) => self.progress = Some(progress),
                Update::Analysis(range, history, analysis) => {
                    if self.range == Some(range)
                        && self
                            .history
                            .as_ref()
                            .is_some_and(|current| Arc::ptr_eq(current, &history))
                    {
                        self.query = Some(analysis);
                    }
                }
                Update::Loaded(result, job, bytes) => {
                    self.busy = false;
                    self.progress = None;
                    match result {
                        Ok(Some(history)) => {
                            let coverage = history
                                .anchors
                                .first()
                                .zip(history.anchors.last())
                                .map(|(&(start, _), &(end, _))| (start, end));
                            self.source_status = Some(match job {
                                Job::Open => self.path.clone(),
                                Job::Cached => "Production cache".to_owned(),
                                Job::Refresh if bytes == Some(0) => {
                                    "Up to date · 0 log bytes fetched".to_owned()
                                }
                                Job::Refresh => format!(
                                    "{} of log data fetched",
                                    format_bytes(bytes.unwrap_or_default())
                                ),
                            });
                            self.history = Some(Arc::new(history));
                            self.selected_account = None;
                            self.exact = None;
                            self.query = None;
                            if let Some((first, last)) = coverage {
                                if job != Job::Refresh {
                                    self.preset_days = Some(30);
                                }
                                let (start, end) = match self.preset_days {
                                    Some(days) => (preset_start(days, last).max(first), last),
                                    None => self
                                        .range
                                        .filter(|(start, end)| *start >= first && *end <= last)
                                        .unwrap_or(((last - 30 * DAY).max(first), last)),
                                };
                                self.set_range(start, end, ctx);
                            } else {
                                self.range = None;
                                self.at = None;
                            }
                        }
                        Ok(None) => {}
                        Err(error) => self.source_status = Some(error.to_string()),
                    }
                }
            }
        }
    }

    fn set_range(&mut self, start: i64, end: i64, ctx: &Context) {
        self.range = Some((start, end));
        self.draft_start = date(start);
        self.draft_end = date(end);
        self.at = Some(end);
        self.exact = None;
        self.range_error = None;
        self.query = None;
        let Some(history) = self.history.clone() else { return };
        let sender = self.sender.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let overview = queries::overview(&history, start, end);
            let retention = queries::retention(&history, start, end);
            let _ = sender.send(Update::Analysis(
                (start, end),
                history,
                Analysis { overview, retention },
            ));
            ctx.request_repaint();
        });
    }

    fn selected(&self, history: &History) -> Option<(u64, Option<i64>)> {
        if let Some(seq) = self.exact {
            return Some((seq, history.timestamp_at(seq)));
        }
        if let Some(time) = self.at {
            return history.sequence_at(time).map(|seq| (seq, Some(time)));
        }
        Some((history.latest, history.timestamp_at(history.latest)))
    }

    fn controls(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        ui.vertical_centered(|ui| {
            Frame::window(ui.style()).inner_margin(egui::Margin::symmetric(7, 4)).show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    for (tab, label) in [(Tab::Overview, "Overview"), (Tab::Retention, "Retention"), (Tab::Accounts, "Accounts")] {
                        if ui.selectable_label(self.tab == tab, label).clicked() { self.tab = tab; }
                    }
                    ui.separator();
                    let label = self.range.map_or_else(|| "Date range".into(), |(start, end)| {
                        if date(start) == date(end) { date(start) } else { format!("{} – {}", date(start), date(end)) }
                    });
                    let range_button = Button::default().text(&label).show(ui);
                    egui::Popup::menu(&range_button).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
                        let coverage = self.history.as_ref().and_then(|history| {
                            history.anchors.first().zip(history.anchors.last()).map(|(&(first, _), &(last, _))| (first, last))
                        });
                        if let Some((first, last)) = coverage {
                            ui.horizontal(|ui| {
                                for (days, label) in [(0, "Today"), (7, "7 days"), (14, "14 days"), (30, "30 days")] {
                                    if ui.button(label).clicked() {
                                        self.preset_days = Some(days);
                                        let start = preset_start(days, last);
                                        self.set_range(start.max(first), last, &ctx);
                                        ui.close();
                                    }
                                }
                            });
                            ui.separator();
                            ui.label("Custom range (UTC)");
                            ui.horizontal(|ui| {
                                ui.add(egui::TextEdit::singleline(&mut self.draft_start).desired_width(112.0).hint_text("Start date"));
                                ui.add(egui::TextEdit::singleline(&mut self.draft_end).desired_width(112.0).hint_text("End date"));
                                if ui.button("Apply").clicked() {
                                    match (parse_date(&self.draft_start), parse_date(&self.draft_end)) {
                                        (Some(start), Some(end)) if start <= end && end + DAY > first && start <= last => {
                                            self.preset_days = None;
                                            self.set_range(start.max(first), (end + DAY - 1).min(last), &ctx);
                                            ui.close();
                                        }
                                        _ => self.range_error = Some("Enter dates within the imported coverage (YYYY-MM-DD).".into()),
                                    }
                                }
                            });
                            if let Some(error) = &self.range_error { ui.label(error); }
                        } else {
                            ui.label("Import logs with dated events to select a range.");
                        }
                    });
                    let source = Button::default().icon(&Icon::SYNC.size(16.0)).text("Source").show(ui);
                    egui::Popup::menu(&source).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
                        if ui.add_enabled(!self.busy, egui::Button::new("Refresh from production")).clicked() {
                            self.load(Job::Refresh, &ctx);
                        }
                        ui.horizontal(|ui| {
                            ui.add(egui::TextEdit::singleline(&mut self.path).desired_width(215.0).hint_text("Local ServerV6 directory"));
                            if ui.add_enabled(!self.busy && !self.path.is_empty(), egui::Button::new("Open local logs")).clicked() {
                                self.load(Job::Open, &ctx);
                            }
                        });
                        if let Some(progress) = &self.progress { ui.label(progress); }
                        if let Some(status) = &self.source_status { ui.label(status); }
                        if let Some(history) = &self.history {
                            ui.label(format!("Sequences {}–{}", history.oldest, history.latest));
                            if let Some((first, last)) = history.anchors.first().zip(history.anchors.last()) {
                                ui.label(format!("{} – {}", timestamp(first.0), timestamp(last.0)));
                            }
                            if !history.gaps.is_empty() { ui.label(format!("{} gaps in coverage", history.gaps.len())); }
                            if history.partial_tail { ui.label("Import ends at the last complete transaction."); }
                        }
                    });
                });
            });
        });
    }

    fn state_control(&mut self, ui: &mut Ui, history: &History) {
        ui.horizontal_wrapped(|ui| {
            let title = match self.selected(history) {
                Some((seq, Some(time))) if self.exact.is_none() => {
                    format!("State at {} · estimated · sequence {seq}", timestamp(time))
                }
                Some((seq, _)) => format!("State at sequence {seq}"),
                None => "State unavailable for this date".into(),
            };
            let button = Button::default().text(&title).show(ui);
            egui::Popup::menu(&button)
                .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                .show(|ui| {
                    ui.label("Exact state by sequence");
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut self.sequence_text)
                                .desired_width(110.0)
                                .hint_text("Sequence"),
                        );
                        if ui.button("Go").clicked() {
                            if let Ok(seq) = self.sequence_text.parse::<u64>() {
                                if history.sequence_covered(seq) {
                                    self.exact = Some(seq);
                                    self.at = history.timestamp_at(seq);
                                    ui.close();
                                }
                            }
                        }
                    });
                    if !self.sequence_text.is_empty()
                        && !self
                            .sequence_text
                            .parse::<u64>()
                            .is_ok_and(|seq| history.sequence_covered(seq))
                    {
                        ui.label("Sequence outside continuous imported coverage.");
                    }
                    if ui.button("End of range").clicked() {
                        self.exact = None;
                        self.at = self.range.map(|(_, end)| end);
                        ui.close();
                    }
                });
            if self.busy {
                ui.spinner();
            }
        });
    }

    fn overview(&mut self, ui: &mut Ui, history: &History) {
        if let Some((seq, _)) = self.selected(history) {
            if let Some(state) = history.checkpoint(seq) {
                ui.horizontal_wrapped(|ui| {
                    ui.label(format!("{} users", state.users));
                    ui.separator();
                    ui.label(format!("{} metadata entries", state.files));
                    ui.separator();
                    ui.label(if state.unknown_storage == 0 {
                        format_bytes(state.storage)
                    } else {
                        "Storage unavailable".into()
                    });
                });
            }
        }
        let Some(analysis) = &self.query else { return };
        let range = self.range.unwrap();
        let fields: [(&str, Unit, &[Line]); 9] = [
            ("Users", Unit::Count, &[("Total users", |p| p.users)]),
            ("Signups · trailing 7 days", Unit::Count, &[("Signups", |p| p.signups)]),
            (
                "Active users",
                Unit::Count,
                &[
                    ("24 hours", |p| p.active[0]),
                    ("48 hours", |p| p.active[1]),
                    ("7 days", |p| p.active[2]),
                    ("30 days", |p| p.active[3]),
                ],
            ),
            ("DAU / MAU", Unit::Percent, &[("Stickiness", |p| p.stickiness)]),
            ("Sharing", Unit::Count, &[("Users", |p| p.sharing)]),
            (
                "Premium",
                Unit::Count,
                &[
                    ("Total", |p| p.premium[0]),
                    ("Stripe", |p| p.premium[1]),
                    ("Google Play", |p| p.premium[2]),
                    ("App Store", |p| p.premium[3]),
                ],
            ),
            ("Metadata entries", Unit::Count, &[("Entries", |p| p.files)]),
            ("Document storage", Unit::Bytes, &[("Storage", |p| p.storage)]),
            ("Server egress · trailing 30 days", Unit::Bytes, &[("Egress", |p| p.egress)]),
        ];
        for (title, unit, lines) in fields {
            let series: Vec<_> = lines
                .iter()
                .map(|(label, field)| {
                    (
                        *label,
                        analysis
                            .overview
                            .iter()
                            .map(|point| (point.time, field(point)))
                            .collect(),
                    )
                })
                .collect();
            if let Some(time) = chart(ui, title, range, self.at, unit, &series) {
                self.at = Some(time);
                self.exact = None;
            }
        }
    }

    fn retention(&mut self, ui: &mut Ui) {
        let Some(analysis) = &self.query else { return };
        let range = self.range.unwrap();
        ui.horizontal_wrapped(|ui| {
            for (index, state) in analysis.retention.headline.into_iter().enumerate() {
                ui.vertical(|ui| {
                    let label = if index == 0 {
                        if date(range.1) == date(current_time()) {
                            "Today · Day 0".to_owned()
                        } else {
                            format!("Day 0 · {}", date(range.1))
                        }
                    } else {
                        format!("Day {}", queries::RETENTION[index])
                    };
                    ui.label(label);
                    match state {
                        Cohort::Complete(count) | Cohort::Partial(count) => {
                            ui.heading(
                                count
                                    .percent()
                                    .map_or_else(|| "–".to_owned(), |value| format!("{value:.1}%")),
                            );
                            ui.weak(format!(
                                "{} of {} accounts{}",
                                count.returned,
                                count.eligible,
                                if matches!(state, Cohort::Partial(_)) {
                                    " · partial day"
                                } else {
                                    ""
                                }
                            ));
                            if index != 0 && analysis.retention.excluded[index] != 0 {
                                ui.weak(format!(
                                    "{} cohorts missing coverage",
                                    analysis.retention.excluded[index]
                                ));
                            }
                        }
                        Cohort::Pending => {
                            ui.weak("Not yet observable");
                        }
                        Cohort::Missing => {
                            ui.weak("Missing coverage");
                        }
                    }
                });
                if index != 3 {
                    ui.separator();
                }
            }
        });
        let series: Vec<_> = ["Day 0", "Day 7", "Day 14", "Day 30"]
            .into_iter()
            .enumerate()
            .map(|(index, label)| {
                (
                    label,
                    analysis
                        .retention
                        .cohorts
                        .iter()
                        .map(|(day, values)| ((*day).max(range.0), values[index].rate()))
                        .collect(),
                )
            })
            .collect();
        if let Some(time) =
            chart(ui, "Retention by signup date", range, self.at, Unit::Percent, &series)
        {
            self.at = Some(time);
            self.exact = None;
        }
    }

    fn accounts(&mut self, ui: &mut Ui, history: &History) {
        let Some((seq, time)) = self.selected(history) else { return };
        if !history.sequence_covered(seq) {
            ui.label("Account state is unavailable in this gap.");
            return;
        }
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.search)
                    .desired_width(245.0)
                    .hint_text("Search accounts"),
            );
            if self.selected_account.is_some() && ui.button("Back to accounts").clicked() {
                self.selected_account = None;
            }
        });
        if let Some(index) = self.selected_account {
            let account = &history.accounts[index];
            ui.heading(account.display_name().as_ref());
            if let Some(created) = account.created {
                ui.weak(format!("Created {}", date(created)));
            }
            if let Some(range) = self.range {
                let times = queries::sample_times(range.0, range.1);
                for (label, getter) in [
                    ("Storage", (|s: &super::history::AccountState| s.storage) as fn(_) -> _),
                    ("Cumulative egress", |s| Some(s.egress)),
                ] {
                    let values = times
                        .iter()
                        .map(|&time| {
                            (
                                time,
                                history
                                    .sequence_at(time)
                                    .filter(|seq| history.sequence_covered(*seq))
                                    .and_then(|seq| account.at(seq))
                                    .and_then(getter)
                                    .map(|value| value as f64),
                            )
                        })
                        .collect();
                    if let Some(time) =
                        chart(ui, label, range, self.at, Unit::Bytes, &[(label, values)])
                    {
                        self.at = Some(time);
                        self.exact = None;
                    }
                }
                let values = times
                    .iter()
                    .map(|&time| {
                        let start = (time - DAY).max(range.0);
                        (
                            time,
                            history
                                .covered(start, time)
                                .then(|| account.active_hours(start, time)),
                        )
                    })
                    .collect();
                if let Some(time) = chart(
                    ui,
                    "Active hours · trailing day",
                    range,
                    self.at,
                    Unit::Hours,
                    &[("Hours", values)],
                ) {
                    self.at = Some(time);
                    self.exact = None;
                }
            }
            return;
        }
        let search = self.search.to_lowercase();
        let egress_from = time.and_then(|time| {
            history
                .covered(time - 30 * DAY, time)
                .then(|| history.sequence_at(time - 30 * DAY))
                .flatten()
        });
        let activity_window = time
            .zip(self.range)
            .filter(|(end, (start, _))| history.covered(*start, *end));
        let mut rows: Vec<_> = history
            .accounts
            .iter()
            .enumerate()
            .filter_map(|(index, account)| {
                let state = account.at(seq)?;
                if !state.live {
                    return None;
                }
                let name = account.display_name();
                if !name.to_lowercase().contains(&search) {
                    return None;
                }
                let age = account
                    .created
                    .zip(time)
                    .map(|(created, time)| (time - created).max(0) / DAY);
                let egress = egress_from.and_then(|from| {
                    state
                        .egress
                        .checked_sub(account.at(from).map_or(0, |prior| prior.egress))
                });
                let active =
                    activity_window.map(|(end, (start, _))| account.active_hours(start, end));
                Some((index, state, age, egress, active, name))
            })
            .collect();
        rows.sort_by(|a, b| {
            match self.sort {
                Sort::Storage => b.1.storage.cmp(&a.1.storage),
                Sort::Egress => b.3.cmp(&a.3),
                Sort::Activity => b.4.partial_cmp(&a.4).unwrap_or(std::cmp::Ordering::Equal),
                Sort::Age => b.2.cmp(&a.2),
                Sort::Name => a.5.cmp(&b.5),
            }
            .then_with(|| a.5.cmp(&b.5))
        });
        let width = (ui.available_width() - 4.0 * ui.spacing().item_spacing.x) / 5.0;
        ui.horizontal(|ui| {
            for (sort, label) in [
                (Sort::Name, "Account"),
                (Sort::Age, "Age"),
                (Sort::Activity, "Active hours"),
                (Sort::Storage, "Storage"),
                (Sort::Egress, "30d egress"),
            ] {
                if ui
                    .add_sized([width, 26.0], egui::Button::selectable(self.sort == sort, label))
                    .clicked()
                {
                    self.sort = sort;
                }
            }
        });
        egui::ScrollArea::vertical().show_rows(ui, 26.0, rows.len(), |ui, range| {
            for (index, state, age, egress, active, name) in &rows[range] {
                ui.horizontal(|ui| {
                    if ui
                        .add_sized([width, 26.0], egui::Button::new(name.as_ref()).frame(false))
                        .clicked()
                    {
                        self.selected_account = Some(*index);
                    }
                    for label in [
                        age.map_or_else(|| "–".into(), |value| format!("{value} d")),
                        active.map_or_else(|| "–".into(), |value| format!("{value:.1} h")),
                        state.storage.map_or_else(|| "–".into(), format_bytes),
                        egress.map_or_else(|| "–".into(), format_bytes),
                    ] {
                        ui.add_sized([width, 26.0], egui::Label::new(label));
                    }
                });
            }
        });
    }
}

impl eframe::App for Metrics {
    fn ui(&mut self, ui: &mut Ui, _: &mut eframe::Frame) {
        self.updates(ui.ctx());
        let available_width = ui.available_width();
        let toolbar = egui::Area::new(egui::Id::unique("metrics toolbar"))
            .anchor(Align2::CENTER_TOP, [0.0, 16.0])
            .order(egui::Order::Foreground)
            .show(ui.ctx(), |ui| {
                ui.set_max_width((available_width - 36.0).min(730.0));
                self.controls(ui);
            });
        Frame::new()
            .inner_margin(egui::Margin::same(20))
            .show(ui, |ui| {
                ui.add_space(toolbar.response.rect.height() + 12.0);
                let Some(history) = self.history.clone() else {
                    ui.vertical_centered(|ui| {
                        if let Some(progress) = &self.progress {
                            ui.spinner();
                            ui.label(progress);
                        } else {
                            ui.label("Open a log directory or refresh from production.");
                            if let Some(status) = &self.source_status {
                                ui.weak(status);
                            }
                        }
                    });
                    return;
                };
                self.state_control(ui, &history);
                ui.add_space(8.0);
                if self.query.is_none() && self.range.is_some() {
                    ui.spinner();
                }
                if self.tab == Tab::Accounts && self.selected_account.is_none() {
                    self.accounts(ui, &history);
                    return;
                }
                egui::ScrollArea::vertical()
                    .id_salt(self.tab as u8)
                    .show(ui, |ui| match self.tab {
                        Tab::Overview => self.overview(ui, &history),
                        Tab::Retention => self.retention(ui),
                        Tab::Accounts => self.accounts(ui, &history),
                    });
            });
    }
}

type Line = (&'static str, fn(&SeriesPoint) -> Option<f64>);
type PlotSeries<'a> = (&'a str, Vec<(i64, Option<f64>)>);

#[derive(Clone, Copy)]
enum Unit {
    Count,
    Bytes,
    Percent,
    Hours,
}
impl Unit {
    fn label(self, value: f64) -> String {
        match self {
            Self::Count => format!("{value:.0}"),
            Self::Bytes => format_bytes(value as u64),
            Self::Percent => format!("{value:.1}%"),
            Self::Hours => format!("{value:.1} h"),
        }
    }
}

fn chart(
    ui: &mut Ui, title: &str, (start, end): (i64, i64), selected: Option<i64>, unit: Unit,
    series: &[PlotSeries<'_>],
) -> Option<i64> {
    ui.label(RichText::new(title).strong());
    if series.len() > 1 {
        ui.horizontal_wrapped(|ui| {
            for (index, (label, _)) in series.iter().enumerate() {
                ui.colored_label(COLORS[index], *label);
            }
        });
    }
    let (response, painter) =
        ui.allocate_painter(Vec2::new(ui.available_width(), 138.0), egui::Sense::click());
    let rect = Rect::from_min_max(
        response.rect.min + Vec2::new(75.0, 11.0),
        response.rect.max - Vec2::new(16.0, 24.0),
    );
    let maximum = series
        .iter()
        .flat_map(|(_, values)| values.iter().filter_map(|(_, value)| *value))
        .fold(0.0f64, f64::max);
    let limit = if matches!(unit, Unit::Percent) { 100.0 } else { maximum.max(1.0) };
    let span = (end - start).max(1) as f64;
    let x = |time: i64| rect.left() + ((time - start) as f64 / span) as f32 * rect.width();
    let y = |value: f64| rect.bottom() - (value / limit).clamp(0.0, 1.0) as f32 * rect.height();
    for value in [0.0, limit / 2.0, limit] {
        let y = y(value);
        painter.hline(rect.x_range(), y, ui.visuals().widgets.noninteractive.bg_stroke);
        painter.text(
            Pos2::new(rect.left() - 8.0, y),
            Align2::RIGHT_CENTER,
            unit.label(value),
            FontId::proportional(11.0),
            ui.visuals().weak_text_color(),
        );
    }
    for (time, align) in [(start, Align2::LEFT_TOP), (end, Align2::RIGHT_TOP)] {
        if start == end && align == Align2::RIGHT_TOP {
            continue;
        }
        painter.text(
            Pos2::new(
                if align == Align2::RIGHT_TOP { rect.right() } else { rect.left() },
                rect.bottom() + 4.0,
            ),
            align,
            date(time),
            FontId::proportional(11.0),
            ui.visuals().weak_text_color(),
        );
    }
    for (index, (_, values)) in series.iter().enumerate() {
        let mut prior = None;
        for (time, value) in values {
            let point = value.map(|value| Pos2::new(x(*time), y(value)));
            if let Some(point) = point {
                if let Some(prior) = prior {
                    painter.line_segment([prior, point], Stroke::new(1.5, COLORS[index]));
                } else {
                    painter.circle_filled(point, 2.0, COLORS[index]);
                }
            }
            prior = point;
        }
    }
    if let Some(time) = selected.filter(|time| (start..=end).contains(time)) {
        painter.vline(x(time), rect.y_range(), Stroke::new(1.0, ui.visuals().weak_text_color()));
    }
    let pointer = response
        .hover_pos()
        .or(response.interact_pointer_pos())
        .filter(|pos| rect.contains(*pos));
    let mut closest_time = None;
    if let Some(pointer) = pointer {
        let time = start + (((pointer.x - rect.left()) / rect.width()) as f64 * span) as i64;
        let mut values = Vec::new();
        for (index, (name, points)) in series.iter().enumerate() {
            if let Some(&(stamp, value)) =
                points.iter().min_by_key(|(stamp, _)| stamp.abs_diff(time))
            {
                closest_time.get_or_insert(stamp);
                if let Some(value) = value {
                    painter.circle_filled(Pos2::new(x(stamp), y(value)), 3.0, COLORS[index]);
                    values.push((*name, unit.label(value)));
                }
            }
        }
        egui::Tooltip::for_widget(&response)
            .at_pointer()
            .show(|ui| {
                if let Some(stamp) = closest_time {
                    ui.label(timestamp(stamp));
                }
                for (name, value) in &values {
                    ui.label(format!("{name} · {value}"));
                }
                if values.is_empty() {
                    ui.weak("Missing coverage");
                }
            });
    }
    ui.add_space(11.0);
    response.clicked().then_some(closest_time).flatten()
}

fn parse_date(value: &str) -> Option<i64> {
    let day = time::Date::parse(value.trim(), DATE).ok()?;
    let timestamp = day.midnight().assume_utc().unix_timestamp();
    (timestamp >= 0).then_some(timestamp * 1000)
}

fn current_time() -> i64 {
    OffsetDateTime::now_utc().unix_timestamp() * 1000
}

fn preset_start(days: i64, end: i64) -> i64 {
    if days == 0 { queries::day(end) } else { end - days * DAY }
}

fn date(value: i64) -> String {
    format_date(value, DATE)
}
fn timestamp(value: i64) -> String {
    format_date(value, TIME)
}

fn format_date(value: i64, format: &[time::format_description::BorrowedFormatItem<'_>]) -> String {
    OffsetDateTime::from_unix_timestamp(value.div_euclid(1000))
        .ok()
        .and_then(|date| date.format(format).ok())
        .unwrap_or_else(|| "Unknown date".into())
}
