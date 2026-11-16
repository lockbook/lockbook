//! Sign-up as a list of steps in the sidebar's column, beside a read-only
//! note about the open step. The list starts with the two steps every path
//! shares, Account and Look; choosing Create adds Account key. The open step
//! holds its controls under its row, a finished step folds to one line and a
//! waiting step sits in secondary ink. A new account lands in its welcome
//! note, a sign-in lands in its files with nothing open.

use egui::{Align, Align2, Area, Id, Layout, Order, Rect, UiBuilder, pos2, vec2};
use workspace_rs::font::slot::Slot;
use workspace_rs::style::motion::SURFACE_SECS;
use workspace_rs::style::swatches::swatches;
use workspace_rs::tab::markdown_editor::{MdLabel, MdLayout, text_column};
use workspace_rs::theme::palette_v2::Palette;
use workspace_rs::widgets::GlyphonLabel;

use crate::components::domain::sidebar_resize::WIDTH_DEFAULT;
use crate::components::{
    Button, FG_HOVER, FG_PRESS, Field, Radius, SheetFooterOpts, Space, Spacer, Theme, TypeRole,
    control_height, phosphor, phosphor_font_id, phosphor_ui_font_id, plate_content_shadow,
    segmented, sense_click, sheet_footer, shortcut_cmd_i, shortcut_cmd_n, shortcut_return,
    ui_width, with_pad_fit,
};
use workspace_rs::style::{CHROME_BAND_H, overlay_scroll, with_overlay_scroll};

use super::ShellApp;
use super::action::Action as A;
use super::action::{Action, Modal, OnboardLookup, OnboardMode};
use super::apply_onboard::{
    display_server_host, onboard_server_editing_key, onboard_server_snap_key,
    uname_check_matches_core,
};
use super::titlebar::HEADER_H;

/// The note's heading, above a section per step reached.
const NOTE_HEADING: &str = "# Welcome to Lockbook";

/// One block the note has written. Going forward appends one; going back
/// takes off the blocks after the reopened step. A block's text never
/// changes, only its task box checking off.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Entry {
    step: Step,
    /// Starts with a task box; the opening line points at the list instead.
    task: bool,
    body: &'static str,
}

/// The block the open step writes. Account writes one before the choice and
/// another after it.
fn entry(step: Step, mode: OnboardMode, signing_in: bool) -> Entry {
    let paste = "Type or paste your 24 word phrase separated by commas, spaces, or both.";
    let (task, body) = match step {
        Step::Account => match mode {
            OnboardMode::Choice => (false, "← Create an account, or sign in, to get started."),
            OnboardMode::Import => (true, paste),
            _ if signing_in => (true, paste),
            _ => (true, "Lockbook doesn't ask for an email or a password, just a username."),
        },
        Step::Look => (true, "Pick a color and a font for your notes."),
        Step::Key => (
            true,
            "Save your account key.\n\n\
Your secret key serves as your password and is presented as a 24-word phrase or as a QR code to sign you in on a mobile device.\n\n\
Your key comes with two rules:\n\n\
1. **Keep it secret**: Anyone who gains access to your key gains access to your account. They can sign in as you, read and modify your files, and delete your account.\n\
2. **Don't lose it**: Your key is required to access your files, so without it, your files are forever lost. The easy way to make a backup is to sign in on another device.",
        ),
        Step::Complete if signing_in => (true, "Your notes are synced and ready."),
        Step::Complete => (true, "Write your first note."),
    };
    Entry { step, task, body }
}

fn entry_md(e: Entry, done: bool) -> String {
    let rule = if e.step == Step::Complete { "---\n\n" } else { "" };
    let check = match (e.task, done) {
        (false, _) => "",
        (true, true) => "- [x] ",
        (true, false) => "- [ ] ",
    };
    format!("{rule}{check}{}", e.body)
}

/// The heading, then the blocks; a block that is only a task line shares one
/// list with the next.
fn note_md(entries: &[Entry], done: impl Fn(Entry) -> bool) -> String {
    let mut md = NOTE_HEADING.to_owned();
    let mut sep = "\n\n";
    for &e in entries {
        md.push_str(sep);
        md.push_str(&entry_md(e, done(e)));
        sep = if e.task && !e.body.contains("\n\n") { "\n" } else { "\n\n" };
    }
    md.push('\n');
    md
}

/// The note beside the list, read-only. Each step appends a block as it
/// opens, the newest fading in, and checks its task off once done; going
/// back undoes what came after. It restyles live as Look changes.
pub(crate) struct OnboardNote {
    label: MdLabel,
    /// Measures the note above the newest block, where its fade starts.
    above: MdLabel,
    entries: Vec<Entry>,
    /// The markdown last drawn.
    shown: String,
    /// When the newest block was written, for its fade-in.
    appended_at: f64,
    /// Points scrolled down a pane too short for the note, and whether the
    /// newest block still needs bringing into view.
    scroll: f32,
    follow: bool,
    /// The note's height as last drawn, which bounds `scroll`.
    content: f32,
}

impl OnboardNote {
    fn new(ctx: &egui::Context) -> Self {
        Self {
            label: MdLabel::new(ctx.clone()),
            above: MdLabel::new(ctx.clone()),
            entries: vec![],
            shown: String::new(),
            appended_at: 0.0,
            scroll: 0.0,
            follow: false,
            content: 0.0,
        }
    }

    /// The note as a new account keeps it in `welcome.md`: the page as last
    /// drawn, so the editor opens on the same text in the same place.
    pub(crate) fn saved(&self) -> String {
        self.shown.clone()
    }
}

/// The steps a new account walks, in list order.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Step {
    Account,
    Look,
    Key,
    Complete,
}

impl Step {
    const ALL: [Step; 4] = [Step::Account, Step::Look, Step::Key, Step::Complete];
    /// The steps every path shares; Create adds the key.
    const SHARED: [Step; 3] = [Step::Account, Step::Look, Step::Complete];

