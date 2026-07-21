//! Lightweight shell modals for context-menu actions that need extra input:
//! Share (username + access), confirm sheets (Delete, Dismiss share, …),
//! and the folder destination picker (Move / Add-to-files). Confirm sheets
//! share canvas `sheet_panel` chrome.

use std::collections::HashSet;

use egui::{
    Align, Area, CornerRadius, Frame, Id, Key, Layout, Margin, Modifiers, Order, RichText,
    ScrollArea, Stroke, Ui, vec2,
};
use lb::Uuid;
use lb::model::file::ShareMode;
use workspace_rs::file_cache::FilesExt;
use workspace_rs::show::InputStateExt;

use crate::theme::icons;
use crate::theme::tokens::Tokens;
use crate::widgets::button::Button;
use crate::widgets::search_field;
use crate::widgets::tree_chrome::{self, FolderRowVisual};

// ── Shared sheet chrome ─────────────────────────────────────────────────────

const SHEET_RADIUS: u8 = 10;
const SHEET_PAD: i8 = 16;
/// Space around the footer hairline (Share).
const SHEET_FOOTER_GAP: f32 = 10.0;

/// Action (left, strong) + subject (right, muted truncate). Same for Share /
/// Move / Delete so every sheet declares *what* it’s acting on. Subject is
/// constrained to remaining width so long names never run into the action.
fn sheet_header(ui: &mut Ui, t: &Tokens, action: &str, subject: &str) {
    let full_w = ui.available_width();
    let action_galley = ui.painter().layout_no_wrap(
        action.into(),
        egui::FontId::proportional(16.0),
        t.fg(),
    );
    let action_w = action_galley.size().x;
    let gap = 12.0;
    let subject_w = (full_w - action_w - gap).max(48.0);

    ui.horizontal(|ui| {
        ui.set_min_height(action_galley.size().y.max(20.0));
        ui.label(RichText::new(action).size(16.0).strong().color(t.fg()));
        ui.add_space(gap);
        // Fixed remaining band — Label::truncate needs a max width.
        ui.allocate_ui_with_layout(
            vec2(subject_w, ui.min_rect().height().max(20.0)),
            Layout::right_to_left(Align::Center),
            |ui| {
                ui.set_max_width(subject_w);
                ui.add(
                    egui::Label::new(RichText::new(subject).size(13.0).color(t.text_muted()))
                        .truncate(),
                );
            },
        );
    });
}

/// One name, or `N items` for multi-select headers / primary labels.
fn sheet_subject(names: &[String]) -> String {
    match names {
        [] => "item".into(),
        [one] => one.clone(),
        xs => format!("{} items", xs.len()),
    }
}

fn names_for_ids(files: &impl FilesExt, ids: &[Uuid]) -> Vec<String> {
    ids.iter()
        .filter_map(|id| files.get_by_id(*id).map(|f| f.name.clone()))
        .collect()
}

/// Cancel badge — Escape.
fn shortcut_esc() -> &'static str {
    "esc"
}

/// Primary-commit badge — ⌘↩ / Ctrl+↵.
fn shortcut_return() -> &'static str {
    if cfg!(target_os = "macos") {
        "⌘↩"
    } else {
        "Ctrl+↵"
    }
}

/// ⌘1–9 → index `0..8`, or `None`. Shared by Share / Move list shortcuts.
fn consume_cmd_digit(i: &mut egui::InputState) -> Option<usize> {
    const NUMS: [Key; 9] = [
        Key::Num1,
        Key::Num2,
        Key::Num3,
        Key::Num4,
        Key::Num5,
        Key::Num6,
        Key::Num7,
        Key::Num8,
        Key::Num9,
    ];
    for (n, key) in NUMS.iter().enumerate() {
        if i.consume_key_exact(Modifiers::COMMAND, *key) {
            return Some(n);
        }
    }
    None
}

/// Full-window modal scrim (Share / Move / Delete). Sibling **Middle** layer
/// behind the sheet — never nest the sheet inside this Area.
const MODAL_DIM_ALPHA: u8 = 40;

/// Paint the shared modal dim. Returns `true` if the user primary-clicked the
/// scrim (and not the sheet layer).
pub fn show_modal_dim(
    ctx: &egui::Context, dim_id: Id, sheet_layer: egui::LayerId,
) -> bool {
    let screen = ctx.screen_rect();
    let mut outside = false;
    Area::new(dim_id)
        .order(Order::Middle)
        .fixed_pos(screen.min)
        .default_size(screen.size())
        .fade_in(false)
        .sense(egui::Sense::click())
        .show(ctx, |ui| {
            let (rect, resp) = ui.allocate_exact_size(screen.size(), egui::Sense::click());
            ui.painter()
                .rect_filled(rect, 0.0, egui::Color32::from_black_alpha(MODAL_DIM_ALPHA));
            if resp.clicked() {
                let on_sheet = ctx
                    .pointer_interact_pos()
                    .is_some_and(|pos| ctx.layer_id_at(pos) == Some(sheet_layer));
                if !on_sheet {
                    outside = true;
                }
            }
        });
    outside
}

/// Canvas panel used by Share / Move / Delete.
fn sheet_panel(ui: &mut Ui, t: &Tokens, content_w: f32, add: impl FnOnce(&mut Ui)) {
    Frame::new()
        .inner_margin(Margin::same(SHEET_PAD))
        .corner_radius(CornerRadius::same(SHEET_RADIUS))
        .fill(t.canvas())
        .stroke(Stroke::new(1.0, t.line()))
        .show(ui, |ui| {
            ui.set_width(content_w);
            add(ui);
        });
}

/// Result of the standard Cancel | Primary footer row.
struct SheetFooterClick {
    cancel: bool,
    primary: bool,
}

/// Cancel (esc) left, primary (⌘↩ when enabled) right. Optional hairline above.
/// Primary is allocated the remaining width (with a floor) so a long label
/// truncates instead of overlapping Cancel.
fn sheet_footer(
    ui: &mut Ui,
    t: &Tokens,
    primary_label: &str,
    primary_enabled: bool,
    opts: SheetFooterOpts,
) -> SheetFooterClick {
    if opts.divider {
        ui.add_space(SHEET_FOOTER_GAP);
        let (div, _) =
            ui.allocate_exact_size(vec2(opts.divider_w, 1.0), egui::Sense::hover());
        ui.painter()
            .hline(div.x_range(), div.center().y, Stroke::new(1.0, t.line()));
        ui.add_space(SHEET_FOOTER_GAP);
    }

    let mut click = SheetFooterClick {
        cancel: false,
        primary: false,
    };
    let row_w = ui.available_width();
    let row_h = opts.btn_height.unwrap_or(32.0);
    ui.horizontal(|ui| {
        ui.set_min_height(row_h);
        let mut cancel = Button::secondary(t, "Cancel").shortcut(shortcut_esc());
        if let Some(h) = opts.btn_height {
            cancel = cancel.height(h);
        }
        // Measure cancel before paint so primary can claim the rest.
        let cancel_w = {
            let font = egui::FontId::proportional(14.0);
            let g = ui.painter().layout_no_wrap("Cancel".into(), font.clone(), t.fg());
            let sc = ui
                .painter()
                .layout_no_wrap(shortcut_esc().into(), egui::FontId::proportional(12.0), t.fg());
            g.size().x + 14.0 * 2.0 + 8.0 + sc.size().x + 4.0
        };
        let gap = 12.0;
        let primary_max = (row_w - cancel_w - gap).max(96.0);

        if cancel.show(ui).clicked() {
            click.cancel = true;
        }
        ui.add_space(gap);
        ui.allocate_ui_with_layout(
            vec2(primary_max, row_h),
            Layout::right_to_left(Align::Center),
            |ui| {
                ui.set_max_width(primary_max);
                let mut primary = if opts.danger {
                    Button::secondary(t, primary_label).danger()
                } else {
                    Button::primary(t, primary_label)
                }
                .enabled(primary_enabled)
                .max_width(primary_max);
                if primary_enabled {
                    primary = primary.shortcut(shortcut_return());
                }
                if let Some(h) = opts.btn_height {
                    primary = primary.height(h);
                }
                if primary.show(ui).clicked() {
                    click.primary = true;
                }
            },
        );
    });
    click
}

struct SheetFooterOpts {
    /// Match search-field height (Share).
    btn_height: Option<f32>,
    danger: bool,
    divider: bool,
    divider_w: f32,
}

impl Default for SheetFooterOpts {
    fn default() -> Self {
        Self {
            btn_height: None,
            danger: false,
            divider: false,
            divider_w: 0.0,
        }
    }
}

// ── Share (batch staging + collaborator suggestions) ───────────────────────

/// Match quality for people-picker typeahead (lower = better).
/// Prior art: prefix-first (Docs / Graph / Slack), then contains, then light
/// subsequence for “jsm” → “jsmith”-style skips. No network — known pool only.
fn match_quality(username: &str, q: &str) -> Option<u8> {
    let n = username.to_lowercase();
    if n == q {
        return Some(0);
    }
    if n.starts_with(q) {
        return Some(1);
    }
    if n.contains(q) {
        return Some(2);
    }
    if is_subsequence(&n, q) {
        return Some(3);
    }
    None
}

/// True if every char of `q` appears in order in `name` (not necessarily contiguous).
fn is_subsequence(name: &str, q: &str) -> bool {
    let mut it = name.chars();
    for qc in q.chars() {
        loop {
            match it.next() {
                Some(nc) if nc == qc => break,
                Some(_) => continue,
                None => return false,
            }
        }
    }
    true
}

/// One person staged for share but not yet written to lb.
#[derive(Clone, Debug)]
pub struct PendingShare {
    pub username: String,
    pub mode: ShareMode,
}

/// Visual weight for a hit row (icon + subtitle color).
#[derive(Clone, Copy, Debug)]
enum ShareHitTone {
    /// In-progress lookup.
    Progress,
    /// Account exists; can stage.
    Ok,
    /// No account.
    Bad,
    /// Offline / other failure.
    Caution,
}

#[derive(Clone, Debug)]
struct ShareHit {
    username: String,
    /// Single status line under the name (keep rows to two lines).
    status: String,
    icon: &'static str,
    tone: ShareHitTone,
    /// When false, row explains membership (owner / already on) — don’t stage.
    can_stage: bool,
}

/// Async username → key lookup for free-typed “new person” hits.
/// Think of it like checking if a username is free to add — not a share yet.
#[derive(Clone, Debug, Default)]
pub enum UsernameLookup {
    #[default]
    Idle,
    /// Waiting for typing to settle (no network yet).
    Debouncing {
        query: String,
    },
    /// `get_public_key` in flight.
    Checking {
        query: String,
    },
    /// Account exists (cache or server).
    Found {
        query: String,
    },
    /// Server said user does not exist.
    NotFound {
        query: String,
    },
    /// Network unreachable.
    Offline {
        query: String,
    },
    Failed {
        query: String,
        msg: String,
    },
}

