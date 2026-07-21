//! Settings shell for egui desktop.
//!
//! System Settings–style: dim + wide modal, left category rail with search,
//! canvas body with page title + inset grouped lists. Instant apply for
//! toggles; confirm for logout / delete / cancel subscription.

use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use egui::{
    Align, Area, Color32, CornerRadius, FontId, Frame, Id, Key, Layout, Margin, Modifiers, Order,
    Rect, RichText, ScrollArea, Sense, Stroke, TextEdit, Ui, UiBuilder, pos2, vec2,
};
use lb::blocking::Lb;
use lb::model::api::{PaymentMethod, StripeAccountTier};
use lb::service::usage::UsageMetrics;
use workspace_rs::show::InputStateExt;
use workspace_rs::theme::palette_v2::ThemeExt;

use crate::settings::{self, Settings, ThemeMode};
use crate::theme::icons;
use crate::theme::tokens::Tokens;
use crate::widgets::button::Button;
use crate::widgets::modals::show_modal_dim;
use crate::widgets::search_field;
use crate::widgets::sync_footer::AccountInfo;

const SETTINGS_W: f32 = 760.0;
const SETTINGS_H: f32 = 520.0;
const SHELL_R: u8 = 14;
const TITLE_H: f32 = 48.0;
const RAIL_W: f32 = 188.0;
const BODY_PAD_X: f32 = 28.0;
const BODY_PAD_Y: f32 = 22.0;
const SECTION_GAP: f32 = 22.0;
/// Form row height. Trailing chips sit with equal air top / bottom / right.
const ROW_H: f32 = 40.0;
const ROW_PAD_L: f32 = 14.0;
/// Matches vertical inset around a `TRAIL_H` chip: (ROW_H − TRAIL_H) / 2.
const ROW_PAD_R: f32 = 9.0;
const TRAIL_H: f32 = 22.0;
const GROUP_RADIUS: u8 = 10;
const BTN_H: f32 = 28.0;
const FLASH_SECS: f32 = 2.4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingsCategory {
    Account,
    Plan,
    Appearance,
    Editor,
    Privacy,
    Debug,
}

impl SettingsCategory {
    pub const ALL: [Self; 6] = [
        Self::Account,
        Self::Plan,
        Self::Appearance,
        Self::Editor,
        Self::Privacy,
        Self::Debug,
    ];