    fn title(self) -> &'static str {
        match self {
            Step::Account => "Account",
            Step::Look => "Look",
            Step::Key => "Account key",
            Step::Complete => "Complete",
        }
    }

    /// A stable key for the step's animations.
    fn number(self) -> usize {
        Step::ALL.iter().position(|s| *s == self).unwrap() + 1
    }

    /// The step a mode has open.
    fn of(mode: OnboardMode) -> Step {
        match mode {
            OnboardMode::Choice | OnboardMode::Create | OnboardMode::Import => Step::Account,
            OnboardMode::Look => Step::Look,
            OnboardMode::Backup => Step::Key,
            OnboardMode::Complete => Step::Complete,
        }
    }
}

/// Where a step stands in the list.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Standing {
    Done,
    /// Done once the worker finishes; shown with a spinner.
    Finishing,
    Open,
    Waiting,
}

/// A snapshot of the onboard modal, so the drawing code borrows `app` freely.
struct View {
    mode: OnboardMode,
    uname: String,
    lookup: OnboardLookup,
    lookup_for: String,
    busy: bool,
    ready: bool,
    signing_in: bool,
    draft: Option<lb::model::account::Account>,
    err: Option<String>,
    api_url: String,
    /// The text family picked on the look step, from settings.
    font: Option<String>,
    /// The account's name once it is on this device.
    account_name: Option<String>,
}

impl View {
    fn field_dirty(&self) -> bool {
        let u = self.uname.trim();
        !u.is_empty() && !u.eq_ignore_ascii_case(&self.lookup_for)
    }

    fn can_create(&self) -> bool {
        // Create waits on a pending check, like Return; only a settled taken/invalid name blocks it.
        let rejected = !self.field_dirty()
            && matches!(self.lookup, OnboardLookup::Taken | OnboardLookup::Error(_));
        !self.uname.trim().is_empty() && !rejected
    }

    /// The rows the list shows: the two every path shares until Create is
    /// chosen, which adds the key; a sign-in never has one.
    fn steps(&self) -> &'static [Step] {
        match self.mode {
            OnboardMode::Choice | OnboardMode::Import => &Step::SHARED,
            _ if self.signing_in => &Step::SHARED,
            _ => &Step::ALL,
        }
    }

    fn standing(&self, step: Step) -> Standing {
        let open = Step::of(self.mode);
        if step == open {
            return Standing::Open;
        }
        let at = |s: Step| self.steps().iter().position(|&x| x == s);
        match step {
            Step::Account if self.ready => Standing::Done,
            Step::Account if self.busy => Standing::Finishing,
            // a step above the open one is behind the person: done; one off
            // this path (Account key, once signing in) is not
            _ if matches!((at(step), at(open)), (Some(s), Some(o)) if s < o) => Standing::Done,
            _ => Standing::Waiting,
        }
    }

    /// The step's place in the list this path shows.
    fn place(&self, step: Step) -> usize {
        self.steps()
            .iter()
            .position(|&s| s == step)
            .map_or(0, |i| i + 1)
    }

    /// What the Account row reads once it is done: the name chosen, or the
    /// name signed in as.
    fn account_summary(&self) -> String {
        if self.signing_in {
            self.account_name.clone().unwrap_or_default()
        } else {
            self.uname.trim().to_owned()
        }
    }
}

pub(crate) fn show_onboard(
    app: &mut ShellApp, ctx: &egui::Context, t: &Theme, queue: &mut Vec<Action>,
) {
    let account_name = app.session.ready().and_then(|r| {
        r.workspace
            .core
            .get_account()
            .ok()
            .map(|a| a.username.clone())
    });
    let view = match &app.modal {
        Some(Modal::Onboard {
            mode,
            uname,
            uname_lookup,
            uname_lookup_for,
            busy,
            ready,
            signing_in,
            err,
            api_url,
            draft,
            ..
        }) => View {
            mode: *mode,
            uname: uname.clone(),
            lookup: uname_lookup.clone(),
            lookup_for: uname_lookup_for.clone(),
            busy: *busy,
            ready: *ready,
            signing_in: *signing_in,
            draft: draft.clone(),
            err: err.clone(),
            api_url: api_url.clone(),
            font: app.settings.text_font.clone(),
            account_name,
        },
        _ => return,
    };
    // the note and the font rows draw in families read off the UI thread
    workspace_rs::font::poll(ctx);

    if view.mode == OnboardMode::Create
        && !view.busy
        && super::sheets::debounce_query(
            ctx,
            Id::unique("onboard_uname_verify_due"),
            Id::unique("onboard_uname_verify_q"),
            view.uname.trim(),
            &view.lookup_for,
        )
    {
        queue.push(A::OnboardVerifyUname);
    }

    // the app's own layout: the sidebar's column holds the list, the editor's pane the note
    let screen = ctx.content_rect();
    let side = Rect::from_min_max(screen.min, pos2(screen.left() + WIDTH_DEFAULT, screen.bottom()));
    let pane = Rect::from_min_max(pos2(side.right(), screen.top()), screen.max);
    let stage = pane.with_min_y(pane.top() + HEADER_H);
    let pad = Space::Md.pts();
    let column = Rect::from_min_max(
        pos2(side.left() + pad, side.top() + HEADER_H + Space::Sm.pts()),
        pos2(side.right() - pad, side.bottom() - pad),
    );

    Area::new(Id::unique("shell_onboard"))
        .order(Order::Foreground)
        .fixed_pos(screen.min)
        .show(ctx, |ui| {
            ui.painter()
                .rect_filled(side, 0.0, t.neutral_bg_secondary());
            ui.painter().rect_filled(pane, 0.0, t.neutral_bg());
            // nothing behind the sign-up list takes a click; the header band
            // stays free so the title bar can move the window
            ui.allocate_rect(
                screen.with_min_y(screen.top() + HEADER_H),
                egui::Sense::click_and_drag(),
            );

            ui.scope_builder(
                UiBuilder::new()
                    .max_rect(column)
                    .layout(Layout::top_down(Align::Min)),
                |ui| {
                    // a long step scrolls in the sidebar's column, as the file tree does
                    let id = Id::unique("onboard_steps");
                    with_overlay_scroll(ui, id, |ui| {
                        let out = egui::ScrollArea::vertical()
                            .id_salt(id)
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                ui.set_width(column.width());
                                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                                wordmark(ui, t);
                                ui.add(Spacer::new(Space::Lg));
                                steps(app, ui, t, queue, &view);
                            });
                        ((), out.state.clamped_offset().y, out.id)
                    });
                },
            );
            if view.mode == OnboardMode::Choice {
                server_caption_at_bottom(app, ui, t, column, &view.api_url);
            }

            stage_note(app, ui, t, stage, &view);
        });
}