impl UsernameLookup {
    fn query(&self) -> Option<&str> {
        match self {
            Self::Idle => None,
            Self::Debouncing { query }
            | Self::Checking { query }
            | Self::Found { query }
            | Self::NotFound { query }
            | Self::Offline { query }
            | Self::Failed { query, .. } => Some(query.as_str()),
        }
    }

    fn matches_q(&self, q: &str) -> bool {
        self.query().is_some_and(|cq| cq.eq_ignore_ascii_case(q))
    }

    /// Terminal answer for this query — do not re-fetch.
    fn is_settled_for(&self, q: &str) -> bool {
        self.matches_q(q)
            && matches!(
                self,
                Self::Found { .. }
                    | Self::NotFound { .. }
                    | Self::Offline { .. }
                    | Self::Failed { .. }
            )
    }

    /// Worker already launched for this query.
    fn is_inflight_for(&self, q: &str) -> bool {
        matches!(self, Self::Checking { query } if query.eq_ignore_ascii_case(q))
    }

    /// Icon + tone + one status line for the active-search row.
    fn query_hit_visual(&self, q: &str) -> (&'static str, ShareHitTone, String) {
        match self {
            Self::Debouncing { query } | Self::Checking { query }
                if query.eq_ignore_ascii_case(q) =>
            {
                (icons::SPINNER_GAP, ShareHitTone::Progress, "Checking…".into())
            }
            Self::Found { query } if query.eq_ignore_ascii_case(q) => {
                (icons::CHECK_CIRCLE, ShareHitTone::Ok, "User found".into())
            }
            Self::NotFound { query } if query.eq_ignore_ascii_case(q) => {
                (icons::X_CIRCLE, ShareHitTone::Bad, "User not found".into())
            }
            Self::Offline { query } if query.eq_ignore_ascii_case(q) => {
                (icons::WARNING_CIRCLE, ShareHitTone::Caution, "Offline".into())
            }
            Self::Failed { query, .. } if query.eq_ignore_ascii_case(q) => {
                (icons::WARNING_CIRCLE, ShareHitTone::Caution, "Couldn’t check".into())
            }
            _ => (icons::SPINNER_GAP, ShareHitTone::Progress, "Checking…".into()),
        }
    }

    /// Whether we may stage this free-typed name right now.
    fn allows_stage(&self, q: &str) -> Result<(), String> {
        match self {
            Self::Found { query } if query.eq_ignore_ascii_case(q) => Ok(()),
            Self::NotFound { query } if query.eq_ignore_ascii_case(q) => {
                Err("No account with that username.".into())
            }
            Self::Offline { query } if query.eq_ignore_ascii_case(q) => {
                Err("Can't reach the server to verify that username.".into())
            }
            Self::Failed { query, msg } if query.eq_ignore_ascii_case(q) => {
                if msg.is_empty() {
                    Err("Couldn't verify that username.".into())
                } else {
                    Err(format!("Couldn't verify that username: {msg}"))
                }
            }
            Self::Debouncing { query } | Self::Checking { query }
                if query.eq_ignore_ascii_case(q) =>
            {
                Err("Still checking that username…".into())
            }
            _ => Err("Still checking that username…".into()),
        }
    }
}

type LookupMsg = (u64, String, UsernameLookup);

pub struct ShareModal {
    pub id: Uuid,
    /// Search / compose field.
    pub query: String,
    /// Default access for the next staged person.
    pub default_mode: ShareMode,
    /// Staged adds — applied only on Submit.
    pub pending: Vec<PendingShare>,
    pub error: String,
    /// Request focus into the username field once (after click / type-to-invite).
    pub focus_user: bool,
    /// Recent collabs not already on this file (pre-capped at open).
    pub suggested: Vec<String>,
    pub me: String,
    /// Keyboard highlight among suggestion rows under the composer (`None` = none).
    pub selected: Option<String>,
    /// Proactive get_public_key for free-typed names.
    pub lookup: UsernameLookup,
    lookup_gen: u64,
    /// Time of last query edit (debounce remote lookup).
    query_edit_at: f64,
    last_seen_query: String,
    lookup_tx: std::sync::mpsc::Sender<LookupMsg>,
    lookup_rx: std::sync::mpsc::Receiver<LookupMsg>,
}

/// Host id for the username field; the actual edit id is `share_username_id()`.
fn share_username_host() -> Id {
    Id::new("lb_share_username")
}

/// Stable edit id — also requested early so the editor loses focus before
/// `workspace.show` (see `hold_modal_focus`). Matches `search_field`’s `.with("edit")`.
pub fn share_username_id() -> Id {
    share_username_host().with("edit")
}

impl ShareModal {
    pub fn new(id: Uuid, me: String, suggested: Vec<String>) -> Self {
        let (lookup_tx, lookup_rx) = std::sync::mpsc::channel();
        Self {
            id,
            query: String::new(),
            default_mode: ShareMode::Write,
            pending: Vec::new(),
            error: String::new(),
            // Docs-style: invite field is primary chrome — focus on open.
            focus_user: true,
            suggested,
            me,
            selected: None,
            lookup: UsernameLookup::Idle,
            lookup_gen: 0,
            query_edit_at: 0.0,
            last_seen_query: String::new(),
            lookup_tx,
            lookup_rx,
        }
    }

    /// File owner username (collaborator with top-of-list treatment).
    fn file_owner(&self, files: &impl FilesExt) -> Option<String> {
        files.get_by_id(self.id).map(|f| f.owner.clone())
    }

    fn is_file_owner(&self, files: &impl FilesExt, name: &str) -> bool {
        self.file_owner(files)
            .is_some_and(|o| o.eq_ignore_ascii_case(name))
    }

    /// Owner, existing sharee, or staged pending — already in the membership set.
    fn already_on_file_or_pending(&self, files: &impl FilesExt, name: &str) -> bool {
        let n = name.to_lowercase();
        if self.pending.iter().any(|p| p.username.eq_ignore_ascii_case(&n)) {
            return true;
        }
        files.get_by_id(self.id).is_some_and(|f| {
            f.owner.eq_ignore_ascii_case(&n)
                || f.shares
                    .iter()
                    .any(|s| s.shared_with.eq_ignore_ascii_case(&n))
        })
    }

    fn is_suggested_exact(&self, name: &str) -> bool {
        self.suggested.iter().any(|u| u.eq_ignore_ascii_case(name))
    }

    /// Idle typeahead (empty query): few recent people — browse, not search.
    const IDLE_CAP: usize = 3;
    /// While typing: more hits from the full known-collaborator pool.
    const SEARCH_CAP: usize = 8;

    /// Typeahead under the invite field.
    ///
    /// - Empty query → top recent **not** already reaching this file via an
    ///   ancestor share (idle “suggestions” only)
    /// - Non-empty → search full known pool **including** inherited-access
    ///   people (still stageable for a direct share), ranked by match quality
    fn suggested_nav(&self, inherited: &std::collections::HashSet<String>) -> Vec<String> {
        let q = self.query.trim().to_lowercase();
        let available: Vec<&String> = self
            .suggested
            .iter()
            .filter(|u| {
                !self
                    .pending
                    .iter()
                    .any(|p| p.username.eq_ignore_ascii_case(u))
            })
            .collect();

        if q.is_empty() {
            return available
                .into_iter()
                .filter(|u| !inherited.contains(&u.to_lowercase()))
                .take(Self::IDLE_CAP)
                .cloned()
                .collect();
        }

        // Search: include inherited (subtitle shows “Can edit · via …”).
        let mut ranked: Vec<(u8, &String)> = available
            .into_iter()
            .filter_map(|u| match_quality(u, &q).map(|rank| (rank, u)))
            .collect();
        ranked.sort_by(|a, b| a.0.cmp(&b.0));
        ranked
            .into_iter()
            .take(Self::SEARCH_CAP)
            .map(|(_, u)| u.clone())
            .collect()
    }

    /// Free-typed sticky invite — only when the known pool has **no** partial
    /// matches (Docs-style: typeahead wins over “invite this exact string”).
    fn needs_remote_lookup(&self, files: &impl FilesExt, q: &str) -> bool {
        let q = q.trim().to_lowercase();
        if q.is_empty() || q.contains(char::is_whitespace) {
            return false;
        }
        if q == self.me.to_lowercase() {
            return false;
        }
        if self.already_on_file_or_pending(files, &q) {
            return false;
        }
        // Known pool match (prefix/substring/…) → typeahead only, no network.
        if self.known_pool_matches(&q) {
            return false;
        }
        true
    }

    /// Any known collaborator matches this query (for typeahead priority).
    fn known_pool_matches(&self, q: &str) -> bool {
        if q.is_empty() {
            return false;
        }
        self.suggested.iter().any(|u| {
            !self
                .pending
                .iter()
                .any(|p| p.username.eq_ignore_ascii_case(u))
                && match_quality(u, q).is_some()
        })
    }

    /// Sticky row under the field for the current query.
    ///
    /// Priority (people-picker prior art):
    /// 1. Membership / self — explain “already on”
    /// 2. Known-pool partial matches — **no sticky** (typeahead rows instead)
    /// 3. Else free-type lookup for inviting someone new by exact username
    fn sticky_invite(&self, files: &impl FilesExt) -> Option<ShareHit> {
        let q = self.query.trim().to_lowercase();
        if q.is_empty() || q.contains(char::is_whitespace) {
            return None;
        }

        // Membership / self — surface why they can’t be invited again.
        if self.is_file_owner(files, &q) {
            let status = if q == self.me.to_lowercase() {
                "You own this file".into()
            } else {
                "Owner — already on this file".into()
            };
            return Some(ShareHit {
                username: q,
                status,
                icon: icons::CHECK_CIRCLE,
                tone: ShareHitTone::Ok,
                can_stage: false,
            });
        }
        if self.pending.iter().any(|p| p.username.eq_ignore_ascii_case(&q)) {
            return Some(ShareHit {
                username: q,
                status: "Already staged".into(),
                icon: icons::CHECK_CIRCLE,
                tone: ShareHitTone::Ok,
                can_stage: false,
            });
        }
        if files.get_by_id(self.id).is_some_and(|f| {
            f.shares
                .iter()
                .any(|s| s.shared_with.eq_ignore_ascii_case(&q))
        }) {
            return Some(ShareHit {
                username: q,
                status: "Already a collaborator".into(),
                icon: icons::CHECK_CIRCLE,
                tone: ShareHitTone::Ok,
                can_stage: false,
            });
        }
        if q == self.me.to_lowercase() {
            return Some(ShareHit {
                username: q,
                status: "That’s you".into(),
                icon: icons::CHECK_CIRCLE,
                tone: ShareHitTone::Ok,
                can_stage: false,
            });
        }

        // Partial match in known pool → typeahead only (don’t hide with
        // “bow not found” while chefbowyer matches).
        if self.known_pool_matches(&q) {
            return None;
        }

        let (icon, tone, status) = self.lookup.query_hit_visual(&q);
        Some(ShareHit {
            username: q,
            status,
            icon,
            tone,
            can_stage: matches!(tone, ShareHitTone::Ok),
        })
    }

