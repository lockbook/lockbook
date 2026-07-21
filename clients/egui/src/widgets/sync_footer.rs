//! Sidebar sync footer — Apple `SyncStatusFooter` for egui.
//!
//! Vertical rhythm (one pad `V_PAD` everywhere it appears):
//!   pad → [usage bar → pad] → status row → pad
//!
//! Status row: colored dot · message · icon-only sync control (arrows spin
//! while syncing / after tap). Indicator priority mirrors Apple
//! `SyncIndicator`; during sync the *dot* stays green so arrows show activity.
//!
//! Hovering **anywhere on the footer** shows one floating card (status +
//! storage + account when known) — not a tiny bar-only hit target.

use chrono::{Datelike, Local, TimeZone};
use egui::text::{LayoutJob, TextFormat, TextWrapping};
use egui::{
    Align, Area, FontId, Id, Layout, Margin, Order, RichText, Sense, Ui, pos2, vec2,
};
use lb::model::api::{
    AppStoreAccountState, FREE_TIER_USAGE_SIZE, GooglePlayAccountState, PaymentPlatform,
    SubscriptionInfo,
};
use lb::subscribers::status::Status;
use workspace_rs::theme::palette_v2::ThemeExt;

use crate::theme::icons;
use crate::theme::tokens::Tokens;

/// Free vs Premium for the footer hover card.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccountTier {
    Free,
    Premium,
}

impl AccountTier {
    pub fn label(self) -> &'static str {
        match self {
            Self::Free => "Free",
            Self::Premium => "Premium",
        }
    }
}

/// Account standing derived from `get_subscription_info` for the footer card.
///
/// Display order (most important first):
/// 1. tier — Free / Premium
/// 2. standing / date — renews, access until, grace, expired…
/// 3. source — Stripe / Google Play / App Store (muted; never last-4)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountInfo {
    pub tier: AccountTier,
    /// e.g. "Renews April 12, 2026", "Access until …", "Expired".
    pub detail: Option<String>,
    /// Billing source, quiet secondary — not the hero line.
    pub source: Option<&'static str>,
}

impl AccountInfo {
    /// Map subscription info + server data cap → footer / settings display.
    ///
    /// Stripe’s public payload has **no** account state. After cancel the
    /// server still returns a Stripe row but sets the data cap to free —
    /// same signal `cancel_subscription` uses. Always pass `get_usage().data_cap`.
    pub fn from_subscription_and_cap(
        info: Option<SubscriptionInfo>, data_cap_exact: Option<u64>,
    ) -> Self {
        // Server truth for "is premium / can cancel".
        let cap_is_free = data_cap_exact.is_some_and(|c| c <= FREE_TIER_USAGE_SIZE);

        let Some(info) = info else {
            return Self {
                tier: AccountTier::Free,
                detail: None,
                source: None,
            };
        };

        let date = format_period_end(info.period_end);
        let renews = date
            .as_ref()
            .map(|d| format!("Renews at {PREMIUM_PRICE} on {d}"));

        match info.payment_platform {
            // Never show card last-4. Canceled Stripe still returns a row.
            PaymentPlatform::Stripe { .. } => {
                if cap_is_free {
                    Self {
                        tier: AccountTier::Free,
                        detail: Some("Canceled".into()),
                        source: Some("Stripe"),
                    }
                } else {
                    Self {
                        tier: AccountTier::Premium,
                        detail: renews,
                        source: Some("Stripe"),
                    }
                }
            }

            PaymentPlatform::GooglePlay { account_state } => match account_state {
                GooglePlayAccountState::Ok if !cap_is_free => Self {
                    tier: AccountTier::Premium,
                    detail: renews,
                    source: Some("Google Play"),
                },
                GooglePlayAccountState::Ok => Self {
                    tier: AccountTier::Free,
                    detail: Some("Canceled".into()),
                    source: Some("Google Play"),
                },
                // Still premium through the paid period (unless cap already free).
                GooglePlayAccountState::Canceled if !cap_is_free => Self {
                    tier: AccountTier::Premium,
                    detail: date.map(|d| format!("Access until {d}")),
                    source: Some("Google Play"),
                },
                GooglePlayAccountState::Canceled => Self {
                    tier: AccountTier::Free,
                    detail: date.map(|d| format!("Ended {d}")),
                    source: Some("Google Play"),
                },
                GooglePlayAccountState::GracePeriod => Self {
                    tier: AccountTier::Premium,
                    detail: date
                        .map(|d| format!("Grace period · until {d}"))
                        .or_else(|| Some("Grace period".into())),
                    source: Some("Google Play"),
                },
                GooglePlayAccountState::OnHold => Self {
                    tier: AccountTier::Free,
                    detail: Some("On hold".into()),
                    source: Some("Google Play"),
                },
            },

            PaymentPlatform::AppStore { account_state } => match account_state {
                AppStoreAccountState::Ok if !cap_is_free => Self {
                    tier: AccountTier::Premium,
                    detail: renews,
                    source: Some("App Store"),
                },
                AppStoreAccountState::Ok => Self {
                    tier: AccountTier::Free,
                    detail: None,
                    source: Some("App Store"),
                },
                AppStoreAccountState::GracePeriod => Self {
                    tier: AccountTier::Premium,
                    detail: date
                        .map(|d| format!("Grace period · until {d}"))
                        .or_else(|| Some("Grace period".into())),
                    source: Some("App Store"),
                },
                AppStoreAccountState::FailedToRenew => Self {
                    tier: AccountTier::Free,
                    detail: Some("Failed to renew".into()),
                    source: Some("App Store"),
                },
                AppStoreAccountState::Expired => Self {
                    tier: AccountTier::Free,
                    detail: Some("Expired".into()),
                    source: Some("App Store"),
                },
            },
        }
    }
}