fn wordmark(ui: &mut egui::Ui, t: &Theme) {
    ui.label(
        TypeRole::Title
            .rich("Lockbook")
            .strong()
            .color(t.neutral_fg()),
    );
    ui.add(Spacer::new(Space::Xs));
    ui.label(
        TypeRole::Body
            .rich("Notes that stay yours.")
            .color(t.neutral_fg_secondary()),
    );
}

/// The list. A row the path adds or drops fades in or out in its place.
fn steps(app: &mut ShellApp, ui: &mut egui::Ui, t: &Theme, queue: &mut Vec<Action>, view: &View) {
    for step in Step::ALL {
        let shown = view.steps().contains(&step);
        let shown_t = ui.ctx().animate_bool_with_time(
            Id::unique("onboard_step_shown").with(step.number()),
            shown,
            SURFACE_SECS,
        );
        if shown_t <= 0.0 {
            continue;
        }
        let standing = view.standing(step);
        ui.scope(|ui| {
            ui.set_opacity(shown_t);
            if step_row(ui, t, step, standing, view) {
                // a second look; the key waits, made already
                queue.push(A::OnboardSetMode(match step {
                    Step::Account => OnboardMode::Create,
                    Step::Look => OnboardMode::Look,
                    Step::Key | Step::Complete => OnboardMode::Backup,
                }));
            }
        });
        let open_t = ui.ctx().animate_bool_with_time(
            Id::unique("onboard_step_open").with(step.number()),
            standing == Standing::Open,
            SURFACE_SECS,
        );
        if standing == Standing::Open {
            step_body(app, ui, t, queue, view, step, open_t);
        }
        ui.add(Spacer::new(Space::Sm));
    }
}

/// Diameter of a step's marker: the line box of the row's text.
fn marker_d() -> f32 {
    TypeRole::Body.line_height()
}

/// Where a step's controls start: in from the marker, under the title.
fn body_indent() -> f32 {
    marker_d() + Space::Sm.pts()
}

/// One row of the list: marker, title, and the finished step's summary at the
/// right. A finished step that can reopen takes a click.
fn step_row(ui: &mut egui::Ui, t: &Theme, step: Step, standing: Standing, view: &View) -> bool {
    let h = control_height();
    let w = ui_width(ui);
    // a new account can go back to its name or its look until Done registers it
    let reopens = standing == Standing::Done
        && !view.busy
        && match step {
            Step::Account => !view.signing_in,
            Step::Look | Step::Key => true,
            Step::Complete => false,
        };
    let (rect, resp) = ui.allocate_exact_size(
        vec2(w, h),
        if reopens { sense_click() } else { egui::Sense::hover() },
    );
    let over = reopens && ui.ctx().rect_contains_pointer(ui.layer_id(), rect);
    if over {
        let wash = if resp.is_pointer_button_down_on() { FG_PRESS } else { FG_HOVER };
        ui.painter().rect_filled(
            rect.expand2(vec2(Space::Xs.pts(), 0.0)),
            Radius::Control.corner(),
            t.wash_toward_neutral_fg(t.neutral_bg_secondary(), wash),
        );
    }

    let d = marker_d();
    let marker = Rect::from_center_size(pos2(rect.left() + d / 2.0, rect.center().y), vec2(d, d));
    match standing {
        Standing::Done => {
            ui.painter().text(
                marker.center(),
                Align2::CENTER_CENTER,
                phosphor::CHECK_CIRCLE,
                phosphor_font_id(d),
                t.accent(),
            );
        }
        Standing::Finishing => {
            ui.scope_builder(UiBuilder::new().max_rect(marker), |ui| {
                ui.add(egui::Spinner::new().size(d).color(t.accent()));
            });
        }
        Standing::Open => {
            ui.painter()
                .circle_filled(marker.center(), d / 2.0, t.neutral_fg());
            ui.painter().text(
                marker.center(),
                Align2::CENTER_CENTER,
                view.place(step).to_string(),
                TypeRole::Caption.font_id(),
                t.neutral_bg(),
            );
        }
        Standing::Waiting => {
            ui.painter().text(
                marker.center(),
                Align2::CENTER_CENTER,
                phosphor::CIRCLE,
                phosphor_font_id(d),
                t.neutral_fg_secondary(),
            );
        }
    }

    let title_ink = match standing {
        Standing::Waiting => t.neutral_fg_secondary(),
        _ => t.neutral_fg(),
    };
    let title_x = rect.left() + body_indent();
    let title_g =
        ui.painter()
            .layout_no_wrap(step.title().to_owned(), TypeRole::Body.font_id(), title_ink);
    ui.painter().galley(
        pos2(title_x, rect.center().y - title_g.size().y / 2.0),
        title_g.clone(),
        title_ink,
    );

    // the username the finished Account step settled, at the right
    let summary = match (step, standing) {
        (Step::Account, Standing::Done | Standing::Finishing) => view.account_summary(),
        _ => String::new(),
    };
    if !summary.is_empty() {
        let right = rect.right() - Space::Sm.pts();
        let max_w = (right - title_x - title_g.size().x - Space::Sm.pts()).max(0.0);
        let g = ui.painter().layout_job(egui::text::LayoutJob {
            wrap: egui::text::TextWrapping::truncate_at_width(max_w),
            ..egui::text::LayoutJob::simple_singleline(
                summary,
                TypeRole::Mono.font_id(),
                t.neutral_fg_secondary(),
            )
        });
        ui.painter().galley(
            pos2(right - g.size().x, rect.center().y - g.size().y / 2.0),
            g,
            t.neutral_fg_secondary(),
        );
    }
    resp.clicked()
}