    /// Drain worker results and kick debounced get_public_key for free-typed names.
    pub fn maintain_lookup(
        &mut self, core: &lb::blocking::Lb, ctx: &egui::Context, files: &impl FilesExt,
    ) {
        // Apply finished lookups (ignore stale gens).
        while let Ok((gen, _q, state)) = self.lookup_rx.try_recv() {
            if gen == self.lookup_gen {
                self.lookup = state;
            }
        }

        let now = ctx.input(|i| i.time);
        let q = self.query.trim().to_lowercase();
        if q != self.last_seen_query {
            self.last_seen_query = q.clone();
            self.query_edit_at = now;
            // New query invalidates prior answer immediately.
            if !self.lookup.matches_q(&q) {
                self.lookup = if q.is_empty() {
                    UsernameLookup::Idle
                } else {
                    UsernameLookup::Debouncing { query: q.clone() }
                };
            }
        }

        let needs = self.needs_remote_lookup(files, &q);

        if !needs {
            // Idle when typeahead / membership covers the query (including
            // partial known-pool hits — don't leave a stale “not found”).
            if q.is_empty()
                || self.is_suggested_exact(&q)
                || self.already_on_file_or_pending(files, &q)
                || self.known_pool_matches(&q)
            {
                self.lookup = UsernameLookup::Idle;
            }
            return;
        }

        // Already have a final answer or a live request for this query.
        if self.lookup.is_settled_for(&q) || self.lookup.is_inflight_for(&q) {
            return;
        }

        // Debounce typing — do **not** treat this as in-flight network.
        const DEBOUNCE: f64 = 0.20;
        if now - self.query_edit_at < DEBOUNCE {
            self.lookup = UsernameLookup::Debouncing { query: q };
            return;
        }

        // Launch get_public_key (cache-first inside lb-rs).
        self.lookup_gen = self.lookup_gen.wrapping_add(1);
        let gen = self.lookup_gen;
        self.lookup = UsernameLookup::Checking { query: q.clone() };

        let tx = self.lookup_tx.clone();
        let core = core.clone();
        let ctx = ctx.clone();
        let query = q;
        std::thread::spawn(move || {
            use lb::model::errors::LbErrKind;
            let state = match core.get_public_key(&query) {
                Ok(Some(_)) => UsernameLookup::Found {
                    query: query.clone(),
                },
                Ok(None) => UsernameLookup::NotFound {
                    query: query.clone(),
                },
                Err(e) if matches!(e.kind, LbErrKind::ServerUnreachable) => {
                    UsernameLookup::Offline {
                        query: query.clone(),
                    }
                }
                Err(e) => UsernameLookup::Failed {
                    query: query.clone(),
                    msg: format!("{e}"),
                },
            };
            let _ = tx.send((gen, query, state));
            ctx.request_repaint();
        });
    }

    /// Stage `name` with `default_mode` if non-empty and not already present.
    fn stage(&mut self, files: &impl FilesExt, name: &str) {
        let name = name.trim().to_lowercase();
        if name.is_empty() {
            return;
        }
        if self.is_file_owner(files, &name) {
            self.error = if name == self.me.to_lowercase() {
                "You already own this file.".into()
            } else {
                format!("“{name}” owns this file.")
            };
            return;
        }
        if name == self.me.to_lowercase() {
            self.error = "You can’t share with yourself.".into();
            return;
        }
        if self.already_on_file_or_pending(files, &name) {
            self.error = format!("“{name}” is already a collaborator.");
            return;
        }
        // Free-typed names must pass get_public_key; suggested people skip.
        if !self.is_suggested_exact(&name) {
            if let Err(msg) = self.lookup.allows_stage(&name) {
                self.error = msg;
                return;
            }
        }
        self.pending.push(PendingShare {
            username: name,
            mode: self.default_mode,
        });
        self.query.clear();
        self.selected = None;
        self.lookup = UsernameLookup::Idle;
        self.error.clear();
    }
}

/// Result of early keyboard handling for the share sheet.
pub enum ShareKeyResult {
    None,
    /// Escape — close without submitting.
    Dismiss,
    /// ⌘↩ — apply staged shares.
    Submit,
}

/// Keyboard + focus for the share sheet. Call **before** `workspace.show`.
///
/// - Invite field is Docs-style primary chrome (focused on open)
/// - Type-to-invite re-focuses the field if focus left
/// - ↑/↓ / ⌘1–9 navigate typeahead under the field
/// - While typing with known hits: first match is auto-highlighted (people-picker)
/// - Enter: stage **highlight / first match**, only free-type when no known hits
/// - ⌘↩ submits when anything is staged
/// - Esc dismisses
pub fn handle_share_keyboard(
    ctx: &egui::Context, files: &impl FilesExt, modal: &mut ShareModal,
) -> ShareKeyResult {
    let search_id = share_username_id();
    let search_focused = ctx.memory(|m| m.has_focus(search_id));

    // Keep typing landing in the invite field (palette-style).
    if !search_focused {
        let mut stole_text = false;
        ctx.input_mut(|i| {
            let mut steal: Vec<egui::Event> = Vec::new();
            i.events.retain(|e| match e {
                egui::Event::Text(s) if !s.chars().all(|c| c.is_control()) => {
                    steal.push(e.clone());
                    false
                }
                _ => true,
            });
            for e in steal {
                if let egui::Event::Text(s) = e {
                    modal.query.push_str(&s);
                    stole_text = true;
                }
            }
        });
        if stole_text {
            modal.focus_user = true;
            ctx.memory_mut(|m| m.request_focus(search_id));
        }
    }

    let inherited_names = inherited_usernames(files, modal.id);
    let nav = modal.suggested_nav(&inherited_names);
    let q = modal.query.trim().to_lowercase();
    let searching = !q.is_empty();

    // Keep highlight valid; while searching, auto-select the best match so
    // Enter commits a known person (Docs / Slack / Graph people pickers).
    let sel_ok = modal
        .selected
        .as_ref()
        .is_some_and(|s| nav.iter().any(|u| u.eq_ignore_ascii_case(s)));
    if !sel_ok {
        modal.selected = if searching {
            nav.first().cloned()
        } else {
            None
        };
    }

    let mut dismiss = false;
    let mut submit = false;
    let mut stage_name: Option<String> = None;

    ctx.input_mut(|i| {
        if i.consume_key_exact(Modifiers::NONE, Key::Escape) {
            dismiss = true;
            return;
        }

        // Primary commit — before bare Enter (which stages).
        if i.consume_key_exact(Modifiers::COMMAND, Key::Enter) && !modal.pending.is_empty() {
            submit = true;
            return;
        }

        if !nav.is_empty() {
            if i.consume_key_exact(Modifiers::NONE, Key::ArrowDown) {
                let next = match modal
                    .selected
                    .as_ref()
                    .and_then(|s| nav.iter().position(|u| u.eq_ignore_ascii_case(s)))
                {
                    Some(idx) => (idx + 1).min(nav.len() - 1),
                    None => 0,
                };
                modal.selected = Some(nav[next].clone());
            }
            if i.consume_key_exact(Modifiers::NONE, Key::ArrowUp) {
                let next = match modal
                    .selected
                    .as_ref()
                    .and_then(|s| nav.iter().position(|u| u.eq_ignore_ascii_case(s)))
                {
                    Some(idx) => idx.saturating_sub(1),
                    None => nav.len() - 1,
                };
                modal.selected = Some(nav[next].clone());
            }
            if let Some(n) = consume_cmd_digit(i) {
                if let Some(name) = nav.get(n) {
                    stage_name = Some(name.clone());
                }
            }
        }

        if i.consume_key_exact(Modifiers::NONE, Key::Enter) {
            // People-picker: known hits beat free-type. Free-type only when
            // the typeahead is empty (new username / new user path).
            if let Some(sel) = modal.selected.clone() {
                stage_name = Some(sel);
            } else if let Some(first) = nav.first() {
                stage_name = Some(first.clone());
            } else if !q.is_empty() {
                stage_name = Some(q.clone());
            }
        }
    });

    if dismiss {
        return ShareKeyResult::Dismiss;
    }
    if submit {
        return ShareKeyResult::Submit;
    }
    if let Some(name) = stage_name {
        modal.stage(files, &name);
    }
    ShareKeyResult::None
}

pub enum ShareOutcome {
    /// Keep open (user still editing).
    Open,
    /// Discard pending and close (Cancel / Esc / outside).
    Closed,
    /// Apply all staged shares.
    Submit { pending: Vec<PendingShare> },
}

const SHARE_W: f32 = 440.0;
const SHARE_GAP: f32 = 14.0;
const SHARE_CONTENT_W: f32 = SHARE_W - SHEET_PAD as f32 * 2.0;
const MOVE_W: f32 = 420.0;
const DELETE_W: f32 = 340.0;
/// Fixed folder-list height so the Move sheet doesn’t collapse with few rows.
const MOVE_LIST_H: f32 = 280.0;

/// Layer id for the share sheet Area (sibling above the dim, not nested in it).
pub fn share_sheet_layer_id() -> egui::LayerId {
    egui::LayerId::new(Order::Foreground, Id::new("lb_share_modal"))
}