    fn title(self) -> &'static str {
        match self {
            Self::Account => "Account",
            Self::Plan => "Plan & storage",
            Self::Appearance => "Appearance",
            Self::Editor => "Editor",
            Self::Privacy => "Privacy",
            Self::Debug => "Debug",
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Self::Account => icons::USER,
            Self::Plan => icons::CLOUD_ARROW_UP,
            Self::Appearance => icons::PAINT_BRUSH,
            Self::Editor => icons::MARKDOWN_LOGO,
            Self::Privacy => icons::WARNING_CIRCLE,
            Self::Debug => icons::CODE,
        }
    }

    fn keywords(self) -> &'static [&'static str] {
        match self {
            Self::Account => &[
                "account",
                "username",
                "key",
                "phrase",
                "logout",
                "log out",
                "export",
                "identity",
            ],
            Self::Plan => &[
                "plan",
                "premium",
                "free",
                "usage",
                "storage",
                "billing",
                "subscribe",
                "cancel",
                "upgrade",
                "tier",
                "stripe",
                "card",
            ],
            Self::Appearance => &[
                "appearance",
                "theme",
                "dark",
                "light",
                "system",
                "color",
                "palette",
            ],
            Self::Editor => &[
                "editor",
                "link",
                "preview",
                "markdown",
                "privacy",
                "fetch",
                "contact",
            ],
            Self::Privacy => &[
                "privacy",
                "terms",
                "tos",
                "policy",
                "legal",
                "delete account",
            ],
            Self::Debug => &["debug", "server", "api", "url", "version", "info"],
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) enum ConfirmKind {
    Logout,
    DeleteAccount,
    CancelSubscription,
}

/// Data the shell loads once when opening settings (or refreshes on demand).
#[derive(Clone, Debug, Default)]
pub struct SettingsData {
    pub username: String,
    pub api_url: String,
    pub usage: Option<UsageMetrics>,
    pub plan: Option<AccountInfo>,
    pub last_synced: Option<String>,
    pub contact_linked_sites: bool,
    pub writeable_path: String,
    pub theme_mode: ThemeMode,
    pub theme_name: String,
}

/// In-settings Stripe upgrade: paywall (decide) → checkout (pay) → result.
struct UpgradeFlow {
    stage: UpgradeStage,
    number: String,
    exp_month: String,
    exp_year: String,
    cvc: String,
    error: Option<String>,
    last4: Option<String>,
    done: Option<Result<(), String>>,
    result_rx: Option<Receiver<Result<AccountInfo, String>>>,
    /// Internal navigation flags (cleared after handling).
    nav: UpgradeNav,
    /// False until one full frame has painted this stage — avoids the
    /// Settings “Upgrade” click also hitting the paywall primary CTA that
    /// appears under the same pointer on the next frame.
    interact_armed: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum UpgradeNav {
    #[default]
    None,
    Dismiss,
    ToCheckout,
    ToPaywall,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum UpgradeStage {
    /// Value prop — no card fields yet.
    Paywall,
    /// Card entry + trust strip.
    Checkout,
    /// In-flight / success / error.
    Paying,
}

pub struct SettingsModal {
    pub category: SettingsCategory,
    pub search: String,
    pub data: SettingsData,
    pub phrase: Option<String>,
    pub phrase_error: Option<String>,
    pub private_key: Option<String>,
    pub keys_error: Option<String>,
    pub confirm: Option<ConfirmKind>,
    pub flash: Option<String>,
    pub error: Option<String>,
    focus_search: bool,
    flash_at: Option<Instant>,
    upgrade: Option<UpgradeFlow>,
}

pub enum SettingsOutcome {
    Open,
    Closed,
    /// Wipe local data and exit (matches mobile logoutAndExit).
    Logout { writeable_path: String },
    DeleteAccount,
    CancelSubscription,
    /// Persist editor/privacy prefs.
    SavePrefs { contact_linked_sites: bool },
    /// Persist + apply appearance.
    SaveAppearance {
        theme_mode: ThemeMode,
        theme_name: String,
    },
    /// Stripe upgrade succeeded — refresh footer account standing.
    PlanRefreshed(AccountInfo),
}

impl SettingsModal {
    pub fn new(data: SettingsData) -> Self {
        Self {
            category: SettingsCategory::Account,
            search: String::new(),
            data,
            phrase: None,
            phrase_error: None,
            private_key: None,
            keys_error: None,
            confirm: None,
            flash: None,
            error: None,
            focus_search: true,
            flash_at: None,
            upgrade: None,
        }
    }

    pub(crate) fn set_flash(&mut self, msg: impl Into<String>) {
        self.flash = Some(msg.into());
        self.flash_at = Some(Instant::now());
        self.error = None;
    }

    fn tick_flash(&mut self, ctx: &egui::Context) {
        if let Some(at) = self.flash_at {
            let left = Duration::from_secs_f32(FLASH_SECS).saturating_sub(at.elapsed());
            if left.is_zero() {
                self.flash = None;
                self.flash_at = None;
                ctx.request_repaint();
            } else {
                // Keep the banner alive until it expires.
                ctx.request_repaint_after(left.min(Duration::from_millis(200)));
            }
        }
    }

    fn search_matches(&self, cat: SettingsCategory) -> bool {
        let q = self.search.trim().to_lowercase();
        if q.is_empty() {
            return true;
        }
        cat.title().to_lowercase().contains(&q)
            || cat
                .keywords()
                .iter()
                .any(|k| k.contains(q.as_str()) || q.contains(k))
    }

    fn visible_categories(&self) -> Vec<SettingsCategory> {
        SettingsCategory::ALL
            .into_iter()
            .filter(|c| self.search_matches(*c))
            .collect()
    }
}

pub fn settings_layer_id() -> egui::LayerId {
    egui::LayerId::new(Order::Foreground, Id::new("lb_settings_modal"))
}

/// Load snapshot for the settings UI (blocking; call off first paint if needed).
pub fn load_settings_data(core: &Lb, settings: &Settings) -> SettingsData {
    let account = core.get_account().ok();
    let username = account
        .as_ref()
        .map(|a| a.username.clone())
        .unwrap_or_default();
    let api_url = account
        .as_ref()
        .map(|a| a.api_url.clone())
        .unwrap_or_default();
    let usage = core.get_usage().ok();
    let cap = usage.as_ref().map(|u| u.data_cap.exact);
    let plan = core
        .get_subscription_info()
        .ok()
        .map(|info| AccountInfo::from_subscription_and_cap(info, cap));
    let last_synced = core.get_last_synced_human_string().ok();
    let writeable_path = core.get_config().writeable_path.clone();
    SettingsData {
        username,
        api_url,
        usage,
        plan,
        last_synced,
        contact_linked_sites: settings.contact_linked_sites,
        writeable_path,
        theme_mode: settings.theme_mode,
        theme_name: settings.theme_name.clone(),
    }
}

/// Keyboard: Esc closes (or cancels confirm). ↑/↓ move categories when search
/// is empty. Call before workspace.
pub fn handle_settings_keyboard(ctx: &egui::Context, modal: &mut SettingsModal) -> bool {
    let mut close = false;
    ctx.input_mut(|i| {
        if i.consume_key_exact(Modifiers::NONE, Key::Escape) {
            if modal.confirm.is_some() {
                modal.confirm = None;
            } else {
                close = true;
            }
        }
        // Category nav when not typing a multi-word filter intent — only if
        // search field isn't the focus sink eating arrows (Glyphon does).
        // Use ⌥↑/↓ so search can still use plain arrows for cursor.
        if modal.confirm.is_none() {
            let vis = modal.visible_categories();
            if !vis.is_empty() {
                if i.consume_key_exact(Modifiers::ALT, Key::ArrowDown) {
                    let idx = vis
                        .iter()
                        .position(|c| *c == modal.category)
                        .unwrap_or(0);
                    modal.category = vis[(idx + 1).min(vis.len() - 1)];
                }
                if i.consume_key_exact(Modifiers::ALT, Key::ArrowUp) {
                    let idx = vis
                        .iter()
                        .position(|c| *c == modal.category)
                        .unwrap_or(0);
                    modal.category = vis[idx.saturating_sub(1)];
                }
            }
        }
    });
    close
}

pub fn show_settings(
    ctx: &egui::Context, t: &Tokens, modal: &mut SettingsModal, core: &Lb,
) -> SettingsOutcome {
    modal.tick_flash(ctx);
    let mut outcome = SettingsOutcome::Open;
    let screen = ctx.screen_rect();
    let w = SETTINGS_W.min(screen.width() - 48.0).max(520.0);
    let h = SETTINGS_H.min(screen.height() - 48.0).max(380.0);

    if show_modal_dim(ctx, Id::new("lb_modal_dim_settings"), settings_layer_id()) {
        outcome = SettingsOutcome::Closed;
    }

    Area::new(Id::new("lb_settings_modal"))
        .order(Order::Foreground)
        .fixed_pos(screen.center() - vec2(w / 2.0, h / 2.0))
        .constrain(true)
        .fade_in(false)
        .show(ctx, |ui| {
            // Kill default spacing for the whole shell so regions/hairlines don't drift.
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);

            Frame::new()
                .fill(t.surface())
                .stroke(Stroke::new(1.0, t.line()))
                .corner_radius(CornerRadius::same(SHELL_R))
                .inner_margin(Margin::ZERO)
                .shadow(egui::Shadow {
                    offset: [0, 8],
                    blur: 32,
                    spread: 0,
                    color: Color32::from_black_alpha(40),
                })
                .show(ui, |ui| {
                    // Frame content_ui inherits parent spacing — zero again.
                    ui.spacing_mut().item_spacing = vec2(0.0, 0.0);

                    // One exact panel. All chrome geometry is derived from this rect —
                    // no horizontal/vertical stacks that inject item_spacing.
                    let (panel, _) =
                        ui.allocate_exact_size(vec2(w, h), Sense::hover());

                    let title_r =
                        Rect::from_min_size(panel.min, vec2(panel.width(), TITLE_H));
                    let body_r = Rect::from_min_max(
                        pos2(panel.min.x, title_r.bottom()),
                        panel.max,
                    );
                    let rail_r =
                        Rect::from_min_size(body_r.min, vec2(RAIL_W, body_r.height()));
                    let content_r =
                        Rect::from_min_max(pos2(rail_r.right(), body_r.min.y), body_r.max);

                    // Content fill: only the SE corner is rounded (matches shell).
                    // Rail + title stay surface via the outer Frame fill.
                    ui.painter().rect_filled(
                        content_r,
                        CornerRadius {
                            nw: 0,
                            ne: 0,
                            sw: 0,
                            se: SHELL_R,
                        },
                        t.canvas(),
                    );

                    // Hairlines on the exact region boundaries (not layout widgets).
                    let line = Stroke::new(1.0, t.line());
                    ui.painter().hline(panel.x_range(), title_r.bottom(), line);
                    ui.painter().vline(rail_r.right(), body_r.y_range(), line);

                    // ── Title ────────────────────────────────────────────
                    {
                        let mut title_ui = ui.new_child(
                            UiBuilder::new()
                                .id_salt("settings_title")
                                .max_rect(title_r)
                                .layout(Layout::left_to_right(Align::Center)),
                        );
                        title_ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                        title_ui.set_clip_rect(title_r);
                        title_ui.add_space(18.0);
                        title_ui.label(
                            RichText::new("Settings")
                                .size(15.0)
                                .strong()
                                .color(t.fg()),
                        );
                        title_ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                            ui.add_space(12.0);
                            if close_btn(ui, t).clicked() {
                                outcome = SettingsOutcome::Closed;
                            }
                        });
                    }

                    // ── Rail ─────────────────────────────────────────────
                    {
                        let mut rail_ui = ui.new_child(
                            UiBuilder::new()
                                .id_salt("settings_rail")
                                .max_rect(rail_r)
                                .layout(Layout::top_down(Align::Min)),
                        );
                        rail_ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                        rail_ui.set_clip_rect(rail_r);
                        rail_ui.add_space(12.0);
                        rail_ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                            ui.add_space(12.0);
                            ui.vertical(|ui| {
                                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                                ui.set_width((RAIL_W - 24.0).max(40.0));
                                let search_resp = search_field::show(
                                    ui,
                                    t,
                                    "settings_search",
                                    &mut modal.search,
                                    "Search",
                                );
                                if modal.focus_search {
                                    search_resp.request_focus();
                                    modal.focus_search = false;
                                }
                                ui.add_space(12.0);

                                let visible = modal.visible_categories();
                                if visible.is_empty() {
                                    ui.add_space(8.0);
                                    ui.label(
                                        RichText::new("No matches")
                                            .size(12.0)
                                            .color(t.text_muted()),
                                    );
                                }
                                for (i, cat) in visible.iter().enumerate() {
                                    if i > 0 {
                                        ui.add_space(2.0);
                                    }
                                    let active = modal.category == *cat;
                                    if category_row(ui, t, *cat, active) {
                                        modal.category = *cat;
                                    }
                                }

                                if !modal.search.trim().is_empty() {
                                    let vis = modal.visible_categories();
                                    if !vis.is_empty() && !vis.contains(&modal.category) {
                                        modal.category = vis[0];
                                    }
                                }
                            });
                            ui.add_space(12.0);
                        });
                    }

                    // ── Content ──────────────────────────────────────────
                    {
                        let mut content_ui = ui.new_child(
                            UiBuilder::new()
                                .id_salt("settings_content")
                                .max_rect(content_r)
                                .layout(Layout::top_down(Align::Min)),
                        );
                        content_ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                        content_ui.set_clip_rect(content_r);
                        // Scroll fills the exact content rect.
                        ScrollArea::vertical()
                            .id_salt("settings_body")
                            .auto_shrink([false, false])
                            .show(&mut content_ui, |ui| {
                                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                                ui.set_min_width(content_r.width());
                                ui.set_max_width(content_r.width());

                                ui.add_space(BODY_PAD_Y);
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                                    ui.add_space(BODY_PAD_X);
                                    ui.vertical(|ui| {
                                        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                                        let max_w =
                                            (content_r.width() - BODY_PAD_X * 2.0).max(200.0);
                                        ui.set_width(max_w);

                                        page_header(ui, t, modal.category.title());

                                        if let Some(flash) = modal.flash.clone() {
                                            ui.add_space(10.0);
                                            banner(ui, t, &flash, false);
                                        }
                                        if let Some(err) = modal.error.clone() {
                                            ui.add_space(10.0);
                                            banner(ui, t, &err, true);
                                        }

                                        ui.add_space(18.0);

                                        let out = match modal.category {
                                            SettingsCategory::Account => {
                                                show_account(ui, t, modal, core)
                                            }
                                            SettingsCategory::Plan => {
                                                show_plan(ui, t, modal, core)
                                            }
                                            SettingsCategory::Appearance => {
                                                show_appearance(ui, t, modal)
                                            }
                                            SettingsCategory::Editor => {
                                                show_editor(ui, t, modal)
                                            }
                                            SettingsCategory::Privacy => {
                                                show_privacy(ui, t, modal)
                                            }
                                            SettingsCategory::Debug => {
                                                show_debug(ui, t, modal, core)
                                            }
                                        };
                                        if !matches!(out, SettingsOutcome::Open) {
                                            outcome = out;
                                        }
                                    });
                                    ui.add_space(BODY_PAD_X);
                                });
                                ui.add_space(BODY_PAD_Y + 8.0);
                            });
                    }

                    if let Some(kind) = modal.confirm.clone() {
                        if let Some(o) =
                            show_confirm_overlay(ui, t, modal, &kind, panel)
                        {
                            outcome = o;
                        }
                    }
                });
        });

    outcome
}