/// The open step's controls, indented under its title and fading in.
fn step_body(
    app: &mut ShellApp, ui: &mut egui::Ui, t: &Theme, queue: &mut Vec<Action>, view: &View,
    step: Step, open_t: f32,
) {
    let w = ui_width(ui);
    let indent = body_indent();
    let body = Rect::from_min_max(
        pos2(ui.cursor().left() + indent, ui.cursor().top()),
        pos2(ui.cursor().left() + w, ui.max_rect().bottom()),
    );
    // the scope claims what the body used from the column
    ui.scope_builder(
        UiBuilder::new()
            .max_rect(body)
            .layout(Layout::top_down(Align::Min)),
        |ui| {
            ui.set_width(body.width());
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            ui.set_opacity(open_t);
            ui.add(Spacer::new(Space::Xs));
            match step {
                Step::Account => match view.mode {
                    OnboardMode::Choice => body_choice(ui, t, queue, view),
                    OnboardMode::Create => body_username(app, ui, t, queue, view),
                    OnboardMode::Import => body_sign_in(app, ui, t, queue, view),
                    _ => {}
                },
                Step::Look => body_look(ui, t, queue, view),
                Step::Key => body_key(app, ui, t, queue, view),
                Step::Complete => body_complete(ui, t, queue, view),
            }
            ui.add(Spacer::new(Space::Xs));
        },
    );
    if open_t < 1.0 {
        ui.ctx().request_repaint();
    }
}

fn label(ui: &mut egui::Ui, t: &Theme, text: &str) {
    ui.label(TypeRole::Body.rich(text).color(t.neutral_fg_secondary()));
    ui.add(Spacer::new(Space::Xs));
}

fn lede(ui: &mut egui::Ui, t: &Theme, text: &str) {
    ui.label(TypeRole::Body.rich(text).color(t.neutral_fg()));
    ui.add(Spacer::new(Space::Sm));
}

fn aside(ui: &mut egui::Ui, t: &Theme, text: &str) {
    ui.label(TypeRole::Body.rich(text).color(t.neutral_fg_secondary()));
    ui.add(Spacer::new(Space::Sm));
}

/// One line of status under a field, its height held so nothing below jumps.
fn status_row(ui: &mut egui::Ui, status: &str, ink: egui::Color32) {
    let (slot, _) = ui.allocate_exact_size(
        vec2(ui_width(ui).max(1.0), TypeRole::Body.line_height()),
        egui::Sense::hover(),
    );
    if !status.is_empty() {
        let g = ui
            .painter()
            .layout_no_wrap(status.to_owned(), TypeRole::Body.font_id(), ink);
        ui.painter()
            .galley(pos2(slot.left(), slot.center().y - g.size().y / 2.0), g, ink);
    }
}

/// A full-width primary action.
fn wide_primary(ui: &mut egui::Ui, t: &Theme, text: &str, enabled: bool) -> bool {
    let w = ui_width(ui);
    Button::primary(t, text)
        .shortcut(shortcut_return())
        .min_width(w)
        .max_width(w)
        .enabled(enabled)
        .show(ui)
        .clicked()
}

/// The first step: Create account or Sign in. ⌘N / ⌘I are wired in `process_keys`.
fn body_choice(ui: &mut egui::Ui, t: &Theme, queue: &mut Vec<Action>, view: &View) {
    let w = ui_width(ui);
    if Button::primary(t, "Create account")
        .shortcut(shortcut_cmd_n())
        .min_width(w)
        .max_width(w)
        .enabled(!view.busy)
        .show(ui)
        .clicked()
    {
        queue.push(A::OnboardSetMode(OnboardMode::Create));
    }
    ui.add(Spacer::new(Space::Xs));
    if Button::quiet(t, "Sign in")
        .shortcut(shortcut_cmd_i())
        .min_width(w)
        .max_width(w)
        .enabled(!view.busy)
        .show(ui)
        .clicked()
    {
        queue.push(A::OnboardSetMode(OnboardMode::Import));
    }
}

fn body_username(
    app: &mut ShellApp, ui: &mut egui::Ui, t: &Theme, queue: &mut Vec<Action>, view: &View,
) {
    aside(ui, t, "People you share with see your username. It can’t change later.");
    let dirty = view.field_dirty();
    let (lead_icon, lead_ink) = match &view.lookup {
        OnboardLookup::Available => {
            (phosphor::USER_CHECK, if dirty { t.neutral_fg_secondary() } else { t.accent() })
        }
        OnboardLookup::Taken | OnboardLookup::Error(_) => {
            (phosphor::X_CIRCLE, if dirty { t.neutral_fg_secondary() } else { t.danger() })
        }
        OnboardLookup::Idle | OnboardLookup::Checking { .. } => {
            (phosphor::USER, t.neutral_fg_secondary())
        }
    };
    let edit_id = Id::unique("onboard_uname").with("edit");
    let need_focus = ui.ctx().data(|d| {
        d.get_temp::<bool>(Id::unique("onboard_uname_need_focus"))
            .unwrap_or(false)
    });
    {
        let Some(Modal::Onboard { uname, err, .. }) = &mut app.modal else {
            return;
        };
        let before = uname.clone();
        let _ = Field::new(t, uname)
            .hint("username")
            .id(Id::unique("onboard_uname"))
            .leading(lead_icon)
            .leading_ink(lead_ink)
            .clearable(true)
            .show(ui);
        if need_focus {
            ui.memory_mut(|m| m.request_focus(edit_id));
            if ui.memory(|m| m.has_focus(edit_id)) {
                ui.ctx().data_mut(|d| {
                    d.insert_temp(Id::unique("onboard_uname_need_focus"), false);
                });
            }
        }
        if *uname != before {
            *err = None;
        }
    }
    ui.add(Spacer::new(Space::Xs));
    let (status, ink) = if view.uname.trim().is_empty() {
        ("", t.neutral_fg_secondary())
    } else if dirty {
        ("Checking…", t.neutral_fg_secondary())
    } else {
        match &view.lookup {
            OnboardLookup::Available if uname_check_matches_core(&view.api_url) => {
                ("Available", t.accent())
            }
            OnboardLookup::Available => ("", t.neutral_fg_secondary()),
            OnboardLookup::Taken => ("Username taken", t.danger()),
            OnboardLookup::Error(e) => (e.as_str(), t.danger()),
            OnboardLookup::Checking { .. } => ("Checking…", t.neutral_fg_secondary()),
            OnboardLookup::Idle => ("", t.neutral_fg_secondary()),
        }
    };
    status_row(ui, status, ink);
    if let Some(e) = &view.err {
        ui.add(Spacer::new(Space::Xs));
        ui.label(TypeRole::Body.rich(e).color(t.danger()));
    }
    ui.add(Spacer::new(Space::Sm));
    let foot = sheet_footer(
        ui,
        t,
        "Create",
        SheetFooterOpts::default()
            .divider(false)
            .cancel_label("Back")
            .primary_enabled(!view.busy && view.can_create())
            .primary_shortcut(shortcut_return()),
    );
    if foot.cancel {
        queue.push(A::OnboardSetMode(OnboardMode::Choice));
    }
    if foot.primary {
        queue.push(A::OnboardSubmit { show_error: true });
    }
}