/// Docs-style share sheet:
/// - Invite field fixed at top (field Y never depends on typeahead height)
/// - Typeahead under the field (suggestions or sticky status)
/// - Collaborators roster: owner · direct shares · **inherited via ancestors**
///   · pending (lb-rs: folder shares grant access to descendants)
/// - Stage → Share (direct share still allowed for people who only inherit)
///
/// Drawn as its own Foreground Area (sibling of the dim layer).
pub fn show_share(
    ctx: &egui::Context,
    t: &Tokens,
    files: &impl FilesExt,
    modal: &mut ShareModal,
) -> ShareOutcome {
    let file_name = files
        .get_by_id(modal.id)
        .map(|f| f.name.clone())
        .unwrap_or_else(|| "File".into());
    let owner = files.get_by_id(modal.id).map(|f| f.owner.clone());
    let mut existing: Vec<(String, ShareMode)> = files
        .get_by_id(modal.id)
        .map(|f| {
            f.shares
                .iter()
                // Owner is listed separately at the top — don’t double-count.
                .filter(|s| {
                    owner
                        .as_ref()
                        .is_none_or(|o| !s.shared_with.eq_ignore_ascii_case(o))
                })
                .map(|s| (s.shared_with.clone(), s.mode))
                .collect()
        })
        .unwrap_or_default();
    existing.sort_by(|a, b| {
        let mode_ord = |m: &ShareMode| match m {
            ShareMode::Write => 0,
            ShareMode::Read => 1,
        };
        mode_ord(&a.1)
            .cmp(&mode_ord(&b.1))
            .then_with(|| a.0.to_lowercase().cmp(&b.0.to_lowercase()))
    });

    let direct_set: std::collections::HashSet<String> = existing
        .iter()
        .map(|(u, _)| u.to_lowercase())
        .collect();
    let inherited = inherited_accesses(
        files,
        modal.id,
        owner.as_deref(),
        &direct_set,
    );
    let inherited_names: std::collections::HashSet<String> = inherited
        .iter()
        .map(|i| i.username.to_lowercase())
        .collect();
    // username → inherited (for typeahead subtitles).
    let inherited_by: std::collections::HashMap<String, &InheritedAccess> = inherited
        .iter()
        .map(|i| (i.username.to_lowercase(), i))
        .collect();

    let sticky = modal.sticky_invite(files);
    let nav = modal.suggested_nav(&inherited_names);
    let selected = modal.selected.clone();
    // Typeahead: sticky status row *or* known-pool matches (not both).
    let show_suggestions = sticky.is_none() && !nav.is_empty();
    let roster_empty = owner.is_none()
        && existing.is_empty()
        && inherited.is_empty()
        && modal.pending.is_empty();

    let mut outcome = ShareOutcome::Open;
    let screen = ctx.screen_rect();
    // Label names who you're about to share with once staged (same idea as Move).
    let (share_label, can_submit) = match modal.pending.as_slice() {
        [] => ("Share".into(), false),
        [p] => (format!("Share with “{}”", p.username), true),
        xs => (format!("Share with {} users", xs.len()), true),
    };

    Area::new(Id::new("lb_share_modal"))
        .order(Order::Foreground)
        .fixed_pos(screen.center() - vec2(SHARE_W / 2.0, 220.0))
        .constrain(true)
        .fade_in(false)
        .show(ctx, |ui| {
            sheet_panel(ui, t, SHARE_CONTENT_W, |ui| {
                    let content_w = ui.available_width();

                    sheet_header(ui, t, "Share", &file_name);
                    ui.add_space(SHARE_GAP);

                    // ── Invite chrome (fixed at top — Docs style) ────────
                    let ctrl_h = search_field::HEIGHT;
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 8.0;
                        ui.set_min_height(ctrl_h);
                        let picker_w = 148.0_f32;
                        let field_w = (ui.available_width() - picker_w - 8.0).max(100.0);

                        ui.scope(|ui| {
                            ui.set_width(field_w);
                            ui.set_min_height(ctrl_h);
                            ui.set_max_height(ctrl_h);
                            let field_resp = search_field::show_with_leading(
                                ui,
                                t,
                                share_username_host(),
                                &mut modal.query,
                                "Add people…",
                                Some(icons::USER),
                            );
                            if field_resp.clicked() {
                                modal.focus_user = true;
                            }
                            if modal.focus_user {
                                field_resp.request_focus();
                                modal.focus_user = false;
                            }
                        });
                        share_mode_picker(ui, t, &mut modal.default_mode, picker_w, ctrl_h);
                    });

                    // Error — fixed slot so the field doesn’t jump when it appears.
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new(if modal.error.is_empty() {
                            " "
                        } else {
                            modal.error.as_str()
                        })
                        .size(13.0)
                        .strong()
                        .color(if modal.error.is_empty() {
                            egui::Color32::TRANSPARENT
                        } else {
                            t.danger()
                        }),
                    );

                    // Typeahead under the field (grows downward — field Y stable).
                    ui.spacing_mut().item_spacing.y = 0.0;
                    if let Some(hit) = sticky.as_ref() {
                        let resp = share_suggestion_row(
                            ui,
                            t,
                            Id::new(("share_sticky", hit.username.as_str())),
                            hit,
                            false,
                            None,
                        );
                        if resp.clicked() && hit.can_stage {
                            modal.stage(files, &hit.username);
                        }
                    } else if show_suggestions {
                        for (i, name) in nav.iter().enumerate() {
                            let is_sel = selected
                                .as_ref()
                                .is_some_and(|s| s.eq_ignore_ascii_case(name));
                            let shortcut = if i < 9 { Some((i + 1) as u8) } else { None };
                            let via = inherited_by
                                .get(&name.to_lowercase())
                                .map(|a| a.typeahead_status());
                            if share_suggested_row(
                                ui,
                                t,
                                Id::new(("share_sug", name.as_str())),
                                name,
                                is_sel,
                                shortcut,
                                via.as_deref(),
                            ) {
                                modal.selected = Some(name.clone());
                                modal.stage(files, name);
                            }
                        }
                    }

                    // ── Collaborators (scroll body under invite chrome) ──
                    ui.add_space(SHARE_GAP);
                    ui.label(
                        RichText::new("Collaborators")
                            .size(11.0)
                            .color(t.text_muted()),
                    );
                    ui.add_space(4.0);

                    crate::widgets::scroll_overlay::with_overlay_scroll(
                        ui,
                        Id::new("share_people_overlay_scroll"),
                        |ui| {
                            let out = ScrollArea::vertical()
                                .id_salt("share_people_list")
                                .max_height(240.0)
                                .min_scrolled_height(0.0)
                                .auto_shrink([false, true])
                                .show(ui, |ui| {
                                    ui.set_min_width(content_w);
                                    ui.spacing_mut().item_spacing.y = 0.0;

                                    if roster_empty {
                                        sheet_empty_row(
                                            ui,
                                            t,
                                            "Not shared yet — add someone above.",
                                        );
                                    } else {
                                        if let Some(owner_name) = &owner {
                                            share_member_row(
                                                ui,
                                                t,
                                                owner_name,
                                                "Owner",
                                                /* accent check */ true,
                                            );
                                        }
                                        // Direct shares on this file.
                                        for (user, mode) in &existing {
                                            share_member_row(
                                                ui,
                                                t,
                                                user,
                                                mode_label(*mode),
                                                true,
                                            );
                                        }
                                        // Inherited via shared ancestor folders
                                        // (lb-rs access_mode walks parents).
                                        for inh in &inherited {
                                            // Skip if already staged for a direct share.
                                            if modal.pending.iter().any(|p| {
                                                p.username.eq_ignore_ascii_case(&inh.username)
                                            }) {
                                                continue;
                                            }
                                            share_member_row(
                                                ui,
                                                t,
                                                &inh.username,
                                                &inh.role_label(),
                                                /* muted — not a direct grant */ false,
                                            );
                                        }
                                        let mut remove_i: Option<usize> = None;
                                        for (i, p) in modal.pending.iter_mut().enumerate() {
                                            if share_pending_row(ui, t, p) {
                                                remove_i = Some(i);
                                            }
                                        }
                                        if let Some(i) = remove_i {
                                            modal.pending.remove(i);
                                        }
                                    }
                                });
                            ((), out.state.offset.y)
                        },
                    );

                    let foot = sheet_footer(
                        ui,
                        t,
                        &share_label,
                        can_submit,
                        SheetFooterOpts {
                            btn_height: Some(search_field::HEIGHT),
                            divider: true,
                            divider_w: content_w,
                            ..Default::default()
                        },
                    );
                    if foot.cancel {
                        outcome = ShareOutcome::Closed;
                    }
                    if foot.primary {
                        outcome = ShareOutcome::Submit {
                            pending: modal.pending.clone(),
                        };
                    }
            });
        });

    outcome
}

/// Empty section / search placeholder — same height as a list row, message
/// centered (Share people list, Move “No folders match.”, etc.).
fn sheet_empty_row(ui: &mut Ui, t: &Tokens, msg: &str) {
    let h = 40.0;
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), h), egui::Sense::hover());
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        msg,
        egui::FontId::proportional(13.0),
        t.text_muted(),
    );
}

/// Access granted by a shared **ancestor** folder (not a direct share on this file).
/// Mirrors lb-rs: folder shares apply to descendants (`access_mode` walks parents).
#[derive(Clone, Debug)]
struct InheritedAccess {
    username: String,
    mode: ShareMode,
    /// Nearest ancestor folder that grants this share.
    via_name: String,
}

impl InheritedAccess {
    /// Roster trailing role — same verbs as direct shares, plus ancestor.
    fn role_label(&self) -> String {
        format!("{} · via {}", mode_label(self.mode), self.via_name)
    }

    /// Typeahead subtitle — dense mode + where the grant lives.
    fn typeahead_status(&self) -> String {
        format!("{} · via {}", mode_label(self.mode), self.via_name)
    }
}

/// Usernames that already reach `file_id` via a shared ancestor (not a direct
/// share). Used to suppress idle suggestions while still allowing search.
fn inherited_usernames(files: &impl FilesExt, file_id: Uuid) -> std::collections::HashSet<String> {
    let owner = files.get_by_id(file_id).map(|f| f.owner.clone());
    let direct: std::collections::HashSet<String> = files
        .get_by_id(file_id)
        .map(|f| {
            f.shares
                .iter()
                .map(|s| s.shared_with.to_lowercase())
                .collect()
        })
        .unwrap_or_default();
    inherited_accesses(files, file_id, owner.as_deref(), &direct)
        .into_iter()
        .map(|i| i.username.to_lowercase())
        .collect()
}

/// People who can reach `file_id` only because an ancestor folder is shared
/// with them. Direct sharees and the owner are excluded.
fn inherited_accesses(
    files: &impl FilesExt, file_id: Uuid, owner: Option<&str>,
    direct: &std::collections::HashSet<String>,
) -> Vec<InheritedAccess> {
    // username → (mode, via_name); first (closest) ancestor wins for via_name;
    // mode upgrades if a farther ancestor grants Write.
    let mut map: std::collections::HashMap<String, (ShareMode, String)> =
        std::collections::HashMap::new();

    for ancestor_id in files.ancestors(file_id) {
        let Some(ancestor) = files.get_by_id(ancestor_id) else {
            continue;
        };
        let via_name = if ancestor.is_root() {
            "Home".to_string()
        } else {
            ancestor.name.clone()
        };
        for s in &ancestor.shares {
            let u = s.shared_with.to_lowercase();
            if u.is_empty() || u == "<unknown>" {
                continue;
            }
            if owner.is_some_and(|o| o.eq_ignore_ascii_case(&u)) {
                continue;
            }
            if direct.contains(&u) {
                continue;
            }
            map.entry(u)
                .and_modify(|(mode, _)| {
                    if matches!((s.mode, *mode), (ShareMode::Write, ShareMode::Read)) {
                        *mode = ShareMode::Write;
                    }
                })
                .or_insert((s.mode, via_name.clone()));
        }
    }

    let mut v: Vec<InheritedAccess> = map
        .into_iter()
        .map(|(username, (mode, via_name))| InheritedAccess {
            username,
            mode,
            via_name,
        })
        .collect();
    v.sort_by(|a, b| {
        let mode_ord = |m: &ShareMode| match m {
            ShareMode::Write => 0,
            ShareMode::Read => 1,
        };
        mode_ord(&a.mode)
            .cmp(&mode_ord(&b.mode))
            .then_with(|| a.username.cmp(&b.username))
    });
    v
}