/// Lockbook Premium list price (not on the subscription payload today).
const PREMIUM_PRICE: &str = "$2.99";

/// `period_end` is epoch milliseconds (`UnixTimeMillis`).
///
/// Stripe used to store **seconds** by mistake; values below ~1e11 (~1973 in ms,
/// or year ~5138 in seconds) are treated as seconds and scaled so existing
/// accounts still show a real renew date until the server rewrites them.
fn format_period_end(period_end: u64) -> Option<String> {
    if period_end == 0 {
        return None;
    }
    let ms = if period_end < 100_000_000_000 {
        period_end.saturating_mul(1000)
    } else {
        period_end
    };
    Local
        .timestamp_millis_opt(ms as i64)
        .single()
        // e.g. "August 19" — year is noise for a monthly renew line.
        .map(|dt| format!("{} {}", dt.format("%B"), dt.day()))
}

/// Mirrors Apple `SyncIndicator`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyncIndicator {
    Synced,
    Syncing,
    Offline,
    OutOfSpace,
    UpdateRequired,
    SyncError,
}

impl SyncIndicator {
    pub fn from_status(status: &Status) -> Self {
        if status.offline {
            Self::Offline
        } else if status.out_of_space {
            Self::OutOfSpace
        } else if status.update_required {
            Self::UpdateRequired
        } else if status.unexpected_sync_problem.is_some() {
            Self::SyncError
        } else if status.syncing {
            Self::Syncing
        } else {
            Self::Synced
        }
    }

    /// Short fallback label when `Status::msg()` is empty.
    pub fn short_label(self) -> &'static str {
        match self {
            Self::Synced => "Synced",
            Self::Syncing => "Syncing",
            Self::Offline => "Offline",
            Self::OutOfSpace => "No space",
            Self::UpdateRequired => "Update",
            Self::SyncError => "Error",
        }
    }

    /// Dot color. Attention states only — activity is the spinning arrows.
    pub fn color(self, ui: &Ui) -> egui::Color32 {
        let theme = ui.ctx().get_lb_theme();
        match self {
            Self::Synced | Self::Syncing => theme.fg().green,
            Self::Offline => theme.neutral_fg_secondary(),
            Self::OutOfSpace => theme.fg().yellow,
            Self::UpdateRequired | Self::SyncError => theme.fg().red,
        }
    }
}

/// Local footer chrome (stable message + spin), updated from `Status` each frame.
#[derive(Default)]
pub struct SyncFooter {
    /// Last non-syncing message — held during sync so "Last synced: …" doesn't
    /// flash away while the arrows spin (Apple `stableMessage`).
    stable_message: String,
    /// Manual sync tap keeps arrows spinning until status reports done.
    spin_until_idle: bool,
}