fn body_sign_in(
    app: &mut ShellApp, ui: &mut egui::Ui, t: &Theme, queue: &mut Vec<Action>, view: &View,
) {
    lede(ui, t, "Welcome back.");
    aside(
        ui,
        t,
        "Paste your account key: the 24 words from when you signed up. Any signed-in device shows them in Settings.",
    );
    let edit_id = Id::unique("onboard_account_key").with("edit");
    let need_focus = ui.ctx().data(|d| {
        d.get_temp::<bool>(Id::unique("onboard_account_key_need_focus"))
            .unwrap_or(false)
    });
    {
        let Some(Modal::Onboard { account_key, err, .. }) = &mut app.modal else {
            return;
        };
        let before = account_key.clone();
        let _ = Field::new(t, account_key)
            .hint("Account key")
            .id(Id::unique("onboard_account_key"))
            .password(true)
            .show(ui);
        if need_focus {
            ui.memory_mut(|m| m.request_focus(edit_id));
            if ui.memory(|m| m.has_focus(edit_id)) {
                ui.ctx().data_mut(|d| {
                    d.insert_temp(Id::unique("onboard_account_key_need_focus"), false);
                });
            }
        }
        if *account_key != before {
            *err = None;
        }
    }
    if let Some(e) = &view.err {
        ui.add(Spacer::new(Space::Xs));
        ui.label(TypeRole::Body.rich(e).color(t.danger()));
    }
    ui.add(Spacer::new(Space::Md));
    let secret = match &app.modal {
        Some(Modal::Onboard { account_key, .. }) => account_key.trim().to_owned(),
        _ => String::new(),
    };
    let can_import = !secret.is_empty();
    // Auto-submit on every change. Failures stay silent until the person hits Sign in.
    if can_import && !view.busy {
        let last = ui
            .ctx()
            .data(|d| d.get_temp::<String>(Id::unique("onboard_auto_submit_secret")));
        if last.as_deref() != Some(secret.as_str()) {
            ui.ctx().data_mut(|d| {
                d.insert_temp(Id::unique("onboard_auto_submit_secret"), secret);
            });
            queue.push(A::OnboardSubmit { show_error: false });
        }
    }
    let foot = sheet_footer(
        ui,
        t,
        "Sign in",
        SheetFooterOpts::default()
            .divider(false)
            .cancel_label("Back")
            .primary_enabled(!view.busy && can_import)
            .primary_shortcut(shortcut_return()),
    );
    if foot.cancel {
        queue.push(A::OnboardSetMode(OnboardMode::Choice));
    }
    if foot.primary {
        queue.push(A::OnboardSubmit { show_error: true });
    }
}

/// The accent dots, in the order Settings shows them.
const HUES: [Palette; 6] =
    [Palette::Red, Palette::Yellow, Palette::Green, Palette::Cyan, Palette::Blue, Palette::Magenta];

/// A color and a font; the note beside the list takes each on the spot.
/// Tab and ⇧Tab step through the colors, ↑ and ↓ through the fonts. The
/// account is made, or the notes synced, meanwhile.
fn body_look(ui: &mut egui::Ui, t: &Theme, queue: &mut Vec<Action>, view: &View) {
    aside(ui, t, "Pick a color and a font for your notes. Settings has both later too.");
    let slots: Vec<Slot> = Slot::shown().collect();
    let chosen = Slot::of(view.font.as_deref());
    let mut hue = HUES.iter().position(|&p| p == t.accent_hue);
    let font = slots.iter().position(|&s| Some(s) == chosen);
    let (color_step, font_step) = ui.input_mut(|i| {
        use egui::{Key, Modifiers};
        // ⇧Tab first: a plain-Tab match would also take it
        let back = i.consume_key(Modifiers::SHIFT, Key::Tab) as i32;
        let color = i.consume_key(Modifiers::NONE, Key::Tab) as i32 - back;
        let font = i.consume_key(Modifiers::NONE, Key::ArrowDown) as i32
            - i.consume_key(Modifiers::NONE, Key::ArrowUp) as i32;
        (color, font)
    });
    if color_step != 0 {
        queue.push(A::SetThemeAccent(HUES[cycle(hue, color_step, HUES.len())]));
    }
    if font_step != 0 {
        let slot = slots[cycle(font, font_step, slots.len())];
        queue.push(A::SetTextFont(slot.choice().map(str::to_owned)));
    }

    label(ui, t, "Color");
    let colors = HUES.map(|p| t.fg().get_color(p));
    if swatches(ui, t, &colors, &mut hue).changed() {
        if let Some(i) = hue {
            queue.push(A::SetThemeAccent(HUES[i]));
        }
    }
    ui.add(Spacer::new(Space::Sm));
    label(ui, t, "Font");
    for slot in slots {
        if font_row(ui, t, slot, chosen == Some(slot)) {
            queue.push(A::SetTextFont(slot.choice().map(str::to_owned)));
        }
    }
    ui.add(Spacer::new(Space::Sm));
    // the work runs while the person picks; the step waits for it only at the end
    let status = match (view.ready, view.signing_in) {
        (true, _) => "",
        (false, true) => "Syncing your notes…",
        (false, false) => "Creating your account…",
    };
    status_row(ui, status, t.neutral_fg_secondary());
    ui.add(Spacer::new(Space::Xs));
    let next = if view.signing_in { OnboardMode::Complete } else { OnboardMode::Backup };
    if wide_primary(ui, t, "Continue", true) {
        queue.push(A::OnboardSetMode(next));
    }
}