// ── Chrome primitives ────────────────────────────────────────────────────────

/// Sheet dismiss — plain ×, ghost wash, ink firms on hover (toolbar language).
fn close_btn(ui: &mut Ui, t: &Tokens) -> egui::Response {
    let size = 28.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(size, size), Sense::click());
    let hover = ui.ctx().animate_bool(resp.id, resp.hovered());
    let press = resp.is_pointer_button_down_on();
    // Soft resting wash so it’s findable; firms a touch on hover/press.
    let wash = if press {
        0.75
    } else {
        0.35 + 0.45 * hover
    };
    ui.painter().rect_filled(
        rect,
        7.0,
        t.canvas().lerp_to_gamma(t.surface_raised(), wash),
    );
    let ink = t.text_muted().lerp_to_gamma(t.fg(), 0.45 + 0.55 * hover);
    let g = ui
        .painter()
        .layout_no_wrap(icons::X.into(), icons::font(16.0), ink);
    ui.painter()
        .galley(rect.center() - g.size() / 2.0, g, ink);
    resp
}

fn category_row(ui: &mut Ui, t: &Tokens, cat: SettingsCategory, active: bool) -> bool {
    // Stable id per category (not sequential auto-ids) so hover animation
    // doesn't thrash when the rail re-layouts or search filters rows.
    let id = ui.id().with("settings_cat").with(cat as u8);
    let size = vec2(ui.available_width(), 34.0);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let resp = ui.interact(rect, id, Sense::click());

    // Instant hover — no animate_bool repaint loop (that read as flicker on
    // this quiet rail). Same language as soft nav: wash + ink hierarchy.
    let hovered = resp.hovered();
    if active {
        ui.painter().rect_filled(
            rect,
            8.0,
            t.canvas().lerp_to_gamma(t.fg(), if hovered { 0.09 } else { 0.07 }),
        );
    } else if hovered {
        ui.painter().rect_filled(
            rect,
            8.0,
            t.canvas().lerp_to_gamma(t.surface_raised(), 0.85),
        );
    }
    let ink = if active || hovered {
        t.fg()
    } else {
        t.text_muted()
    };
    let ig = ui
        .painter()
        .layout_no_wrap(cat.icon().into(), icons::font(15.0), ink);
    let tg = ui.painter().layout_no_wrap(
        cat.title().into(),
        FontId::proportional(13.0),
        ink,
    );
    let cy = rect.center().y;
    let mut x = rect.left() + 10.0;
    ui.painter()
        .galley(pos2(x, cy - ig.size().y / 2.0), ig, ink);
    x += 24.0;
    ui.painter()
        .galley(pos2(x, cy - tg.size().y / 2.0), tg, ink);
    resp.clicked()
}

fn page_header(ui: &mut Ui, t: &Tokens, title: &str) {
    ui.label(
        RichText::new(title)
            .size(22.0)
            .strong()
            .color(t.fg()),
    );
}

fn banner(ui: &mut Ui, t: &Tokens, text: &str, danger: bool) {
    let (fill, ink) = if danger {
        (
            t.danger().gamma_multiply(0.12),
            t.danger(),
        )
    } else {
        (
            t.accent().gamma_multiply(0.12),
            t.accent(),
        )
    };
    Frame::new()
        .fill(fill)
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::symmetric(12, 8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(text).size(12.5).color(ink));
        });
}

/// Inset grouped list — System Settings card: surface on canvas, soft stroke.
fn group(ui: &mut Ui, t: &Tokens, add: impl FnOnce(&mut Ui)) {
    Frame::new()
        .fill(t.surface())
        .stroke(Stroke::new(1.0, t.line().gamma_multiply(0.85)))
        .corner_radius(CornerRadius::same(GROUP_RADIUS))
        .inner_margin(Margin::symmetric(0, 2))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            ui.set_width(ui.available_width());
            add(ui);
        });
}

/// Section eyebrow above a group (small caps-ish muted).
fn section_label(ui: &mut Ui, t: &Tokens, title: &str) {
    ui.label(
        RichText::new(title.to_uppercase())
            .size(11.0)
            .strong()
            .color(t.text_muted()),
    );
    ui.add_space(7.0);
}

/// Footnote under a group.
fn footnote(ui: &mut Ui, t: &Tokens, text: &str) {
    ui.add_space(7.0);
    ui.label(
        RichText::new(text)
            .size(11.5)
            .color(t.text_muted()),
    );
}

/// Standard label | trailing control row.
fn form_row(ui: &mut Ui, t: &Tokens, label: &str, trailing: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        ui.set_min_height(ROW_H);
        ui.set_max_height(ROW_H);
        ui.add_space(ROW_PAD_L);
        ui.label(RichText::new(label).size(13.5).color(t.fg()));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            // Same air as top/bottom around a TRAIL_H chip in this row.
            ui.add_space(ROW_PAD_R);
            trailing(ui);
        });
    });
}

fn value_text(ui: &mut Ui, t: &Tokens, value: &str) {
    ui.label(
        RichText::new(value)
            .size(13.5)
            .color(t.text_muted()),
    );
}

fn kv_row(ui: &mut Ui, t: &Tokens, key: &str, value: &str) {
    form_row(ui, t, key, |ui| value_text(ui, t, value));
}

/// Multiline secondary value (path, monospaced secrets).
fn secret_block(ui: &mut Ui, t: &Tokens, text: &str) {
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        ui.add_space(ROW_PAD_L);
        Frame::new()
            .fill(t.canvas())
            .stroke(Stroke::new(1.0, t.line()))
            .corner_radius(CornerRadius::same(8))
            .inner_margin(Margin::symmetric(10, 8))
            .show(ui, |ui| {
                ui.set_width((ui.available_width() - ROW_PAD_R).max(40.0));
                ui.add(
                    egui::Label::new(
                        RichText::new(text)
                            .size(12.0)
                            .monospace()
                            .color(t.fg()),
                    )
                    .wrap(),
                );
            });
    });
    ui.add_space(6.0);
}

/// Compact in-row secondary button.
fn row_btn<'a>(t: &'a Tokens, label: impl Into<String>) -> Button<'a> {
    Button::secondary(t, label).height(BTN_H)
}