fn mode_label(mode: ShareMode) -> &'static str {
    match mode {
        ShareMode::Write => "Can edit",
        ShareMode::Read => "Can view",
    }
}

/// Settled roster member — checkmark + trailing role (“Owner”, “Can edit”, …).
fn share_member_row(ui: &mut Ui, t: &Tokens, user: &str, role: &str, accent_check: bool) {
    use tree_chrome::{INDENT_BASE, NAME_FONT, TYPE_ICON_SIZE, TYPE_ICON_SLOT};
    let h = 40.0;
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), h), egui::Sense::hover());
    let painter = ui.painter();
    let mut x = rect.left() + INDENT_BASE;
    let cy = rect.center().y;
    let check_ink = if accent_check { t.accent() } else { t.text_muted() };
    let check = painter.layout_no_wrap(
        icons::CHECK_CIRCLE.into(),
        icons::font(TYPE_ICON_SIZE + 1.0),
        check_ink,
    );
    painter.galley(
        egui::pos2(x, cy - check.size().y / 2.0),
        check,
        check_ink,
    );
    x += TYPE_ICON_SLOT;
    let name_g =
        painter.layout_no_wrap(user.into(), egui::FontId::proportional(NAME_FONT), t.fg());
    painter.galley(egui::pos2(x, cy - name_g.size().y / 2.0), name_g, t.fg());
    let mode_g = painter.layout_no_wrap(
        role.into(),
        egui::FontId::proportional(12.0),
        t.text_muted(),
    );
    painter.galley(
        egui::pos2(rect.right() - 8.0 - mode_g.size().x, cy - mode_g.size().y / 2.0),
        mode_g,
        t.text_muted(),
    );
}

/// Suggested person — + stages them. Optional status (e.g. inherited access).
/// Returns true if + / row clicked.
fn share_suggested_row(
    ui: &mut Ui, t: &Tokens, id: Id, name: &str, selected: bool, shortcut: Option<u8>,
    status: Option<&str>,
) -> bool {
    use tree_chrome::{INDENT_BASE, NAME_FONT, TYPE_ICON_SIZE, TYPE_ICON_SLOT};
    let h = if status.is_some() { 44.0 } else { 40.0 };
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), h), egui::Sense::hover());
    let resp = ui.interact(rect, id, egui::Sense::click());
    let hover = ui.ctx().animate_bool(resp.id, resp.hovered());
    let painter = ui.painter();

    if selected {
        painter.rect_filled(rect, 5.0, t.surface().lerp_to_gamma(t.fg(), 0.10));
    } else if hover > 0.0 {
        painter.rect_filled(rect, 5.0, t.surface().lerp_to_gamma(t.fg(), 0.05 * hover));
    }

    let mut x = rect.left() + INDENT_BASE;
    let cy = rect.center().y;
    let plus = painter.layout_no_wrap(
        icons::USER_PLUS.into(),
        icons::font(TYPE_ICON_SIZE + 1.0),
        t.accent(),
    );
    painter.galley(egui::pos2(x, cy - plus.size().y / 2.0), plus, t.accent());
    x += TYPE_ICON_SLOT;

    let name_g =
        painter.layout_no_wrap(name.into(), egui::FontId::proportional(NAME_FONT), t.fg());
    if let Some(st) = status {
        let status_g =
            painter.layout_no_wrap(st.into(), egui::FontId::proportional(12.0), t.text_muted());
        let stack_h = name_g.size().y + 2.0 + status_g.size().y;
        let y0 = cy - stack_h / 2.0;
        painter.galley(egui::pos2(x, y0), name_g, t.fg());
        painter.galley(
            egui::pos2(x, y0 + stack_h - status_g.size().y),
            status_g,
            t.text_muted(),
        );
    } else {
        painter.galley(egui::pos2(x, cy - name_g.size().y / 2.0), name_g, t.fg());
    }

    if let Some(n) = shortcut {
        let modifier = if cfg!(target_os = "macos") { "⌘" } else { "⌃" };
        let badge = format!("{modifier}{n}");
        let muted = t.text_muted();
        let bg = painter.layout_no_wrap(badge, egui::FontId::proportional(12.0), muted);
        painter.galley(
            egui::pos2(rect.right() - 8.0 - bg.size().x, cy - bg.size().y / 2.0),
            bg,
            muted,
        );
    }
    resp.clicked()
}

/// Two-line hit: colored status icon + username + one status line.
fn share_suggestion_row(
    ui: &mut Ui, t: &Tokens, id: Id, hit: &ShareHit, selected: bool, shortcut: Option<u8>,
) -> egui::Response {
    use tree_chrome::{INDENT_BASE, NAME_FONT, TYPE_ICON_SIZE, TYPE_ICON_SLOT};

    let h = 44.0;
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), h), egui::Sense::hover());
    let resp = ui.interact(rect, id, egui::Sense::click());
    let hover = ui.ctx().animate_bool(resp.id, resp.hovered());
    let painter = ui.painter();

    // Selection / hover fills stay on the neutral ramp (palette steps only).
    if selected {
        painter.rect_filled(rect, 5.0, t.surface_raised());
    } else if hover > 0.0 {
        painter.rect_filled(
            rect,
            5.0,
            t.surface().lerp_to_gamma(t.surface_raised(), hover),
        );
    }

    let (icon_ink, status_ink) = match hit.tone {
        ShareHitTone::Progress => (t.text_muted(), t.text_muted()),
        ShareHitTone::Ok => (t.accent(), t.accent()),
        ShareHitTone::Bad | ShareHitTone::Caution => (t.danger(), t.danger()),
    };
    let name_ink = t.fg();

    let mut x = rect.left() + INDENT_BASE;
    let name_g = painter.layout_no_wrap(
        hit.username.clone(),
        egui::FontId::proportional(NAME_FONT),
        name_ink,
    );
    let status_g = painter.layout_no_wrap(
        hit.status.clone(),
        egui::FontId::proportional(12.0),
        status_ink,
    );
    let name_h = name_g.size().y;
    let stack_h = name_h + 2.0 + status_g.size().y;
    let y0 = rect.center().y - stack_h / 2.0;

    let ig = painter.layout_no_wrap(hit.icon.into(), icons::font(TYPE_ICON_SIZE + 1.0), icon_ink);
    let icon_pos = egui::pos2(x, rect.center().y - ig.size().y / 2.0);
    if matches!(hit.tone, ShareHitTone::Progress) {
        // Continuous spin while looking up (~1 turn / second), same feel as sync footer.
        let angle =
            (ui.input(|i| i.time) as f32 * std::f32::consts::TAU) % std::f32::consts::TAU;
        let shape = egui::epaint::TextShape::new(icon_pos, ig, icon_ink)
            .with_override_text_color(icon_ink)
            .with_angle_and_anchor(angle, egui::Align2::CENTER_CENTER);
        painter.add(shape);
        ui.ctx().request_repaint();
    } else {
        painter.galley(icon_pos, ig, icon_ink);
    }
    x += TYPE_ICON_SLOT;

    painter.galley(egui::pos2(x, y0), name_g, name_ink);
    painter.galley(egui::pos2(x, y0 + name_h + 2.0), status_g, status_ink);

    if let Some(n) = shortcut {
        let modifier = if cfg!(target_os = "macos") { "⌘" } else { "⌃" };
        let badge = format!("{modifier}{n}");
        let muted = t.text_muted();
        let bg = painter.layout_no_wrap(badge, egui::FontId::proportional(12.0), muted);
        painter.galley(
            egui::pos2(rect.right() - 8.0 - bg.size().x, rect.center().y - bg.size().y / 2.0),
            bg,
            muted,
        );
    }
    resp
}

/// Staged sharee under **On this file** — not applied yet. Clock (not ✓),
/// “Pending” subtitle, mode picker, remove. Returns true if remove clicked.
fn share_pending_row(ui: &mut Ui, t: &Tokens, p: &mut PendingShare) -> bool {
    use tree_chrome::{INDENT_BASE, NAME_FONT, TYPE_ICON_SIZE, TYPE_ICON_SLOT};

    let mut remove = false;
    let h = 44.0_f32;
    let picker_w = 148.0_f32;
    let remove_w = 28.0_f32;
    let trail_w = picker_w + 8.0 + remove_w;

    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), h), egui::Sense::hover());

    let mut x = rect.left() + INDENT_BASE;
    let cy = rect.center().y;
    let clock = ui.painter().layout_no_wrap(
        icons::CLOCK.into(),
        icons::font(TYPE_ICON_SIZE + 1.0),
        t.text_muted(),
    );
    ui.painter().galley(
        egui::pos2(x, cy - clock.size().y / 2.0),
        clock,
        t.text_muted(),
    );
    x += TYPE_ICON_SLOT;

    let name_g = ui.painter().layout_no_wrap(
        p.username.clone(),
        egui::FontId::proportional(NAME_FONT),
        t.fg(),
    );
    let status_g = ui.painter().layout_no_wrap(
        "Pending".into(),
        egui::FontId::proportional(12.0),
        t.text_muted(),
    );
    let name_h = name_g.size().y;
    let stack_h = name_h + 2.0 + status_g.size().y;
    let y0 = cy - stack_h / 2.0;
    ui.painter().galley(egui::pos2(x, y0), name_g, t.fg());
    ui.painter()
        .galley(egui::pos2(x, y0 + name_h + 2.0), status_g, t.text_muted());

    let trail = egui::Rect::from_min_max(
        egui::pos2(rect.right() - trail_w, rect.top()),
        rect.right_bottom(),
    );
    ui.scope_builder(egui::UiBuilder::new().max_rect(trail), |ui| {
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.set_min_height(h);
            ui.spacing_mut().item_spacing.x = 8.0;
            let remove_resp = ui.add(
                egui::Button::new(RichText::new("×").size(16.0).color(t.text_muted())).frame(false),
            );
            workspace_rs::widgets::tip_text(ui.ctx(), &remove_resp, "Remove");
            if remove_resp.clicked() {
                remove = true;
            }
            share_mode_picker(ui, t, &mut p.mode, picker_w, search_field::HEIGHT);
        });
    });
    remove
}