/// The end of the list: the note is the person's, and Start writing takes
/// them into their notes once the account is ready.
fn body_complete(ui: &mut egui::Ui, t: &Theme, queue: &mut Vec<Action>, view: &View) {
    aside(
        ui,
        t,
        if view.signing_in {
            "Your notes are on this device. Pick up where you left off."
        } else {
            "The note beside this list is your first. Keep it, change it, or start fresh."
        },
    );
    ui.add(Spacer::new(Space::Sm));
    match (&view.err, view.busy, view.signing_in, view.ready) {
        // as the username form shows its error
        (Some(e), false, ..) => {
            ui.label(TypeRole::Body.rich(e).color(t.danger()));
        }
        (_, true, false, _) => status_row(ui, "Creating your account…", t.neutral_fg_secondary()),
        (_, _, true, false) => status_row(ui, "Syncing your notes…", t.neutral_fg_secondary()),
        _ => status_row(ui, "", t.neutral_fg_secondary()),
    }
    ui.add(Spacer::new(Space::Xs));
    let can = !view.busy && (view.ready || !view.signing_in);
    if wide_primary(ui, t, "Start writing", can) {
        queue.push(A::OnboardFinish);
    }
}

/// The index `step` places away from `at`, wrapping; from none, the first
/// forward or the last back.
fn cycle(at: Option<usize>, step: i32, len: usize) -> usize {
    match at {
        Some(i) => (i as i32 + step).rem_euclid(len as i32) as usize,
        None if step > 0 => 0,
        None => len - 1,
    }
}

/// One font slot: its family's name in its own face, a check on the chosen one.
fn font_row(ui: &mut egui::Ui, t: &Theme, slot: Slot, chosen: bool) -> bool {
    let family = slot.drawn_family();
    let name = workspace_rs::font::system::shown_name(family);
    let h = control_height();
    let (rect, resp) = ui.allocate_exact_size(vec2(ui_width(ui), h), sense_click());
    let over = ui.ctx().rect_contains_pointer(ui.layer_id(), rect);
    if over {
        let wash = if resp.is_pointer_button_down_on() { FG_PRESS } else { FG_HOVER };
        ui.painter().rect_filled(
            rect,
            Radius::Control.corner(),
            t.wash_toward_neutral_fg(t.neutral_bg_secondary(), wash),
        );
    }
    let ink = t.neutral_fg();
    let pad_x = Space::Sm.pts();
    let text = Rect::from_min_max(
        pos2(rect.left() + pad_x, rect.top()),
        pos2(rect.right() - pad_x - h, rect.bottom()),
    );
    // the name shows in the interface font until its family is read
    match workspace_rs::font::load_family(ui.ctx(), family) {
        Some(adjust) => {
            let size = TypeRole::Body.size() * adjust;
            let label = GlyphonLabel::new(name, ink)
                .family(family)
                .font_size(size)
                .line_height(h)
                .max_width(text.width());
            ui.scope_builder(
                UiBuilder::new()
                    .max_rect(text)
                    .layout(Layout::left_to_right(Align::Center)),
                |ui| {
                    ui.add(label);
                },
            );
        }
        None => {
            ui.painter().text(
                pos2(text.left(), text.center().y),
                Align2::LEFT_CENTER,
                name,
                TypeRole::Body.font_id(),
                ink,
            );
        }
    }
    if chosen {
        ui.painter().text(
            pos2(rect.right() - pad_x - h / 2.0, rect.center().y),
            Align2::CENTER_CENTER,
            phosphor::CHECK,
            phosphor_ui_font_id(),
            t.accent(),
        );
    }
    resp.clicked()
}

fn key_view_id() -> Id {
    Id::unique("onboard_key_view")
}

/// Side of the sign-in QR's white plate.
const QR_SIDE: f32 = 180.0;