fn row_btn_danger<'a>(t: &'a Tokens, label: impl Into<String>) -> Button<'a> {
    Button::secondary(t, label).danger().height(BTN_H)
}

fn row_btn_quiet<'a>(t: &'a Tokens, label: impl Into<String>) -> Button<'a> {
    Button::quiet(t, label).height(BTN_H)
}

/// Soft iOS-style toggle.
fn toggle(ui: &mut Ui, t: &Tokens, on: &mut bool) -> bool {
    let w = 38.0;
    let h = 22.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(w, h), Sense::click());
    if resp.clicked() {
        *on = !*on;
    }
    let anim = ui.ctx().animate_bool_with_time_and_easing(
        resp.id.with("on"),
        *on,
        0.14,
        egui::emath::easing::cubic_out,
    );
    let track = t.line().lerp_to_gamma(t.accent(), anim);
    let track = if *on {
        track
    } else {
        t.surface_raised().lerp_to_gamma(t.line(), 0.55)
    };
    ui.painter()
        .rect_filled(rect, h / 2.0, track);
    let kn = h - 4.0;
    let x = rect.left() + 2.0 + (rect.width() - kn - 4.0) * anim;
    let knob = Rect::from_min_size(pos2(x, rect.top() + 2.0), vec2(kn, kn));
    ui.painter().circle_filled(
        knob.center(),
        kn / 2.0,
        Color32::WHITE.lerp_to_gamma(t.canvas(), 0.05),
    );
    // Soft knob edge
    ui.painter().circle_stroke(
        knob.center(),
        kn / 2.0,
        Stroke::new(0.5, Color32::from_black_alpha(20)),
    );
    resp.clicked()
}

/// Status capsule — fixed height so top/bottom/right air in the form row matches.
fn tier_pill(ui: &mut Ui, t: &Tokens, label: &str, premium: bool) {
    let fill = if premium {
        t.accent().gamma_multiply(0.14)
    } else {
        t.canvas().lerp_to_gamma(t.line(), 0.45)
    };
    let ink = if premium { t.accent() } else { t.text_muted() };
    trail_capsule(ui, label, fill, ink, false);
}

/// Compact filled action capsule (Upgrade) — same geometry as [`tier_pill`].
fn trail_action(ui: &mut Ui, t: &Tokens, label: &str) -> egui::Response {
    let fill = t.fg();
    let ink = t.canvas();
    let resp = trail_capsule(ui, label, fill, ink, true);
    let hover = ui.ctx().animate_bool(resp.id, resp.hovered());
    if hover > 0.0 || resp.is_pointer_button_down_on() {
        // Re-paint a slightly softer fill on engage (same rect).
        let rect = resp.rect;
        let engaged = if resp.is_pointer_button_down_on() {
            t.fg().lerp_to_gamma(t.canvas(), 0.16)
        } else {
            t.fg().lerp_to_gamma(t.canvas(), 0.08 * hover)
        };
        ui.painter()
            .rect_filled(rect, rect.height() / 2.0, engaged);
        let g = ui.painter().layout_no_wrap(
            label.into(),
            FontId::proportional(12.0),
            ink,
        );
        ui.painter()
            .galley(rect.center() - g.size() / 2.0, g, ink);
    }
    resp
}

/// Compact danger capsule (Cancel) — wireframe red, same height as status pill.
fn trail_danger(ui: &mut Ui, t: &Tokens, label: &str) -> egui::Response {
    let pad_x = 10.0;
    let font = FontId::proportional(12.0);
    let g = ui
        .painter()
        .layout_no_wrap(label.into(), font.clone(), t.danger());
    let w = (g.size().x + pad_x * 2.0).max(TRAIL_H * 1.6);
    let desired = vec2(w, TRAIL_H);
    let (rect, resp) = ui.allocate_exact_size(desired, Sense::click());
    let hover = ui.ctx().animate_bool(resp.id, resp.hovered());
    let stroke = if hover > 0.5 || resp.is_pointer_button_down_on() {
        t.danger()
    } else {
        t.line()
    };
    let fill = if resp.is_pointer_button_down_on() {
        t.danger().gamma_multiply(0.10)
    } else if hover > 0.0 {
        t.danger().gamma_multiply(0.06 * hover)
    } else {
        Color32::TRANSPARENT
    };
    if fill.a() > 0 {
        ui.painter()
            .rect_filled(rect, rect.height() / 2.0, fill);
    }
    ui.painter().rect_stroke(
        rect,
        rect.height() / 2.0,
        Stroke::new(1.0, stroke),
        egui::StrokeKind::Inside,
    );
    let ink = t.danger();
    let g = ui.painter().layout_no_wrap(label.into(), font, ink);
    ui.painter()
        .galley(rect.center() - g.size() / 2.0, g, ink);
    resp
}

fn trail_capsule(
    ui: &mut Ui, label: &str, fill: Color32, ink: Color32, clickable: bool,
) -> egui::Response {
    let pad_x = 10.0;
    let font = FontId::proportional(12.0);
    let g = ui
        .painter()
        .layout_no_wrap(label.into(), font.clone(), ink);
    let w = (g.size().x + pad_x * 2.0).max(TRAIL_H * 1.6);
    let desired = vec2(w, TRAIL_H);
    let sense = if clickable {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, resp) = ui.allocate_exact_size(desired, sense);
    ui.painter()
        .rect_filled(rect, rect.height() / 2.0, fill);
    let g = ui.painter().layout_no_wrap(label.into(), font, ink);
    ui.painter()
        .galley(rect.center() - g.size() / 2.0, g, ink);
    resp
}

// ── Sections ─────────────────────────────────────────────────────────────────