impl SyncFooter {
    /// Draw the footer. Returns `true` if the user requested a sync.
    /// `account` is standing once subscription info has loaded (or `None`
    /// while unknown / in demo).
    pub fn show(
        &mut self, ui: &mut Ui, t: &Tokens, status: &Status, account: Option<&AccountInfo>,
    ) -> bool {
        // Hold the last quiet message across a sync pulse.
        if !status.syncing {
            if let Some(msg) = status.msg() {
                if !msg.is_empty() {
                    self.stable_message = msg;
                }
            }
            self.spin_until_idle = false;
        }

        let raw = SyncIndicator::from_status(status);
        // Dot color: don't flip while syncing; arrows show activity.
        let display = if raw == SyncIndicator::Syncing {
            SyncIndicator::Synced
        } else {
            raw
        };

        let message = if !self.stable_message.is_empty() {
            self.stable_message.clone()
        } else if let Some(msg) = status.msg() {
            msg
        } else {
            display.short_label().to_string()
        };

        let spinning = status.syncing || self.spin_until_idle;
        if spinning {
            ui.ctx().request_repaint();
        }

        // One vertical rhythm: pad above bar = pad bar↔text = pad below text.
        const V_PAD: f32 = 10.0;
        const PAD_X: f32 = 12.0;
        const BAR_H: f32 = 6.0;
        const ROW_H: f32 = 28.0;
        const ICON_BTN: f32 = 26.0;

        ui.spacing_mut().item_spacing.y = 0.0;

        // Usage metrics (bar only when ≥ 50%; hover card can always show them).
        let usage_info = status.space_used.as_ref().and_then(|usage| {
            let cap = usage.data_cap.exact;
            if cap == 0 {
                return None;
            }
            let frac = (usage.server_usage.exact as f64 / cap as f64).min(1.0) as f32;
            Some((
                usage.server_usage.readable.as_str(),
                usage.data_cap.readable.as_str(),
                frac,
            ))
        });

        // Track the full footer rect for one big hover hit target.
        let footer_top = ui.cursor().top();
        let footer_left = ui.max_rect().left();
        let footer_right = ui.max_rect().right();

        ui.add_space(V_PAD);

        if let Some((_, _, frac)) = usage_info {
            if frac >= 0.5 {
                const WARN: f32 = 0.7;
                let bar_color = if frac >= WARN {
                    ui.ctx().get_lb_theme().fg().yellow
                } else {
                    t.accent()
                };
                let (br, _) =
                    ui.allocate_exact_size(vec2(ui.available_width(), BAR_H), Sense::hover());
                let track = egui::Rect::from_min_size(
                    pos2(br.left() + PAD_X, br.center().y - BAR_H / 2.0),
                    vec2((br.width() - 2.0 * PAD_X).max(0.0), BAR_H),
                );
                ui.painter()
                    .rect_filled(track, BAR_H / 2.0, t.surface_raised());
                let fill = egui::Rect::from_min_size(
                    track.min,
                    vec2(track.width() * frac, track.height()),
                );
                ui.painter().rect_filled(fill, BAR_H / 2.0, bar_color);
                ui.add_space(V_PAD);
            }
        }

        // Status row: dot · message · spacer · icon-only sync control.
        let (rect, _) =
            ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::hover());
        let cy = rect.center().y;
        let mut x = rect.left() + PAD_X;

        let dot_r = 4.0;
        ui.painter()
            .circle_filled(pos2(x + dot_r, cy), dot_r, display.color(ui));
        x += 8.0 + 8.0;

        let ink = t.text_muted();
        let icon_left = rect.right() - PAD_X - ICON_BTN;
        let msg_max = (icon_left - 6.0 - x).max(0.0);
        if msg_max > 0.0 {
            let mut job = LayoutJob {
                wrap: TextWrapping {
                    max_width: msg_max,
                    max_rows: 1,
                    break_anywhere: true,
                    overflow_character: Some('…'),
                },
                ..Default::default()
            };
            job.append(
                &message,
                0.0,
                TextFormat {
                    font_id: FontId::proportional(13.0),
                    color: ink,
                    ..Default::default()
                },
            );
            let msg_g = ui.fonts(|f| f.layout_job(job));
            let msg_pos = pos2(x, cy - msg_g.size().y / 2.0);
            ui.painter().galley(msg_pos, msg_g, ink);
        }

        // Sync = arrows only (not the whole row) — click target on the icon.
        let icon_rect = egui::Rect::from_min_size(
            pos2(icon_left, cy - ICON_BTN / 2.0),
            vec2(ICON_BTN, ICON_BTN),
        );
        let icon_resp = ui.interact(icon_rect, ui.id().with("sync_btn"), Sense::click());
        let ih = ui.ctx().animate_bool(icon_resp.id, icon_resp.hovered());
        if ih > 0.0 {
            ui.painter().rect_filled(
                icon_rect,
                6.0,
                t.canvas().lerp_to_gamma(t.surface_raised(), ih),
            );
        }
        let ag = ui.painter().layout_no_wrap(
            icons::ARROWS_CLOCKWISE.into(),
            icons::font(16.0),
            ink,
        );
        let apos = icon_rect.center() - ag.size() / 2.0;
        if spinning {
            let angle =
                (ui.input(|i| i.time) as f32 * std::f32::consts::TAU) % std::f32::consts::TAU;
            let shape = egui::epaint::TextShape::new(apos, ag, ink)
                .with_override_text_color(ink)
                .with_angle_and_anchor(angle, egui::Align2::CENTER_CENTER);
            ui.painter().add(shape);
        } else {
            ui.painter().galley(apos, ag, ink);
        }

        ui.add_space(V_PAD);