/// The key as words to keep or as a code for the phone, Tab switching
/// between them; Done ends the list.
fn body_key(
    app: &mut ShellApp, ui: &mut egui::Ui, t: &Theme, queue: &mut Vec<Action>, view: &View,
) {
    let mut which = ui
        .ctx()
        .data(|d| d.get_temp::<usize>(key_view_id()))
        .unwrap_or(0);
    // Tab and ⇧Tab switch between the words and the QR code
    let toggle = ui.input_mut(|i| {
        use egui::{Key, Modifiers};
        i.consume_key(Modifiers::NONE, Key::Tab) | i.consume_key(Modifiers::SHIFT, Key::Tab)
    });
    if toggle {
        which = 1 - which.min(1);
        ui.ctx().data_mut(|d| d.insert_temp(key_view_id(), which));
    }
    lede(
        ui,
        t,
        if which == 0 {
            "Sign in anywhere with these 24 words."
        } else {
            "Sign in anywhere with this QR code."
        },
    );
    aside(
        ui,
        t,
        "We can’t reset your key. Make a backup by signing in on another device, writing it down, or memorizing it.",
    );
    aside(
        ui,
        t,
        "We don’t recommend storing it in another notes app, as a photo, or with another cloud storage provider. These have weaker security guarantees.",
    );
    // centered on the column
    ui.allocate_ui_with_layout(
        vec2(ui_width(ui), control_height()),
        Layout::top_down(Align::Center),
        |ui| {
            if segmented(ui, t, &["Words", "QR code"], &mut which).changed() {
                ui.ctx().data_mut(|d| d.insert_temp(key_view_id(), which));
            }
        },
    );
    ui.add(Spacer::new(Space::Sm));

    let phrase = phrase(view);
    let phrase_ok = phrase.split_whitespace().count() == 24;
    match which {
        0 => phrase_plate(ui, t, &phrase),
        _ => qr_plate(app, ui, t),
    }
    ui.add(Spacer::new(Space::Md));

    let row_w = ui_width(ui);
    let row_h = control_height();
    let gap = Space::Sm;
    ui.horizontal(|ui| {
        ui.set_min_height(row_h);
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        let copy = Button::quiet(t, "Copy")
            .copy_feedback(Id::unique("shell_onboard_copy_phrase"))
            .height(row_h)
            .enabled(phrase_ok)
            .show(ui);
        if copy.clicked() {
            queue.push(A::CopyPhrase);
        }
        let copy_w = copy.rect.width();
        ui.add(Spacer::new(gap).fill_cross(row_h));
        let primary_max = (row_w - copy_w - gap.pts()).max(Space::Xl.pts() * 2.4);
        ui.allocate_ui_with_layout(
            vec2(primary_max, row_h),
            Layout::right_to_left(Align::Center),
            |ui| {
                ui.set_max_width(primary_max);
                if Button::primary(t, "Continue")
                    .shortcut(shortcut_return())
                    .height(row_h)
                    .max_width(primary_max)
                    .enabled(phrase_ok)
                    .show(ui)
                    .clicked()
                {
                    queue.push(A::OnboardSetMode(OnboardMode::Complete));
                }
            },
        );
    });
}

/// The sign-in QR on a white tile, with what to do with it, both centered
/// on the column.
fn qr_plate(app: &mut ShellApp, ui: &mut egui::Ui, t: &Theme) {
    ui.vertical_centered(|ui| qr_tile(app, ui, t));
}

fn qr_tile(app: &mut ShellApp, ui: &mut egui::Ui, t: &Theme) {
    let w = ui_width(ui);
    let side = QR_SIDE.min(w);
    let (qr, _) = ui.allocate_exact_size(vec2(side, side), egui::Sense::hover());
    ui.painter()
        .rect_filled(qr, Radius::Control.corner(), egui::Color32::WHITE);
    let png = app.account_qr.lock().ok().and_then(|g| match &*g {
        super::AccountQrCache::Ready(png) => Some(png.clone()),
        _ => None,
    });
    let inner = qr.shrink(Space::Sm.pts());
    match png {
        Some(png) => {
            ui.scope_builder(
                UiBuilder::new()
                    .max_rect(inner)
                    .layout(Layout::top_down(Align::Min)),
                |ui| super::settings_account::paint_account_qr_image(ui, &png, inner.width()),
            );
        }
        None => {
            ui.painter().text(
                inner.center(),
                Align2::CENTER_CENTER,
                "Preparing…",
                TypeRole::Body.font_id(),
                egui::Color32::BLACK,
            );
        }
    }
    ui.add(Spacer::new(Space::Lg));
    ui.label(
        TypeRole::Body
            .rich("Open Lockbook on your phone, choose Sign in, and scan this code.")
            .color(t.neutral_fg_secondary()),
    );
}

/// The 24 words: a new account's from the key it will register, a
/// signed-in one's from its core.
fn phrase(view: &View) -> String {
    view.draft
        .as_ref()
        .and_then(|d| d.get_phrase().ok())
        .map(|w| w.join(" "))
        .unwrap_or_default()
}

/// The 24 words on their own plate.
fn phrase_plate(ui: &mut egui::Ui, t: &Theme, phrase: &str) {
    plate_content_shadow(ui, t.neutral_bg(), egui::Shadow::NONE, Radius::Control.corner(), |ui| {
        ui.set_width(ui_width(ui));
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        with_pad_fit(ui, Space::Sm, |ui| {
            phrase_columns(ui, t, phrase);
        });
    });
}

fn phrase_columns(ui: &mut egui::Ui, t: &Theme, phrase: &str) {
    let words: Vec<&str> = phrase.split_whitespace().collect();
    if words.len() != 24 {
        ui.label(
            TypeRole::Mono
                .rich(if phrase.is_empty() { "Preparing your account key…" } else { phrase })
                .color(t.neutral_fg()),
        );
        return;
    }
    let col_w = (ui_width(ui) / 2.0).max(1.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        ui.vertical(|ui| {
            ui.set_width(col_w);
            for (i, word) in words.iter().take(12).enumerate() {
                phrase_word(ui, t, i + 1, word);
            }
        });
        ui.vertical(|ui| {
            ui.set_width(col_w);
            for (i, word) in words.iter().skip(12).enumerate() {
                phrase_word(ui, t, i + 13, word);
            }
        });
    });
}

fn phrase_word(ui: &mut egui::Ui, t: &Theme, n: usize, word: &str) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = vec2(Space::Xs.pts(), 0.0);
        ui.label(TypeRole::Mono.rich(format!("{n:>2}.")).color(t.accent()));
        ui.label(TypeRole::Mono.rich(word).color(t.neutral_fg()));
    });
}

/// Where the open note's text will sit in the editor's pane: the editor's
/// column at a new account's text width, below the toolbar band plus the
/// editor's first-row padding. Returns the origin and the column width.
fn note_column(stage: Rect) -> (egui::Pos2, f32) {
    let (left, width) = text_column(
        stage.width(),
        MdLayout::desktop().margin,
        workspace_rs::workspace::default_column_width(),
    );
    (pos2(stage.left() + left, stage.top() + CHROME_BAND_H + Space::Md.pts()), width.max(1.0))
}