fn show_account(
    ui: &mut Ui, t: &Tokens, modal: &mut SettingsModal, core: &Lb,
) -> SettingsOutcome {
    // Identity hero
    Frame::new()
        .fill(t.surface())
        .stroke(Stroke::new(1.0, t.line().gamma_multiply(0.85)))
        .corner_radius(CornerRadius::same(GROUP_RADIUS))
        .inner_margin(Margin::symmetric(16, 14))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                // Avatar circle with initial
                let initial = modal
                    .data
                    .username
                    .chars()
                    .next()
                    .map(|c| c.to_uppercase().to_string())
                    .unwrap_or_else(|| "?".into());
                let av = 44.0;
                let (ar, _) = ui.allocate_exact_size(vec2(av, av), Sense::hover());
                ui.painter().circle_filled(
                    ar.center(),
                    av / 2.0,
                    t.canvas().lerp_to_gamma(t.accent(), 0.18),
                );
                let ig = ui.painter().layout_no_wrap(
                    initial,
                    FontId::proportional(18.0),
                    t.accent(),
                );
                ui.painter().galley(
                    ar.center() - ig.size() / 2.0,
                    ig,
                    t.accent(),
                );

                ui.add_space(12.0);
                ui.vertical(|ui| {
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(&modal.data.username)
                            .size(17.0)
                            .strong()
                            .color(t.fg()),
                    );
                    ui.add_space(2.0);
                    let tier = modal
                        .data
                        .plan
                        .as_ref()
                        .map(|p| p.tier.label())
                        .unwrap_or("…");
                    ui.label(
                        RichText::new(format!("{tier} plan"))
                            .size(12.5)
                            .color(t.text_muted()),
                    );
                });
            });
        });

    ui.add_space(SECTION_GAP);
    section_label(ui, t, "Account keys");
    group(ui, t, |ui| {
        form_row(ui, t, "Recovery phrase", |ui| {
            // right-to-left: first painted sits on the right (primary).
            if modal.phrase.is_some() {
                if row_btn(t, "Copy").show(ui).clicked() {
                    if let Some(p) = &modal.phrase {
                        ui.ctx().copy_text(p.clone());
                        modal.set_flash("Phrase copied");
                    }
                }
                ui.add_space(6.0);
                if row_btn_quiet(t, "Hide").show(ui).clicked() {
                    modal.phrase = None;
                }
            } else if row_btn(t, "Reveal").show(ui).clicked() {
                match core.export_account_phrase() {
                    Ok(p) => {
                        modal.phrase = Some(p);
                        modal.phrase_error = None;
                    }
                    Err(e) => {
                        modal.phrase = None;
                        modal.phrase_error = Some(format!("{e}"));
                    }
                }
            }
        });
        if let Some(err) = &modal.phrase_error {
            ui.horizontal(|ui| {
                ui.add_space(14.0);
                ui.label(RichText::new(err).size(12.0).color(t.danger()));
            });
            ui.add_space(6.0);
        }
        if let Some(phrase) = modal.phrase.clone() {
            secret_block(ui, t, &phrase);
            ui.horizontal(|ui| {
                ui.add_space(14.0);
                ui.label(
                    RichText::new(
                        "Confirms your identity. Keep it private — it cannot be recovered if lost.",
                    )
                    .size(11.5)
                    .color(t.text_muted()),
                );
            });
            ui.add_space(8.0);
        }

        form_row(ui, t, "Private key", |ui| {
            if modal.private_key.is_some() {
                if row_btn(t, "Copy").show(ui).clicked() {
                    if let Some(k) = &modal.private_key {
                        ui.ctx().copy_text(k.clone());
                        modal.set_flash("Private key copied");
                    }
                }
                ui.add_space(6.0);
                if row_btn_quiet(t, "Hide").show(ui).clicked() {
                    modal.private_key = None;
                }
            } else if row_btn(t, "Reveal").show(ui).clicked() {
                match core.export_account_private_key() {
                    Ok(k) => {
                        modal.private_key = Some(k);
                        modal.keys_error = None;
                    }
                    Err(e) => {
                        modal.private_key = None;
                        modal.keys_error = Some(format!("{e}"));
                    }
                }
            }
        });
        if let Some(err) = &modal.keys_error {
            ui.horizontal(|ui| {
                ui.add_space(14.0);
                ui.label(RichText::new(err).size(12.0).color(t.danger()));
            });
            ui.add_space(6.0);
        }
        if let Some(key) = modal.private_key.clone() {
            secret_block(ui, t, &key);
        }
    });
    footnote(
        ui,
        t,
        "Export your phrase before logging out of a device or reinstalling.",
    );

    ui.add_space(SECTION_GAP);
    section_label(ui, t, "This device");
    group(ui, t, |ui| {
        form_row(ui, t, "Log out", |ui| {
            if row_btn_danger(t, "Log out").show(ui).clicked() {
                modal.confirm = Some(ConfirmKind::Logout);
            }
        });
    });
    footnote(
        ui,
        t,
        "Removes local data from this computer. Your account and server files stay intact.",
    );

    SettingsOutcome::Open
}

fn show_plan(
    ui: &mut Ui, t: &Tokens, modal: &mut SettingsModal, core: &Lb,
) -> SettingsOutcome {
    // Poll async Stripe result (no overlapping &mut on modal).
    let mut outcome = SettingsOutcome::Open;
    let poll = modal
        .upgrade
        .as_mut()
        .and_then(|f| f.result_rx.as_ref())
        .map(|rx| rx.try_recv());
    match poll {
        Some(Ok(Ok(plan))) => {
            if let Some(flow) = modal.upgrade.as_mut() {
                flow.done = Some(Ok(()));
                flow.result_rx = None;
            }
            modal.data.plan = Some(plan.clone());
            if let Ok(usage) = core.get_usage() {
                modal.data.usage = Some(usage);
            }
            outcome = SettingsOutcome::PlanRefreshed(plan);
            modal.set_flash("You’re Premium — 30 GB is yours.");
        }
        Some(Ok(Err(e))) => {
            if let Some(flow) = modal.upgrade.as_mut() {
                flow.done = Some(Err(e));
                flow.result_rx = None;
            }
        }
        Some(Err(mpsc::TryRecvError::Empty)) => {
            ui.ctx().request_repaint_after(Duration::from_millis(100));
        }
        Some(Err(mpsc::TryRecvError::Disconnected)) => {
            if let Some(flow) = modal.upgrade.as_mut() {
                flow.done = Some(Err("Upgrade channel closed".into()));
                flow.result_rx = None;
            }
        }
        None => {}
    }

    if modal.upgrade.is_some() {
        let o = show_upgrade_flow(ui, t, modal, core);
        if !matches!(o, SettingsOutcome::Open) {
            return o;
        }
        return outcome;
    }

    section_label(ui, t, "Subscription");
    group(ui, t, |ui| {
        let tier = modal
            .data
            .plan
            .as_ref()
            .map(|p| p.tier.label())
            .unwrap_or("…")
            .to_string();
        let is_free = modal
            .data
            .plan
            .as_ref()
            .map(|p| matches!(p.tier, crate::widgets::sync_footer::AccountTier::Free))
            .unwrap_or(true);
        let premium = !is_free;

        form_row(ui, t, "Current plan", |ui| {
            tier_pill(ui, t, &tier, premium);
        });

        if let Some(plan) = &modal.data.plan {
            if let Some(detail) = plan.detail.clone() {
                kv_row(ui, t, "Status", &detail);
            }
            if let Some(source) = plan.source {
                kv_row(ui, t, "Billed via", source);
            }
        }

        if is_free {
            form_row(ui, t, "Get more storage", |ui| {
                if trail_action(ui, t, "Upgrade").clicked() {
                    modal.upgrade = Some(UpgradeFlow {
                        stage: UpgradeStage::Paywall,
                        number: String::new(),
                        exp_month: String::new(),
                        exp_year: String::new(),
                        cvc: String::new(),
                        error: None,
                        last4: None,
                        done: None,
                        result_rx: None,
                        nav: UpgradeNav::None,
                        interact_armed: false,
                    });
                }
            });
        } else {
            form_row(ui, t, "Subscription", |ui| {
                if trail_danger(ui, t, "Cancel").clicked() {
                    modal.confirm = Some(ConfirmKind::CancelSubscription);
                }
            });
        }
    });
    if modal
        .data
        .plan
        .as_ref()
        .map(|p| matches!(p.tier, crate::widgets::sync_footer::AccountTier::Free))
        .unwrap_or(true)
    {
        footnote(ui, t, "We charge for storage. Premium is 30 GB for $2.99/month.");
    }

    ui.add_space(SECTION_GAP);
    section_label(ui, t, "Storage");
    group(ui, t, |ui| {
        if let Some(usage) = &modal.data.usage {
            let used = usage.server_usage.readable.clone();
            let cap = usage.data_cap.readable.clone();
            form_row(ui, t, "Server utilization", |ui| {
                value_text(ui, t, &format!("{used} of {cap}"));
            });

            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                ui.add_space(ROW_PAD_L);
                ui.vertical(|ui| {
                    ui.set_width((ui.available_width() - ROW_PAD_R).max(40.0));
                    let frac = if usage.data_cap.exact == 0 {
                        0.0
                    } else {
                        (usage.server_usage.exact as f32 / usage.data_cap.exact as f32)
                            .clamp(0.0, 1.0)
                    };
                    let bar_h = 8.0;
                    let (br, _) =
                        ui.allocate_exact_size(vec2(ui.available_width(), bar_h), Sense::hover());
                    ui.painter()
                        .rect_filled(br, bar_h / 2.0, t.canvas().lerp_to_gamma(t.line(), 0.35));
                    let fill_w = br.width() * frac;
                    if fill_w > 0.5 {
                        let col = if frac >= 0.9 {
                            t.danger()
                        } else if frac >= 0.7 {
                            ui.ctx().get_lb_theme().fg().yellow
                        } else {
                            t.accent()
                        };
                        ui.painter().rect_filled(
                            Rect::from_min_size(br.min, vec2(fill_w, bar_h)),
                            bar_h / 2.0,
                            col,
                        );
                    }
                    ui.add_space(6.0);
                    let pct = (frac * 100.0).round() as i32;
                    ui.label(
                        RichText::new(format!("{pct}% used"))
                            .size(12.0)
                            .color(t.text_muted()),
                    );
                    ui.add_space(8.0);
                });
            });
        } else {
            form_row(ui, t, "Server utilization", |ui| {
                value_text(ui, t, "Unavailable");
            });
        }
    });
    outcome
}