        // Whole-footer hover: status + storage + plan in one card.
        let footer_bot = ui.cursor().top();
        let footer_rect = egui::Rect::from_min_max(
            pos2(footer_left, footer_top),
            pos2(footer_right, footer_bot),
        );
        let footer_hover =
            ui.interact(footer_rect, ui.id().with("sync_footer_hover"), Sense::hover());
        // Don't steal the sync button's hover paint — card still shows when
        // over the icon; click stays on icon_resp.
        if footer_hover.hovered() && !icon_resp.is_pointer_button_down_on() {
            let full_msg = status
                .msg()
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| message.clone());
            footer_hover_card(
                ui,
                t,
                footer_rect,
                display,
                &full_msg,
                spinning,
                usage_info,
                account,
            );
        }

        let clicked = icon_resp.clicked();
        if clicked {
            self.spin_until_idle = true;
        }
        clicked
    }
}

/// Footer hover card — anchored above the footer block (not the cursor).
/// Kept dense: one line per fact, no expand-to-fill layouts.
#[allow(clippy::too_many_arguments)]
fn footer_hover_card(
    ui: &Ui,
    t: &Tokens,
    footer_rect: egui::Rect,
    indicator: SyncIndicator,
    status_msg: &str,
    spinning: bool,
    usage: Option<(&str, &str, f32)>,
    account: Option<&AccountInfo>,
) {
    let pos = pos2(footer_rect.center().x, footer_rect.top() - 6.0);
    const CARD_W: f32 = 200.0;

    Area::new(Id::new("lb_sync_footer_hover_card"))
        .order(Order::Tooltip)
        .fixed_pos(pos)
        .pivot(egui::Align2::CENTER_BOTTOM)
        .constrain(true)
        .sense(Sense::hover())
        // Shrink-wrap to content — without this, Area max-rect is the screen
        // and right_to_left / vertical spacing blow up to full window height.
        .default_size(vec2(0.0, 0.0))
        .show(ui.ctx(), |ui| {
            t.floating()
                .frame_margin(Margin::symmetric(12, 8))
                .show(ui, |ui| {
                    ui.set_width(CARD_W);
                    ui.set_max_width(CARD_W);
                    ui.spacing_mut().item_spacing.y = 2.0;

                    // ── Sync ─────────────────────────────────────────────
                    ui.label(
                        RichText::new("Sync").size(11.0).color(t.text_muted()),
                    );
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        let r = 3.5;
                        let (dot_r, _) =
                            ui.allocate_exact_size(vec2(r * 2.0, r * 2.0), Sense::hover());
                        ui.painter()
                            .circle_filled(dot_r.center(), r, indicator.color(ui));
                        let msg = if spinning {
                            format!("{status_msg} · Syncing…")
                        } else {
                            status_msg.to_string()
                        };
                        ui.label(
                            RichText::new(msg).size(13.0).strong().color(t.fg()),
                        );
                    });

                    // ── Storage ──────────────────────────────────────────
                    if let Some((used, cap, frac)) = usage {
                        ui.add_space(10.0);

                        let pct = (frac * 100.0).round().clamp(0.0, 100.0) as i32;
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("Storage")
                                    .size(11.0)
                                    .color(t.text_muted()),
                            );
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.label(
                                    RichText::new(format!("{pct}%"))
                                        .size(11.0)
                                        .color(t.text_muted()),
                                );
                            });
                        });
                        ui.label(
                            RichText::new(format!("{used} of {cap}"))
                                .size(13.0)
                                .strong()
                                .color(t.fg()),
                        );

                        let bar_color = if frac >= 0.7 {
                            ui.ctx().get_lb_theme().fg().yellow
                        } else {
                            t.accent()
                        };
                        let mini_h = 4.0;
                        let (mr, _) =
                            ui.allocate_exact_size(vec2(ui.available_width(), mini_h), Sense::hover());
                        ui.painter()
                            .rect_filled(mr, mini_h / 2.0, t.surface_raised());
                        let fill_w = (mr.width() * frac).clamp(0.0, mr.width());
                        if fill_w > 0.0 {
                            ui.painter().rect_filled(
                                egui::Rect::from_min_size(mr.min, vec2(fill_w, mini_h)),
                                mini_h / 2.0,
                                bar_color,
                            );
                        }
                    }

                    // ── Account: one strong tier + one muted ·-joined line ─
                    if let Some(account) = account {
                        ui.add_space(10.0);
                        ui.label(
                            RichText::new("Account").size(11.0).color(t.text_muted()),
                        );
                        // e.g. "Free · Expired · App Store" or "Premium · Renews … · Google Play"
                        let mut line = account.tier.label().to_string();
                        if let Some(detail) = &account.detail {
                            line.push_str(" · ");
                            line.push_str(detail);
                        }
                        if let Some(source) = account.source {
                            line.push_str(" · ");
                            line.push_str(source);
                        }
                        ui.label(RichText::new(line).size(13.0).strong().color(t.fg()));
                    }
                });
        });
}