/// The note on the pane, drawn where the editor will draw it.
fn stage_note(app: &mut ShellApp, ui: &mut egui::Ui, t: &Theme, stage: Rect, view: &View) {
    let note = app
        .onboard_note
        .get_or_insert_with(|| OnboardNote::new(ui.ctx()));
    let open = entry(Step::of(view.mode), view.mode, view.signing_in);
    match note.entries.iter().position(|&e| e == open) {
        Some(i) => note.entries.truncate(i + 1),
        None => {
            note.entries.push(open);
            note.appended_at = ui.input(|i| i.time);
            note.follow = true;
        }
    }

    let (top, width) = note_column(stage);
    let over = ui.rect_contains_pointer(stage);
    let wheel = if over { ui.input(|i| i.smooth_scroll_delta.y) } else { 0.0 };
    // clamped to last frame's extent before drawing, so the wheel can't push
    // the note past either end for a frame
    note.scroll = (note.scroll - wheel).clamp(0.0, (note.content - stage.height()).max(0.0));
    let origin = top - vec2(0.0, note.scroll);
    let mut child = ui.new_child(UiBuilder::new().max_rect(stage));
    child.set_clip_rect(stage.intersect(ui.clip_rect()));

    let done = |e: Entry| view.standing(e.step) == Standing::Done;
    note.shown = note_md(&note.entries, done);
    let (areas, rect) = note
        .label
        .paint_at(&mut child, "onboard_note", &note.shown, origin, width);

    let visible = stage.intersect(ui.clip_rect());
    if !areas.is_empty() && visible.is_positive() {
        child
            .painter()
            .add(egui_wgpu_renderer::egui_wgpu::Callback::new_paint_callback(
                visible,
                workspace_rs::GlyphonRendererCallback::new(areas),
            ));
    }

    // a pane too short scrolls; a new block scrolls into view
    // only text the pane cuts off scrolls, as the editor would show it
    let content = rect.bottom() + note.scroll - stage.top();
    note.content = content;
    let max = (content - stage.height()).max(0.0);
    if std::mem::take(&mut note.follow) && note.scroll < max {
        note.scroll = max;
        ui.ctx().request_repaint();
    }
    note.scroll = note.scroll.clamp(0.0, max);
    overlay_scroll::scroller_bar(
        ui,
        Id::unique("onboard_note_scroll"),
        stage,
        content,
        &mut note.scroll,
        wheel == 0.0,
    );

    // a scrim of the page over the newest block while it fades in
    let newest = note.entries.len() - 1;
    let since = (ui.input(|i| i.time) - note.appended_at) as f32;
    let shown = egui::emath::easing::exponential_out((since / SURFACE_SECS).clamp(0.0, 1.0));
    if shown < 1.0 {
        ui.ctx().request_repaint();
        let above = note
            .above
            .height(&note_md(&note.entries[..newest], done), width);
        let band = rect.with_min_y(origin.y + above).expand(Space::Xs.pts());
        child
            .painter()
            .rect_filled(band, 0.0, t.neutral_bg().gamma_multiply(1.0 - shown));
    }
}

fn onboard_server_edit_id() -> Id {
    Id::unique("onboard_server").with("edit")
}

fn onboard_server_need_focus_key() -> Id {
    Id::unique("onboard_server_need_focus")
}

/// The server's host at the foot of the column while the account is chosen.
fn server_caption_at_bottom(
    app: &mut ShellApp, ui: &mut egui::Ui, t: &Theme, column: Rect, api_url: &str,
) {
    let h = control_height();
    let foot = Rect::from_min_max(pos2(column.left(), column.bottom() - h), column.max);
    ui.scope_builder(
        UiBuilder::new()
            .max_rect(foot)
            .layout(Layout::top_down(Align::Min)),
        |ui| {
            ui.set_width(foot.width());
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            onboard_server_row(app, ui, t, api_url);
        },
    );
}

/// Rest: hostname caption. Click: field. Blur / Enter commit; Esc reverts; × defaults.
fn onboard_server_row(app: &mut ShellApp, ui: &mut egui::Ui, t: &Theme, api_url: &str) {
    let editing_key = onboard_server_editing_key();
    let snap_key = onboard_server_snap_key();
    let mut editing = ui
        .ctx()
        .data(|d| d.get_temp::<bool>(editing_key))
        .unwrap_or(false);
    let mut need_focus = ui
        .ctx()
        .data(|d| d.get_temp::<bool>(onboard_server_need_focus_key()))
        .unwrap_or(false);

    if editing {
        {
            let Some(Modal::Onboard { api_url, .. }) = &mut app.modal else {
                return;
            };
            let _ = Field::new(t, api_url)
                .hint(lb::DEFAULT_API_LOCATION)
                .id(Id::unique("onboard_server"))
                .clearable(true)
                .sticky(false)
                .show(ui);
            if need_focus {
                ui.memory_mut(|m| m.request_focus(onboard_server_edit_id()));
                if ui.memory(|m| m.has_focus(onboard_server_edit_id())) {
                    need_focus = false;
                }
            }
        }
        let focused = ui.memory(|m| m.has_focus(onboard_server_edit_id()));
        if !need_focus && !focused {
            editing = false;
        }
    } else if onboard_server_caption(ui, t, &display_server_host(api_url)) {
        editing = true;
        need_focus = true;
        ui.ctx()
            .data_mut(|d| d.insert_temp(snap_key, api_url.to_owned()));
    }

    ui.ctx().data_mut(|d| {
        d.insert_temp(editing_key, editing);
        d.insert_temp(onboard_server_need_focus_key(), need_focus);
    });
}

fn onboard_server_caption(ui: &mut egui::Ui, t: &Theme, host: &str) -> bool {
    let rest = t.neutral_fg_secondary();
    let g = ui
        .painter()
        .layout_no_wrap(host.to_owned(), TypeRole::Body.font_id(), rest);
    let h = control_height();
    let w = ui_width(ui).max(1.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(w, h), sense_click());
    let over = ui.ctx().rect_contains_pointer(ui.layer_id(), rect);
    let ink = if over { t.neutral_fg() } else { rest };
    ui.painter()
        .galley(pos2(rect.left(), rect.center().y - g.size().y / 2.0), g, ink);
    resp.clicked()
}