/// Access mode — two segments, one group.
///
/// No resting “surface on surface” fill. Grouping is a quiet static border;
/// selection is a plain `surface` pill under the active side. Labels stay full
/// ink (selection is the bg). Hover wash only on the half under the pointer.
fn share_mode_picker(ui: &mut Ui, t: &Tokens, mode: &mut ShareMode, width: f32, height: f32) {
    let (rect, resp) = ui.allocate_exact_size(vec2(width, height), egui::Sense::click());
    // Same corner family as the search field (not button 6) so the row reads as one system.
    let radius = CornerRadius::same(8);
    // Quiet group border only (does not firm on hover — that read as one button).
    ui.painter().rect_stroke(
        rect,
        radius,
        Stroke::new(1.0, t.line()),
        egui::StrokeKind::Inside,
    );

    let mid = rect.center().x;
    // 5px all around the group; 2px total between the two options.
    let pad = 5.0_f32;
    let half_gap = 1.0_f32;
    let left = egui::Rect::from_min_max(
        egui::pos2(rect.left() + pad, rect.top() + pad),
        egui::pos2(mid - half_gap, rect.bottom() - pad),
    );
    let right = egui::Rect::from_min_max(
        egui::pos2(mid + half_gap, rect.top() + pad),
        egui::pos2(rect.right() - pad, rect.bottom() - pad),
    );
    let write_on = matches!(*mode, ShareMode::Write);

    let pointer = resp.hover_pos().or_else(|| resp.interact_pointer_pos());
    let hover_left = pointer.is_some_and(|p| p.x < mid) && resp.hovered();
    let hover_right = pointer.is_some_and(|p| p.x >= mid) && resp.hovered();
    let h_l = ui.ctx().animate_bool(resp.id.with("L"), hover_left);
    let h_r = ui.ctx().animate_bool(resp.id.with("R"), hover_right);

    let paint_seg = |r: egui::Rect, label: &str, on: bool, hov: f32| {
        // Same washes as the file tree: canvas ⨁ fg (10% select, 5% hover).
        let fill = if on {
            t.canvas().lerp_to_gamma(t.fg(), 0.10)
        } else if hov > 0.0 {
            t.canvas().lerp_to_gamma(t.fg(), 0.05 * hov)
        } else {
            egui::Color32::TRANSPARENT
        };
        if fill.a() > 0 {
            ui.painter().rect_filled(r, CornerRadius::same(4), fill);
        }
        // Both sides full ink — selection is the surface pill, not muted type.
        let g = ui.painter().layout_no_wrap(
            label.into(),
            egui::FontId::proportional(13.0),
            t.fg(),
        );
        ui.painter().galley(r.center() - g.size() / 2.0, g, t.fg());
    };
    paint_seg(left, "Can edit", write_on, h_l);
    paint_seg(right, "Can view", !write_on, h_r);

    if resp.clicked() {
        if let Some(p) = resp.interact_pointer_pos() {
            *mode = if p.x < mid {
                ShareMode::Write
            } else {
                ShareMode::Read
            };
        }
    }
}

// ── Confirm sheets (Delete, Dismiss share, …) ───────────────────────────────

/// Outcome of a small confirm sheet (Delete / Dismiss share / similar).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfirmOutcome {
    Open,
    Closed,
    Confirm,
}

/// Content for a one-shot confirm: canvas panel, action+subject header, muted
/// body, Cancel | primary footer. Shared by Delete and Dismiss-share (and
/// any future soft/hard confirms that fit this shape).
pub struct ConfirmSheet<'a> {
    pub area_id: Id,
    /// Optional focus sink so keys don't land in the editor (Delete).
    pub focus_id: Option<Id>,
    /// Left header — e.g. "Delete", "Dismiss".
    pub action: &'a str,
    /// Right header (muted) — file name / "N items".
    pub subject: &'a str,
    /// Muted explanation under the header.
    pub body: &'a str,
    /// Primary button label.
    pub primary: &'a str,
    pub danger: bool,
    pub width: f32,
}

impl Default for ConfirmSheet<'_> {
    fn default() -> Self {
        Self {
            area_id: Id::new("lb_confirm_sheet"),
            focus_id: None,
            action: "Confirm",
            subject: "",
            body: "",
            primary: "Confirm",
            danger: false,
            width: DELETE_W,
        }
    }
}

/// Canvas confirm sheet used by Delete, Dismiss share, etc.
pub fn show_confirm_sheet(
    ctx: &egui::Context, t: &Tokens, sheet: &ConfirmSheet<'_>,
) -> ConfirmOutcome {
    let mut outcome = ConfirmOutcome::Open;
    let screen = ctx.screen_rect();
    let w = sheet.width;

    Area::new(sheet.area_id)
        .order(Order::Foreground)
        .fixed_pos(screen.center() - vec2(w * 0.5, 80.0))
        .constrain(true)
        .fade_in(false)
        .show(ctx, |ui| {
            sheet_panel(ui, t, w, |ui| {
                if let Some(fid) = sheet.focus_id {
                    let (focus_rect, _) = ui.allocate_exact_size(
                        vec2(w, 1.0),
                        egui::Sense::focusable_noninteractive(),
                    );
                    ui.interact(focus_rect, fid, egui::Sense::focusable_noninteractive());
                    ui.memory_mut(|m| m.request_focus(fid));
                }

                sheet_header(ui, t, sheet.action, sheet.subject);
                if !sheet.body.is_empty() {
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(sheet.body)
                            .size(13.0)
                            .color(t.text_muted()),
                    );
                }
                ui.add_space(16.0);
                let foot = sheet_footer(
                    ui,
                    t,
                    sheet.primary,
                    true,
                    SheetFooterOpts {
                        danger: sheet.danger,
                        ..Default::default()
                    },
                );
                if foot.cancel {
                    outcome = ConfirmOutcome::Closed;
                }
                if foot.primary {
                    outcome = ConfirmOutcome::Confirm;
                }
            });
        });

    outcome
}

// ── Delete / remove-from-files confirm ──────────────────────────────────────

/// Layer id for the delete confirm sheet.
pub fn delete_sheet_layer_id() -> egui::LayerId {
    egui::LayerId::new(Order::Foreground, Id::new("lb_delete_modal"))
}

/// Whether the confirm is a true delete or un-organizing share links.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeleteKind {
    /// Own files — gone for good.
    Delete,
    /// Share links in the tree — return to Shared with me.
    RemoveFromFiles,
}

pub struct DeleteModal {
    pub ids: Vec<Uuid>,
    /// Display names for the confirmation copy.
    pub names: Vec<String>,
    pub kind: DeleteKind,
}

/// Focus sink for the delete sheet — held before `workspace.show` so typing
/// doesn't land in the markdown editor underneath.
pub fn delete_focus_id() -> Id {
    Id::new("lb_delete_modal_focus")
}

impl DeleteModal {
    pub fn new(ids: Vec<Uuid>, names: Vec<String>) -> Self {
        Self {
            ids,
            names,
            kind: DeleteKind::Delete,
        }
    }

    pub fn remove_from_files(ids: Vec<Uuid>, names: Vec<String>) -> Self {
        Self {
            ids,
            names,
            kind: DeleteKind::RemoveFromFiles,
        }
    }
}

pub enum DeleteOutcome {
    Open,
    Closed,
    Confirm,
}

/// Result of early keyboard handling for the delete sheet.
pub enum DeleteKeyResult {
    None,
    Dismiss,
    Confirm,
}

/// Esc / Enter / ⌘↩ for delete. Call **before** `workspace.show`.
pub fn handle_delete_keyboard(ctx: &egui::Context) -> DeleteKeyResult {
    let mut result = DeleteKeyResult::None;
    ctx.input_mut(|i| {
        if i.consume_key_exact(Modifiers::NONE, Key::Escape) {
            result = DeleteKeyResult::Dismiss;
            return;
        }
        // No field — bare Enter commits; ⌘↩ matches other sheets.
        if i.consume_key_exact(Modifiers::NONE, Key::Enter)
            || i.consume_key_exact(Modifiers::COMMAND, Key::Enter)
        {
            result = DeleteKeyResult::Confirm;
        }
    });
    result
}

pub fn show_delete(ctx: &egui::Context, t: &Tokens, modal: &DeleteModal) -> DeleteOutcome {
    let subject = sheet_subject(&modal.names);
    // Keep primary short — the full name is already the header subject.
    // Long “Delete “name”” labels were overflowing Cancel on the footer.
    let (action, body, primary, danger) = match modal.kind {
        DeleteKind::Delete => (
            "Delete",
            "This cannot be undone.",
            "Delete".to_string(),
            true,
        ),
        DeleteKind::RemoveFromFiles => (
            "Remove from files",
            "It will return to Shared with me. You can add it again later.",
            "Remove".to_string(),
            false,
        ),
    };

    match show_confirm_sheet(
        ctx,
        t,
        &ConfirmSheet {
            area_id: Id::new("lb_delete_modal"),
            focus_id: Some(delete_focus_id()),
            action,
            subject: &subject,
            body,
            primary: &primary,
            danger,
            width: DELETE_W,
        },
    ) {
        ConfirmOutcome::Open => DeleteOutcome::Open,
        ConfirmOutcome::Closed => DeleteOutcome::Closed,
        ConfirmOutcome::Confirm => DeleteOutcome::Confirm,
    }
}

/// Call **before** `workspace.show` whenever a shell modal is open.
///
/// Markdown (and other tab editors) only type when they hold egui focus — but
/// opening a modal from the tree leaves the editor focused, so keys still land
/// under the sheet. Hold focus on the modal's widget and strip text events for
/// confirm-only sheets so nothing leaks through.
pub fn hold_modal_focus(ctx: &egui::Context, kind: ModalKind) {
    match kind {
        ModalKind::Delete => {
            ctx.memory_mut(|m| m.request_focus(delete_focus_id()));
            // Drop editor-bound input for the frame. Keep Esc / Enter / ⌘↩ for
            // `handle_delete_keyboard`; pointer events stay so buttons work.
            ctx.input_mut(|i| {
                i.events.retain(|e| match e {
                    egui::Event::Text(_)
                    | egui::Event::Paste(_)
                    | egui::Event::Cut
                    | egui::Event::Copy => false,
                    egui::Event::Key {
                        key: Key::Escape | Key::Enter,
                        ..
                    } => true,
                    egui::Event::Key { pressed: true, .. } => false,
                    _ => true,
                });
            });
        }
        ModalKind::Share => {
            // Username field paints later this frame; claiming focus now keeps
            // the editor from reclaiming when `focused().is_none()`.
            ctx.memory_mut(|m| m.request_focus(share_username_id()));
        }
        ModalKind::Move => {
            // Search field steals text in `handle_move_keyboard`; ensure focus.
            ctx.memory_mut(|m| m.request_focus(move_search_edit_id()));
        }
    }
}

/// Which shell modal is open (for early focus / input gating).
#[derive(Clone, Copy, Debug)]
pub enum ModalKind {
    Delete,
    Share,
    Move,
}


// ── Folder picker (Move / Add-to-files) ─────────────────────────────────────

/// What the shared folder-destination sheet is doing.
#[derive(Clone, Debug)]
pub enum FolderPickerPurpose {
    /// Move these files into the chosen folder.
    Move { ids: Vec<Uuid> },
    /// Accept a pending share by creating a link under the chosen folder.
    AcceptShare { id: Uuid, name: String },
}

/// Layer id for the move / add-to-files sheet.
pub fn move_sheet_layer_id() -> egui::LayerId {
    egui::LayerId::new(Order::Foreground, Id::new("lb_move_modal"))
}