fn show_upgrade_flow(
    ui: &mut Ui, t: &Tokens, modal: &mut SettingsModal, core: &Lb,
) -> SettingsOutcome {
    let Some(flow) = modal.upgrade.as_mut() else {
        return SettingsOutcome::Open;
    };

    // First frame of a stage: paint only, ignore clicks (prevents the
    // previous control’s click from activating the new primary CTA).
    let armed = flow.interact_armed;
    if !armed {
        flow.interact_armed = true;
    }

    match flow.stage {
        UpgradeStage::Paywall => show_upgrade_paywall(ui, t, flow, armed),
        UpgradeStage::Checkout => show_upgrade_checkout(ui, t, flow, core, armed),
        UpgradeStage::Paying => show_upgrade_paying(ui, t, flow, armed),
    }

    // Navigation resolved after painting (avoids overlapping borrows).
    let nav = modal.upgrade.as_ref().map(|f| f.nav).unwrap_or_default();
    if let Some(flow) = modal.upgrade.as_mut() {
        flow.nav = UpgradeNav::None;
    }
    match nav {
        UpgradeNav::Dismiss => {
            modal.upgrade = None;
        }
        UpgradeNav::ToCheckout => {
            if let Some(flow) = modal.upgrade.as_mut() {
                flow.stage = UpgradeStage::Checkout;
                flow.error = None;
                flow.interact_armed = false;
            }
        }
        UpgradeNav::ToPaywall => {
            if let Some(flow) = modal.upgrade.as_mut() {
                flow.stage = UpgradeStage::Paywall;
                flow.error = None;
                flow.interact_armed = false;
            }
        }
        UpgradeNav::None => {}
    }
    SettingsOutcome::Open
}

/// Decide screen — short offer, then checkout. No feature theater.
fn show_upgrade_paywall(ui: &mut Ui, t: &Tokens, flow: &mut UpgradeFlow, armed: bool) {
    section_label(ui, t, "Premium");
    group(ui, t, |ui| {
        ui.add_space(14.0);
        pad_col(ui, |ui| {
            ui.label(
                RichText::new("30 GB of encrypted storage")
                    .size(16.0)
                    .strong()
                    .color(t.fg()),
            );
            ui.add_space(4.0);
            ui.label(
                RichText::new("$2.99/month · Cancel anytime")
                    .size(13.5)
                    .color(t.text_muted()),
            );

            ui.add_space(16.0);
            // Continue (primary) + Not now (quiet) on one row.
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                let cont = Button::primary(t, "Continue").height(32.0).show(ui);
                if armed && cont.clicked() {
                    flow.nav = UpgradeNav::ToCheckout;
                }
                let skip = Button::quiet(t, "Not now").height(32.0).show(ui);
                if armed && skip.clicked() {
                    flow.nav = UpgradeNav::Dismiss;
                }
            });
        });
        ui.add_space(14.0);
    });
}

/// Card entry — fields + pay. Offer was already on the paywall.
fn show_upgrade_checkout(
    ui: &mut Ui, t: &Tokens, flow: &mut UpgradeFlow, core: &Lb, armed: bool,
) {
    section_label(ui, t, "Payment");
    group(ui, t, |ui| {
        ui.add_space(12.0);
        pad_col(ui, |ui| {
            ui.add(
                TextEdit::singleline(&mut flow.number)
                    .hint_text("Card number")
                    .desired_width(f32::INFINITY),
            );
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                ui.add(
                    TextEdit::singleline(&mut flow.exp_month)
                        .desired_width(56.0)
                        .hint_text("MM"),
                );
                ui.add(
                    TextEdit::singleline(&mut flow.exp_year)
                        .desired_width(56.0)
                        .hint_text("YY"),
                );
                ui.add(
                    TextEdit::singleline(&mut flow.cvc)
                        .desired_width(64.0)
                        .hint_text("CVC"),
                );
            });

            if let Some(err) = &flow.error {
                ui.add_space(8.0);
                ui.label(RichText::new(err).size(12.5).color(t.danger()));
            }

            ui.add_space(14.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                let pay = Button::primary(t, "Pay $2.99/mo").height(32.0).show(ui);
                if armed && pay.clicked() && validate_card(flow) {
                    start_stripe_pay(flow, core, ui.ctx());
                }
                let back = Button::quiet(t, "Back").height(32.0).show(ui);
                if armed && back.clicked() {
                    flow.nav = UpgradeNav::ToPaywall;
                }
            });
            ui.add_space(8.0);
            ui.label(
                RichText::new("Secured by Stripe")
                    .size(12.0)
                    .color(t.text_muted()),
            );
        });
        ui.add_space(12.0);
    });
}

fn show_upgrade_paying(ui: &mut Ui, t: &Tokens, flow: &mut UpgradeFlow, armed: bool) {
    section_label(ui, t, "Premium");
    group(ui, t, |ui| {
        ui.add_space(16.0);
        pad_col(ui, |ui| {
            match &flow.done {
                None => {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 10.0;
                        ui.spinner();
                        ui.label(
                            RichText::new("Processing…")
                                .size(14.0)
                                .color(t.fg()),
                        );
                    });
                }
                Some(Ok(())) => {
                    ui.label(
                        RichText::new("You’re Premium.")
                            .size(16.0)
                            .strong()
                            .color(t.fg()),
                    );
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new("30 GB of encrypted storage.")
                            .size(13.0)
                            .color(t.text_muted()),
                    );
                    ui.add_space(14.0);
                    let cta_w = ui.available_width();
                    if armed && full_cta(ui, t, "Done", cta_w).clicked() {
                        flow.nav = UpgradeNav::Dismiss;
                    }
                }
                Some(Err(e)) => {
                    ui.label(
                        RichText::new("Payment failed")
                            .size(15.0)
                            .strong()
                            .color(t.fg()),
                    );
                    ui.add_space(6.0);
                    ui.label(RichText::new(e).size(12.5).color(t.danger()));
                    ui.add_space(14.0);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 8.0;
                        if armed
                            && Button::secondary(t, "Try again")
                                .height(BTN_H)
                                .show(ui)
                                .clicked()
                        {
                            flow.nav = UpgradeNav::ToCheckout;
                            flow.done = None;
                            flow.error = None;
                            flow.interact_armed = false;
                        }
                        if armed
                            && Button::quiet(t, "Cancel").height(BTN_H).show(ui).clicked()
                        {
                            flow.nav = UpgradeNav::Dismiss;
                        }
                    });
                }
            }
        });
        ui.add_space(16.0);
    });
}

fn pad_col(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        ui.add_space(ROW_PAD_L);
        ui.vertical(|ui| {
            ui.set_width((ui.available_width() - ROW_PAD_R).max(80.0));
            add(ui);
        });
    });
}

/// Full-width primary CTA for paywall / checkout.
fn full_cta(ui: &mut Ui, t: &Tokens, label: &str, width: f32) -> egui::Response {
    let h = 36.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(width.max(120.0), h), Sense::click());
    let hover = ui.ctx().animate_bool(resp.id, resp.hovered());
    let fill = if resp.is_pointer_button_down_on() {
        t.fg().lerp_to_gamma(t.canvas(), 0.16)
    } else {
        t.fg().lerp_to_gamma(t.canvas(), 0.08 * hover)
    };
    ui.painter().rect_filled(rect, 8.0, fill);
    let g = ui.painter().layout_no_wrap(
        label.into(),
        FontId::proportional(14.0),
        t.canvas(),
    );
    ui.painter()
        .galley(rect.center() - g.size() / 2.0, g, t.canvas());
    resp
}