/// Folder destination picker — Move, and Accept share (“Add to files”).
pub struct MoveModal {
    pub purpose: FolderPickerPurpose,
    pub selected: Option<Uuid>,
    pub expanded: HashSet<Uuid>,
    /// Folder name filter (Apple picker search). Empty = full tree.
    pub query: String,
    pub focus_search: bool,
    /// Last frame had a non-empty filter — used to drop the search highlight
    /// when the query is cleared (×, select-all + delete, etc.).
    was_searching: bool,
    /// Set when keyboard moves the highlight; paint clears after `scroll_to_me`.
    /// Avoids fighting free mouse scroll while a row stays selected.
    scroll_sel_into_view: bool,
    /// Ignore outside-click dismiss for this many frames after open so the
    /// primary click that chose "Move" in the context menu doesn't close us.
    pub suppress_outside_frames: u8,
    /// Previous frame's sheet rect — used to swallow outside clicks *before*
    /// the workspace runs so the editor doesn't place a cursor.
    pub sheet_rect: egui::Rect,
}

impl MoveModal {
    pub fn new(ids: Vec<Uuid>, root: Option<Uuid>) -> Self {
        Self::with_purpose(FolderPickerPurpose::Move { ids }, root)
    }

    /// Accept-share destination: same UI as Move, different confirm action.
    pub fn accept_share(id: Uuid, name: String, root: Option<Uuid>) -> Self {
        Self::with_purpose(FolderPickerPurpose::AcceptShare { id, name }, root)
    }

    fn with_purpose(purpose: FolderPickerPurpose, root: Option<Uuid>) -> Self {
        let mut expanded = HashSet::new();
        if let Some(r) = root {
            expanded.insert(r);
        }
        Self {
            purpose,
            // No destination until the user arrows or clicks a row.
            selected: None,
            expanded,
            query: String::new(),
            focus_search: true,
            was_searching: false,
            scroll_sel_into_view: false,
            // Context-menu click is still "primary_clicked" this frame / next.
            suppress_outside_frames: 2,
            sheet_rect: egui::Rect::NOTHING,
        }
    }

    /// File ids that constrain valid destinations (move sources). Empty for accept.
    fn source_ids(&self) -> &[Uuid] {
        match &self.purpose {
            FolderPickerPurpose::Move { ids } => ids.as_slice(),
            FolderPickerPurpose::AcceptShare { .. } => &[],
        }
    }

    fn action_label(&self) -> &'static str {
        match &self.purpose {
            FolderPickerPurpose::Move { .. } => "Move",
            FolderPickerPurpose::AcceptShare { .. } => "Add to files",
        }
    }

    fn subject_label(&self, files: &impl FilesExt) -> String {
        match &self.purpose {
            FolderPickerPurpose::Move { ids } => sheet_subject(&names_for_ids(files, ids)),
            FolderPickerPurpose::AcceptShare { name, .. } => name.clone(),
        }
    }

    /// Primary button copy for the current destination (or disabled fallback).
    /// Short labels only — the destination name is already shown above the footer.
    fn primary_label(&self, _files: &impl FilesExt, dest: Option<Uuid>) -> (String, bool) {
        match (&self.purpose, dest) {
            (FolderPickerPurpose::Move { .. }, Some(_)) => ("Move here".into(), true),
            (FolderPickerPurpose::AcceptShare { .. }, Some(_)) => ("Add here".into(), true),
            (FolderPickerPurpose::Move { .. }, None) => ("Move".into(), false),
            (FolderPickerPurpose::AcceptShare { .. }, None) => ("Add to files".into(), false),
        }
    }
}

pub enum MoveOutcome {
    Open,
    Closed,
    /// Destination folder chosen — shell interprets via [`MoveModal::purpose`].
    Confirm { parent: Uuid },
}

/// Stable id for the move sheet search field (must match `search_field::show`).
pub fn move_search_edit_id() -> Id {
    Id::new("move_search").with("edit")
}

/// Result of early keyboard handling for the move sheet.
pub enum MoveKeyResult {
    None,
    /// Enter / ⌘N — move into this folder.
    Confirm(Uuid),
    /// Escape — close the sheet.
    Dismiss,
}

/// Keyboard + outside-click for the move sheet. Call **before** `workspace.show`
/// so chords and pointer clicks are consumed ahead of the editor / tabs.
pub fn handle_move_keyboard(
    ctx: &egui::Context, files: &impl FilesExt, modal: &mut MoveModal,
) -> MoveKeyResult {
    let root_id = files.root().id;
    // Owned copy so keyboard closures can mutate `modal` without a dual borrow.
    let source_ids: Vec<Uuid> = modal.source_ids().to_vec();
    let forbidden = forbidden_targets(files, &source_ids);
    let search_id = move_search_edit_id();
    let search_focused = ctx.memory(|m| m.has_focus(search_id));

    // Swallow outside primary-clicks *before* the workspace so markdown doesn't
    // also place a caret. Uses last frame's sheet rect (updated in `show_move`).
    if modal.suppress_outside_frames == 0 && modal.sheet_rect.is_positive() {
        let outside = ctx.input(|i| {
            i.pointer.primary_clicked()
                && i.pointer
                    .interact_pos()
                    .is_some_and(|p| !modal.sheet_rect.contains(p))
        });
        if outside {
            ctx.input_mut(|i| {
                i.events.retain(|e| {
                    !matches!(
                        e,
                        egui::Event::PointerButton {
                            button: egui::PointerButton::Primary,
                            pressed: true,
                            ..
                        }
                    )
                });
            });
            return MoveKeyResult::Dismiss;
        }
    }

    // Typing always edits the filter. If focus wandered, steal Text/Backspace
    // into the query and reclaim focus (lax focus — like a command palette).
    if !search_focused {
        ctx.input_mut(|i| {
            let mut steal: Vec<egui::Event> = Vec::new();
            i.events.retain(|e| match e {
                egui::Event::Text(_) => {
                    steal.push(e.clone());
                    false
                }
                egui::Event::Key {
                    key: Key::Backspace,
                    pressed: true,
                    ..
                } => {
                    steal.push(e.clone());
                    false
                }
                _ => true,
            });
            for e in steal {
                match e {
                    egui::Event::Text(s) => modal.query.push_str(&s),
                    egui::Event::Key {
                        key: Key::Backspace,
                        pressed: true,
                        modifiers,
                        ..
                    } => {
                        if modifiers.command {
                            modal.query.clear();
                        } else {
                            modal.query.pop(); // fine for BMP; emoji may need grapheme trim later
                        }
                    }
                    _ => {}
                }
            }
        });
        ctx.memory_mut(|m| m.request_focus(search_id));
    }

    let q = modal.query.trim().to_lowercase();
    let searching = !q.is_empty();
    let visible = if searching {
        collect_search_hits(files, &q)
    } else {
        collect_tree_visible(files, &modal.expanded, root_id)
    };
    let nav: Vec<Uuid> = visible
        .iter()
        .map(|(id, _)| *id)
        .filter(|id| is_valid_dest(files, &source_ids, *id, &forbidden))
        .collect();

    maintain_move_selection(modal, &nav, searching);

    let mut confirm = false;
    let mut dismiss = false;
    ctx.input_mut(|i| {
        // Escape always closes (before workspace / anything else).
        if i.consume_key_exact(Modifiers::NONE, Key::Escape) {
            dismiss = true;
        }
        if nav.is_empty() {
            return;
        }
        // ↑/↓ only move the highlight — never change expansion. From no
        // selection, ↓ takes the first nav row and ↑ the last.
        if i.consume_key_exact(Modifiers::NONE, Key::ArrowDown) {
            let next = match modal.selected.and_then(|s| nav.iter().position(|id| *id == s)) {
                Some(idx) => (idx + 1).min(nav.len() - 1),
                None => 0,
            };
            modal.selected = Some(nav[next]);
            modal.scroll_sel_into_view = true;
        }
        if i.consume_key_exact(Modifiers::NONE, Key::ArrowUp) {
            let next = match modal.selected.and_then(|s| nav.iter().position(|id| *id == s)) {
                Some(idx) => idx.saturating_sub(1),
                None => nav.len() - 1,
            };
            modal.selected = Some(nav[next]);
            modal.scroll_sel_into_view = true;
        }
        // ←/→ expand/collapse (tree mode). Consumed early so Glyphon doesn't
        // use them for the search caret while the field is focused.
        if !searching {
            if i.consume_key_exact(Modifiers::NONE, Key::ArrowRight) {
                if let Some(id) = modal.selected {
                    modal.expanded.insert(id);
                }
            }
            if i.consume_key_exact(Modifiers::NONE, Key::ArrowLeft) {
                if let Some(id) = modal.selected {
                    modal.expanded.remove(&id);
                }
            }
        }
        // Enter / ⌘↩ confirm (before search field would treat it as submit).
        let can_confirm = modal
            .selected
            .is_some_and(|s| is_valid_dest(files, &source_ids, s, &forbidden));
        if can_confirm
            && (i.consume_key_exact(Modifiers::NONE, Key::Enter)
                || i.consume_key_exact(Modifiers::COMMAND, Key::Enter))
        {
            confirm = true;
        }
        if let Some(n) = consume_cmd_digit(i) {
            if let Some(&id) = nav.get(n) {
                modal.selected = Some(id);
                confirm = true;
            }
        }
    });

    if dismiss {
        MoveKeyResult::Dismiss
    } else if confirm {
        if let Some(parent) = modal
            .selected
            .filter(|s| is_valid_dest(files, &source_ids, *s, &forbidden))
        {
            MoveKeyResult::Confirm(parent)
        } else {
            MoveKeyResult::None
        }
    } else {
        MoveKeyResult::None
    }
}