/// Returns true if card fields are valid and normalized onto `flow`.
fn validate_card(flow: &mut UpgradeFlow) -> bool {
    let number: String = flow.number.chars().filter(|c| c.is_ascii_digit()).collect();
    if number.len() < 12 {
        flow.error = Some("Enter a valid card number".into());
        return false;
    }
    let exp_month: i32 = match flow.exp_month.trim().parse() {
        Ok(m) if (1..=12).contains(&m) => m,
        _ => {
            flow.error = Some("Invalid expiry month".into());
            return false;
        }
    };
    let exp_year: i32 = match flow.exp_year.trim().parse() {
        Ok(y) => {
            if y < 100 {
                2000 + y
            } else {
                y
            }
        }
        _ => {
            flow.error = Some("Invalid expiry year".into());
            return false;
        }
    };
    let cvc = flow.cvc.trim().to_string();
    if cvc.len() < 3 {
        flow.error = Some("Enter a valid CVC".into());
        return false;
    }
    flow.last4 = Some(number[number.len() - 4..].to_string());
    flow.error = None;
    flow.number = number;
    flow.exp_month = exp_month.to_string();
    flow.exp_year = exp_year.to_string();
    flow.cvc = cvc;
    true
}

fn start_stripe_pay(flow: &mut UpgradeFlow, core: &Lb, ctx: &egui::Context) {
    let number = flow.number.clone();
    let exp_month: i32 = flow.exp_month.parse().unwrap_or(0);
    let exp_year: i32 = flow.exp_year.parse().unwrap_or(0);
    let cvc = flow.cvc.clone();
    let method = PaymentMethod::NewCard {
        number,
        exp_month,
        exp_year,
        cvc,
    };
    let (tx, rx) = mpsc::channel();
    flow.result_rx = Some(rx);
    flow.stage = UpgradeStage::Paying;
    flow.done = None;
    let core = core.clone();
    let ctx = ctx.clone();
    std::thread::spawn(move || {
        let result = match core.upgrade_account_stripe(StripeAccountTier::Premium(method)) {
            Ok(()) => match core.get_subscription_info() {
                Ok(info) => {
                    let cap = core.get_usage().ok().map(|u| u.data_cap.exact);
                    Ok(AccountInfo::from_subscription_and_cap(info, cap))
                }
                Err(e) => Err(format!("Upgraded, but couldn’t refresh plan: {e}")),
            },
            Err(e) => Err(format!("{e}")),
        };
        let _ = tx.send(result);
        ctx.request_repaint();
    });
}

/// Quiet outline capsule (Cancel / Back / Done) at trail height.
fn trail_capsule_btn(
    ui: &mut Ui, t: &Tokens, label: &str, danger: bool,
) -> egui::Response {
    let pad_x = 10.0;
    let font = FontId::proportional(12.0);
    let base_ink = if danger { t.danger() } else { t.fg() };
    let g = ui
        .painter()
        .layout_no_wrap(label.into(), font.clone(), base_ink);
    let w = (g.size().x + pad_x * 2.0).max(TRAIL_H * 1.6);
    let (rect, resp) = ui.allocate_exact_size(vec2(w, TRAIL_H), Sense::click());
    let hover = ui.ctx().animate_bool(resp.id, resp.hovered());
    let stroke = if hover > 0.5 || resp.is_pointer_button_down_on() {
        base_ink
    } else {
        t.line()
    };
    let fill = if resp.is_pointer_button_down_on() {
        t.surface_raised()
    } else if hover > 0.0 {
        t.canvas().lerp_to_gamma(t.surface_raised(), hover)
    } else {
        Color32::TRANSPARENT
    };
    if fill.a() > 0 {
        ui.painter()
            .rect_filled(rect, rect.height() / 2.0, fill);
    }
    ui.painter().rect_stroke(
        rect,
        rect.height() / 2.0,
        Stroke::new(1.0, stroke),
        egui::StrokeKind::Inside,
    );
    let g = ui.painter().layout_no_wrap(label.into(), font, base_ink);
    ui.painter()
        .galley(rect.center() - g.size() / 2.0, g, base_ink);
    resp
}

fn show_appearance(ui: &mut Ui, t: &Tokens, modal: &mut SettingsModal) -> SettingsOutcome {
    let mut outcome = SettingsOutcome::Open;
    section_label(ui, t, "Theme mode");
    group(ui, t, |ui| {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            ui.add_space(ROW_PAD_L);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                for mode in ThemeMode::ALL {
                    let active = modal.data.theme_mode == mode;
                    let resp = mode_chip(ui, t, mode.label(), active);
                    if resp.clicked() && !active {
                        modal.data.theme_mode = mode;
                        outcome = SettingsOutcome::SaveAppearance {
                            theme_mode: mode,
                            theme_name: modal.data.theme_name.clone(),
                        };
                    }
                }
            });
        });
        ui.add_space(8.0);
    });
    footnote(
        ui,
        t,
        "System follows your OS appearance. Dark and Light stay fixed.",
    );

    ui.add_space(SECTION_GAP);
    section_label(ui, t, "Color theme");
    settings::ensure_themes_dir();
    let themes = settings::list_themes();
    group(ui, t, |ui| {
        for name in &themes {
            let active = modal.data.theme_name == *name;
            form_row(ui, t, name, |ui| {
                if active {
                    ui.label(
                        RichText::new(icons::CHECK_CIRCLE)
                            .font(icons::font(16.0))
                            .color(t.accent()),
                    );
                } else if trail_capsule_btn(ui, t, "Use", false).clicked() {
                    modal.data.theme_name = name.clone();
                    outcome = SettingsOutcome::SaveAppearance {
                        theme_mode: modal.data.theme_mode,
                        theme_name: name.clone(),
                    };
                }
            });
        }
    });
    if let Some(dir) = settings::themes_dir() {
        footnote(
            ui,
            t,
            &format!("Themes folder: {}", dir.display()),
        );
    }
    outcome
}

fn mode_chip(ui: &mut Ui, t: &Tokens, label: &str, active: bool) -> egui::Response {
    let pad_x = 12.0;
    let font = FontId::proportional(12.5);
    let g = ui
        .painter()
        .layout_no_wrap(label.into(), font.clone(), t.fg());
    let w = g.size().x + pad_x * 2.0;
    let h = 28.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(w, h), Sense::click());
    let hover = ui.ctx().animate_bool(resp.id, resp.hovered());
    let fill = if active {
        t.fg()
    } else if hover > 0.0 {
        t.canvas().lerp_to_gamma(t.surface_raised(), hover)
    } else {
        t.surface_raised()
    };
    let ink = if active { t.canvas() } else { t.fg() };
    ui.painter().rect_filled(rect, 8.0, fill);
    if !active {
        ui.painter().rect_stroke(
            rect,
            8.0,
            Stroke::new(1.0, t.line()),
            egui::StrokeKind::Inside,
        );
    }
    let g = ui.painter().layout_no_wrap(label.into(), font, ink);
    ui.painter()
        .galley(rect.center() - g.size() / 2.0, g, ink);
    resp
}

fn show_editor(ui: &mut Ui, t: &Tokens, modal: &mut SettingsModal) -> SettingsOutcome {
    let mut outcome = SettingsOutcome::Open;
    section_label(ui, t, "Link previews");
    group(ui, t, |ui| {
        let mut on = modal.data.contact_linked_sites;
        form_row(ui, t, "Fetch link previews", |ui| {
            if toggle(ui, t, &mut on) {
                modal.data.contact_linked_sites = on;
                outcome = SettingsOutcome::SavePrefs {
                    contact_linked_sites: on,
                };
            }
        });
    });
    footnote(
        ui,
        t,
        "Titles and preview cards contact the linked site, which reveals your IP and that you opened the note. Off by default.",
    );
    outcome
}

fn show_privacy(ui: &mut Ui, t: &Tokens, modal: &mut SettingsModal) -> SettingsOutcome {
    section_label(ui, t, "Legal");
    group(ui, t, |ui| {
        link_row(ui, t, "Privacy Policy", "https://lockbook.net/privacy-policy");
        link_row(ui, t, "Terms of Service", "https://lockbook.net/tos");
    });

    ui.add_space(SECTION_GAP);
    section_label(ui, t, "Danger zone");
    group(ui, t, |ui| {
        form_row(ui, t, "Delete account", |ui| {
            if row_btn_danger(t, "Delete").show(ui).clicked() {
                modal.confirm = Some(ConfirmKind::DeleteAccount);
            }
        });
    });
    footnote(
        ui,
        t,
        "Permanently deletes your account and all data on the server. This cannot be undone.",
    );
    SettingsOutcome::Open
}