/// Folder picker paint + click. Keyboard is handled separately via
/// [`handle_move_keyboard`] (must run earlier in the frame).
pub fn show_move(
    ctx: &egui::Context,
    t: &Tokens,
    files: &impl FilesExt,
    modal: &mut MoveModal,
    pinned: &HashSet<Uuid>,
) -> MoveOutcome {
    let mut outcome = MoveOutcome::Open;
    let screen = ctx.screen_rect();
    let root_id = files.root().id;
    let source_ids = modal.source_ids().to_vec();
    let forbidden = forbidden_targets(files, &source_ids);
    let q = modal.query.trim().to_lowercase();
    let searching = !q.is_empty();

    let visible = if searching {
        collect_search_hits(files, &q)
    } else {
        collect_tree_visible(files, &modal.expanded, root_id)
    };
    let nav: Vec<Uuid> = visible
        .iter()
        .map(|(id, _)| *id)
        .filter(|id| is_valid_dest(files, &source_ids, *id, &forbidden))
        .collect();

    maintain_move_selection(modal, &nav, searching);

    let shortcut_of = |id: Uuid| -> Option<u8> {
        nav.iter()
            .position(|x| *x == id)
            .filter(|&i| i < 9)
            .map(|i| (i + 1) as u8)
    };

    // Shared dim scrim (Middle) + outside click → dismiss.
    let backdrop_clicked =
        show_modal_dim(ctx, Id::new("lb_modal_dim_move"), move_sheet_layer_id());

    let sheet = Area::new(Id::new("lb_move_modal"))
        .order(Order::Foreground)
        .fixed_pos(screen.center() - vec2(MOVE_W / 2.0, 220.0))
        .constrain(true)
        .fade_in(false)
        .sense(egui::Sense::click())
        .show(ctx, |ui| {
            sheet_panel(ui, t, MOVE_W, |ui| {
                    let subject = modal.subject_label(files);
                    sheet_header(ui, t, modal.action_label(), &subject);
                    ui.add_space(12.0);

                    let search_resp = search_field::show(
                        ui,
                        t,
                        "move_search",
                        &mut modal.query,
                        "Search folders",
                    );
                    // Keep the filter field focused so typing always lands there
                    // (palette-style). Arrows/⌘N still work via early consume.
                    if modal.focus_search || !search_resp.has_focus() {
                        search_resp.request_focus();
                        modal.focus_search = false;
                    }

                    if let Some(sel) = modal.selected {
                        ui.add_space(8.0);
                        let cap = if sel == root_id {
                            "Home".to_string()
                        } else {
                            files.path(sel)
                        };
                        ui.label(RichText::new(cap).size(12.0).color(t.text_muted()));
                    }
                    ui.add_space(10.0);

                    // macOS overlay scrollbar (same as file tree / sidebar).
                    crate::widgets::scroll_overlay::with_overlay_scroll(
                        ui,
                        Id::new("move_folder_overlay_scroll"),
                        |ui| {
                        let out = ScrollArea::vertical()
                            .id_salt("move_folder_list")
                            .max_height(MOVE_LIST_H)
                            .min_scrolled_height(MOVE_LIST_H)
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                // Packed like the file tree — no egui inter-row gap.
                                ui.spacing_mut().item_spacing.y = 0.0;
                                if searching {
                                    if visible.is_empty() {
                                        sheet_empty_row(ui, t, "No folders match.");
                                    } else {
                                        for (id, depth) in &visible {
                                            let _ = depth;
                                            let valid = is_valid_dest(
                                                files,
                                                &source_ids,
                                                *id,
                                                &forbidden,
                                            );
                                            let file = files.get_by_id(*id);
                                            let name = file
                                                .map(|f| {
                                                    if f.is_root() {
                                                        "Home".into()
                                                    } else {
                                                        f.name.clone()
                                                    }
                                                })
                                                .unwrap_or_default();
                                            let path = files.path(*id);
                                            let shared =
                                                file.is_some_and(|f| !f.shares.is_empty());
                                            let is_sel = modal.selected == Some(*id);
                                            let resp = tree_chrome::search_folder_row(
                                                ui,
                                                t,
                                                Id::new(("move_hit", *id)),
                                                &name,
                                                &path,
                                                is_sel,
                                                valid,
                                                pinned.contains(id),
                                                shared,
                                                shortcut_of(*id),
                                            );
                                            if is_sel && modal.scroll_sel_into_view {
                                                resp.scroll_to_me(None);
                                                modal.scroll_sel_into_view = false;
                                            }
                                            if resp.clicked() && valid {
                                                modal.selected = Some(*id);
                                                expand_ancestors(modal, files, *id);
                                                modal.query.clear();
                                                // Keep this pick when leaving search
                                                // (don't treat clear as "dismiss suggestion").
                                                modal.was_searching = false;
                                                modal.scroll_sel_into_view = false;
                                            }
                                        }
                                    }
                                } else {
                                    paint_tree(
                                        ui,
                                        t,
                                        files,
                                        modal,
                                        root_id,
                                        0,
                                        &forbidden,
                                        pinned,
                                        &shortcut_of,
                                    );
                                }
                            });
                        ((), out.state.offset.y)
                    });

                    let dest = modal
                        .selected
                        .filter(|s| is_valid_dest(files, &source_ids, *s, &forbidden));
                    let (label, can) = modal.primary_label(files, dest);
                    ui.add_space(SHEET_FOOTER_GAP);
                    let foot = sheet_footer(ui, t, &label, can, SheetFooterOpts::default());
                    if foot.cancel {
                        outcome = MoveOutcome::Closed;
                    }
                    if foot.primary {
                        if let Some(parent) = dest {
                            outcome = MoveOutcome::Confirm { parent };
                        }
                    }
            });
        });

    modal.sheet_rect = sheet.response.rect;

    // Backdrop is a second line of defense (same-frame hit target above the
    // editor). Early path already dismissed using last frame's rect.
    if modal.suppress_outside_frames > 0 {
        modal.suppress_outside_frames -= 1;
    } else if backdrop_clicked {
        outcome = MoveOutcome::Closed;
    }
    // Escape is handled in `handle_move_keyboard` (early, before workspace).
    outcome
}

/// Keep selection valid. In search mode, auto-highlight the first hit so Enter
/// works immediately. Clearing the filter (including select-all → backspace)
/// drops that suggestion so tree mode starts clean again.
fn maintain_move_selection(modal: &mut MoveModal, nav: &[Uuid], searching: bool) {
    if searching {
        if modal.selected.is_some_and(|s| !nav.contains(&s)) {
            modal.selected = None;
        }
        if modal.selected.is_none() {
            if let Some(&id) = nav.first() {
                modal.selected = Some(id);
            }
        }
    } else if modal.was_searching {
        // Left search this frame — clear auto-suggestion / path caption.
        modal.selected = None;
    } else if modal.selected.is_some_and(|s| !nav.contains(&s)) {
        modal.selected = None;
    }
    modal.was_searching = searching;
}

/// Visible folders in tree paint order: `(id, depth)`.
fn collect_tree_visible(
    files: &impl FilesExt, expanded: &HashSet<Uuid>, root: Uuid,
) -> Vec<(Uuid, usize)> {
    let mut out = Vec::new();
    fn walk(
        files: &impl FilesExt,
        expanded: &HashSet<Uuid>,
        id: Uuid,
        depth: usize,
        out: &mut Vec<(Uuid, usize)>,
    ) {
        let Some(file) = files.get_by_id(id) else {
            return;
        };
        if !file.is_folder() {
            return;
        }
        out.push((id, depth));
        if expanded.contains(&id) {
            let mut kids: Vec<Uuid> = files
                .children(id)
                .into_iter()
                .filter(|c| c.is_folder())
                .map(|c| c.id)
                .collect();
            kids.sort_by(|a, b| {
                let an = files.get_by_id(*a).map(|f| f.name.to_lowercase());
                let bn = files.get_by_id(*b).map(|f| f.name.to_lowercase());
                an.cmp(&bn).then(a.cmp(b))
            });
            for kid in kids {
                walk(files, expanded, kid, depth + 1, out);
            }
        }
    }
    walk(files, expanded, root, 0, &mut out);
    out
}

fn collect_search_hits(files: &impl FilesExt, q: &str) -> Vec<(Uuid, usize)> {
    let mut hits: Vec<(Uuid, String)> = files
        .iter_files()
        .filter(|f| f.is_folder())
        .filter(|f| f.name.to_lowercase().contains(q))
        .map(|f| (f.id, f.name.clone()))
        .collect();
    hits.sort_by(|a, b| a.1.to_lowercase().cmp(&b.1.to_lowercase()).then(a.0.cmp(&b.0)));
    hits.into_iter().map(|(id, _)| (id, 0)).collect()
}

#[allow(clippy::too_many_arguments)]
fn paint_tree(
    ui: &mut Ui,
    t: &Tokens,
    files: &impl FilesExt,
    modal: &mut MoveModal,
    id: Uuid,
    depth: usize,
    forbidden: &HashSet<Uuid>,
    pinned: &HashSet<Uuid>,
    shortcut_of: &dyn Fn(Uuid) -> Option<u8>,
) {
    let Some(file) = files.get_by_id(id) else {
        return;
    };
    if !file.is_folder() {
        return;
    }

    let valid = is_valid_dest(files, modal.source_ids(), id, forbidden);
    let kids: Vec<Uuid> = {
        let mut k: Vec<Uuid> = files
            .children(id)
            .into_iter()
            .filter(|c| c.is_folder())
            .map(|c| c.id)
            .collect();
        k.sort_by(|a, b| {
            let an = files.get_by_id(*a).map(|f| f.name.to_lowercase());
            let bn = files.get_by_id(*b).map(|f| f.name.to_lowercase());
            an.cmp(&bn).then(a.cmp(b))
        });
        k
    };
    let has_kids = !kids.is_empty();
    let open = modal.expanded.contains(&id);

    let vis = FolderRowVisual {
        depth,
        expanded: open,
        selected: modal.selected == Some(id),
        enabled: valid,
        is_root: file.is_root(),
        pinned: pinned.contains(&id),
        shared: !file.shares.is_empty(),
        shortcut: shortcut_of(id),
    };
    let resp = tree_chrome::folder_row(ui, t, Id::new(("move_row", id)), &file.name, vis);
    if vis.selected && modal.scroll_sel_into_view {
        // Only after keyboard highlight change — not while the pointer scrolls.
        resp.scroll_to_me(None);
        modal.scroll_sel_into_view = false;
    }
    if resp.clicked() {
        if has_kids {
            if open {
                modal.expanded.remove(&id);
            } else {
                modal.expanded.insert(id);
            }
        }
        if valid {
            modal.selected = Some(id);
            modal.scroll_sel_into_view = false;
        }
    }

    if modal.expanded.contains(&id) {
        for kid in kids {
            paint_tree(ui, t, files, modal, kid, depth + 1, forbidden, pinned, shortcut_of);
        }
    }
}

fn forbidden_targets(files: &impl FilesExt, ids: &[Uuid]) -> HashSet<Uuid> {
    let mut bad = HashSet::new();
    for &id in ids {
        bad.insert(id);
        let mut stack = vec![id];
        while let Some(cur) = stack.pop() {
            for child in files.children(cur) {
                bad.insert(child.id);
                if child.is_folder() {
                    stack.push(child.id);
                }
            }
        }
    }
    bad
}

/// Valid destination: folder, not forbidden.
/// - **Move**: at least one source would actually change parent.
/// - **Accept share** (`ids` empty): any own-tree folder is fine.
fn is_valid_dest(
    files: &impl FilesExt, ids: &[Uuid], parent: Uuid, forbidden: &HashSet<Uuid>,
) -> bool {
    if forbidden.contains(&parent) {
        return false;
    }
    if !files.get_by_id(parent).is_some_and(|f| f.is_folder()) {
        return false;
    }
    // No move sources (Add to files) — every folder is a valid link parent.
    if ids.is_empty() {
        return true;
    }
    ids.iter().any(|id| {
        files
            .get_by_id(*id)
            .is_some_and(|f| f.parent != parent && f.id != parent)
    })
}

fn expand_ancestors(modal: &mut MoveModal, files: &impl FilesExt, id: Uuid) {
    for a in files.ancestors(id) {
        modal.expanded.insert(a);
    }
    modal.expanded.insert(id);
}