/// Trailing control that shows a path (ellipsized) and copies on click —
/// same slot/style family as other trail buttons.
fn path_copy_btn(ui: &mut Ui, t: &Tokens, path: &str, max_w: f32) -> egui::Response {
    let font = FontId::monospace(12.0);
    let pad_x = 10.0;
    let full = ui
        .painter()
        .layout_no_wrap(path.to_owned(), font.clone(), t.text_muted());
    let inner_max = (max_w - pad_x * 2.0).max(24.0);
    let (label, text_w) = if full.size().x <= inner_max {
        (path.to_owned(), full.size().x)
    } else {
        // Left-truncate so the unique tail of the path stays visible.
        let chars: Vec<char> = path.chars().collect();
        let mut start = 0usize;
        let mut shown = path.to_owned();
        let mut w = full.size().x;
        while w > inner_max && start + 1 < chars.len() {
            start += 1;
            shown = format!("…{}", chars[start..].iter().collect::<String>());
            w = ui
                .painter()
                .layout_no_wrap(shown.clone(), font.clone(), t.text_muted())
                .size()
                .x;
        }
        (shown, w.min(inner_max))
    };
    let w = (text_w + pad_x * 2.0).min(max_w).max(TRAIL_H * 2.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(w, TRAIL_H), Sense::click());
    let hover = ui.ctx().animate_bool(resp.id, resp.hovered());
    let stroke = if hover > 0.5 || resp.is_pointer_button_down_on() {
        t.fg()
    } else {
        t.line()
    };
    let fill = if resp.is_pointer_button_down_on() {
        t.surface_raised()
    } else if hover > 0.0 {
        t.canvas().lerp_to_gamma(t.surface_raised(), hover)
    } else {
        Color32::TRANSPARENT
    };
    if fill.a() > 0 {
        ui.painter()
            .rect_filled(rect, rect.height() / 2.0, fill);
    }
    ui.painter().rect_stroke(
        rect,
        rect.height() / 2.0,
        Stroke::new(1.0, stroke),
        egui::StrokeKind::Inside,
    );
    let ink = t.text_muted().lerp_to_gamma(t.fg(), hover);
    let g = ui.painter().layout_no_wrap(label, font, ink);
    ui.painter()
        .galley(rect.center() - g.size() / 2.0, g, ink);
    resp.on_hover_text(path)
}

fn show_debug(
    ui: &mut Ui, t: &Tokens, modal: &mut SettingsModal, core: &Lb,
) -> SettingsOutcome {
    section_label(ui, t, "Connection");
    group(ui, t, |ui| {
        kv_row(ui, t, "Server", &modal.data.api_url);
        if let Some(s) = modal.data.last_synced.clone() {
            kv_row(ui, t, "Last synced", &s);
        }
        // Path itself is the trailing control — same slot as Copy, copies on click.
        let path = modal.data.writeable_path.clone();
        form_row(ui, t, "Data path", |ui| {
            let max_w = (ui.available_width() - 4.0).clamp(80.0, 280.0);
            if path_copy_btn(ui, t, &path, max_w).clicked() {
                ui.ctx().copy_text(path.clone());
                modal.set_flash("Path copied");
            }
        });
    });

    ui.add_space(SECTION_GAP);
    section_label(ui, t, "Diagnostics");
    group(ui, t, |ui| {
        form_row(ui, t, "Debug info", |ui| {
            if row_btn(t, "Copy").show(ui).clicked() {
                match core.debug_info("egui-desktop".into()) {
                    Ok(info) => {
                        let text = serde_json::to_string_pretty(&info)
                            .unwrap_or_else(|_| format!("{info:?}"));
                        ui.ctx().copy_text(text);
                        modal.set_flash("Debug info copied");
                    }
                    Err(e) => {
                        modal.error = Some(format!("Debug info failed: {e}"));
                        modal.flash = None;
                        modal.flash_at = None;
                    }
                }
            }
        });
        kv_row(
            ui,
            t,
            "App version",
            &format!("lockbook-egui {}", env!("CARGO_PKG_VERSION")),
        );
    });
    SettingsOutcome::Open
}

fn link_row(ui: &mut Ui, t: &Tokens, label: &str, url: &str) {
    form_row(ui, t, label, |ui| {
        // Quiet trailing affordance with external-link mark
        let (rect, resp) = ui.allocate_exact_size(vec2(28.0, BTN_H), Sense::click());
        let hover = ui.ctx().animate_bool(resp.id, resp.hovered());
        let ink = t.text_muted().lerp_to_gamma(t.fg(), hover);
        let g = ui.painter().layout_no_wrap(
            icons::ARROW_SQUARE_OUT.into(),
            icons::font(15.0),
            ink,
        );
        ui.painter()
            .galley(rect.center() - g.size() / 2.0, g, ink);
        if resp.clicked() {
            open_url(url);
        }
        if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
    });
}

fn show_confirm_overlay(
    ui: &mut Ui, t: &Tokens, modal: &mut SettingsModal, kind: &ConfirmKind, panel: Rect,
) -> Option<SettingsOutcome> {
    let (title, body, primary) = match kind {
        ConfirmKind::Logout => (
            "Log out of this device?",
            "Local data on this computer will be removed. Make sure your recovery phrase is backed up.",
            "Log out",
        ),
        ConfirmKind::DeleteAccount => (
            "Delete your account?",
            "This permanently deletes your account and all data on the server. This cannot be undone.",
            "Delete account",
        ),
        ConfirmKind::CancelSubscription => (
            "Cancel subscription?",
            "You’ll return to the free storage limit. You can upgrade again anytime.",
            "Cancel subscription",
        ),
    };

    let mut result = None;
    // Soft scrim clipped to the shell rounding.
    ui.painter().rect_filled(
        panel,
        CornerRadius::same(SHELL_R),
        Color32::from_black_alpha(90),
    );

    let scrim_id = ui.id().with("settings_confirm_scrim");
    let scrim_resp = ui.interact(panel, scrim_id, Sense::click());
    if scrim_resp.clicked() {
        modal.confirm = None;
    }

    let card_w = 360.0_f32;
    let center = panel.center();
    Area::new(Id::new("lb_settings_confirm"))
        .order(Order::Tooltip)
        .fixed_pos(center - vec2(card_w / 2.0, 70.0))
        .fade_in(false)
        .show(ui.ctx(), |ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            Frame::new()
                .fill(t.canvas())
                .stroke(Stroke::new(1.0, t.line()))
                .corner_radius(CornerRadius::same(12))
                .inner_margin(Margin::symmetric(18, 16))
                .shadow(egui::Shadow {
                    offset: [0, 6],
                    blur: 24,
                    spread: 0,
                    color: Color32::from_black_alpha(50),
                })
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                    ui.set_width(card_w);
                    ui.label(
                        RichText::new(title)
                            .size(16.0)
                            .strong()
                            .color(t.fg()),
                    );
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(body)
                            .size(13.0)
                            .color(t.text_muted()),
                    );
                    ui.add_space(16.0);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                            if Button::secondary(t, primary)
                                .danger()
                                .height(32.0)
                                .show(ui)
                                .clicked()
                            {
                                result = Some(match kind {
                                    ConfirmKind::Logout => SettingsOutcome::Logout {
                                        writeable_path: modal.data.writeable_path.clone(),
                                    },
                                    ConfirmKind::DeleteAccount => SettingsOutcome::DeleteAccount,
                                    ConfirmKind::CancelSubscription => {
                                        SettingsOutcome::CancelSubscription
                                    }
                                });
                                modal.confirm = None;
                            }
                            ui.add_space(8.0);
                            if Button::secondary(t, "Cancel")
                                .height(32.0)
                                .show(ui)
                                .clicked()
                            {
                                modal.confirm = None;
                            }
                        });
                    });
                });
        });
    result
}

fn open_url(url: &str) {
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(url).spawn();
    #[cfg(target_os = "linux")]
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("cmd")
        .args(["/C", "start", url])
        .spawn();
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    let _ = url;
}
