//! Sidebar sync footer: status text, usage bar, settings entry.
//!
//! Vertical rhythm (one pad `V_PAD` everywhere it appears):
//!   pad → [usage bar → pad] → status row → pad
//!
//! Status row: colored dot · message · icon-only sync (+ settings). Indicator
//! During sync the **dot stays green**
//! and the arrows spin. Usage strings come from lb `readable` (SI / 1000), not
//! binary GiB formatting.

use egui::{Align2, Sense, Ui, pos2, vec2};
use lb::model::api::FREE_TIER_USAGE_SIZE;
use lb::subscribers::status::Status;

use crate::components::{
    FG_HOVER, Radius, Space, Spacer, Theme, ThemeExt, TypeRole, control_height, icon_button,
    phosphor, phosphor_ui_font_id, tip_card_placed, tip_text,
};

use crate::shell::ShellApp;
use crate::shell::action::Action;
use crate::shell::action::Action as A;

/// Local chrome: hold last quiet message while syncing; spin arrows after tap.
#[derive(Default)]
pub struct SyncFooterState {
    /// Last non-syncing status string (held while a pulse runs).
    stable_message: String,
    /// Manual sync keeps arrows spinning until status reports idle.
    spin_until_idle: bool,
}

/// Compact status glyph + optional label.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SyncIndicator {
    Synced,
    Syncing,
    Offline,
    OutOfSpace,
    UpdateRequired,
    SyncError,
}

impl SyncIndicator {
    fn from_status(status: &Status) -> Self {
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

    fn short_label(self) -> &'static str {
        match self {
            Self::Synced => "Up to date",
            Self::Syncing => "Syncing…",
            Self::Offline => "Offline",
            Self::OutOfSpace => "No space",
            Self::UpdateRequired => "Update required",
            Self::SyncError => "Error",
        }
    }

    /// Dot color. Attention only — activity is the spinning arrows.
    fn color(self, t: &Theme) -> egui::Color32 {
        use workspace_rs::theme::palette_v2::Palette;
        match self {
            Self::Synced | Self::Syncing => t.fg().get_color(Palette::Green),
            Self::Offline => t.neutral_fg_secondary(),
            Self::OutOfSpace => t.fg().get_color(Palette::Yellow),
            Self::UpdateRequired | Self::SyncError => t.danger(),
        }
    }
}

/// Vertical air (above / between / below) via Space tokens.
const V_PAD: Space = Space::Sm;
/// Inset for usage track + status row content.
const PAD_X: Space = Space::Sm;
/// Usage track thickness.
const BAR_H: f32 = Space::Xxs.pts() * 3.0;
const BAR_WARN_FRAC: f32 = 0.7;

/// Parent-owned footer band (status row + pads, optional usage track).
pub fn height(show_usage: bool) -> f32 {
    let mut h = V_PAD.pts() + control_height() + V_PAD.pts();
    if show_usage {
        h += BAR_H + V_PAD.pts();
    }
    h
}

pub fn show(app: &mut ShellApp, ui: &mut Ui, t: &Theme, queue: &mut Vec<Action>) {
    let Some(ready) = app.session.ready() else {
        return;
    };

    let status = ready.status.clone();
    let username = ready.workspace.account.username.clone();
    let show_usage = app.settings.sidebar_usage;
    let settings_on = matches!(app.modal, Some(crate::shell::Modal::Settings { .. }));

    if !status.syncing {
        if let Some(msg) = status.msg() {
            if !msg.is_empty() {
                app.sync_footer.stable_message = msg;
            }
        }
        app.sync_footer.spin_until_idle = false;
    }

    let raw = SyncIndicator::from_status(&status);
    // Dot: don't flip green→blue while syncing; arrows show activity.
    let display = if raw == SyncIndicator::Syncing { SyncIndicator::Synced } else { raw };

    let message = if !app.sync_footer.stable_message.is_empty() {
        app.sync_footer.stable_message.clone()
    } else if let Some(msg) = status.msg() {
        msg
    } else {
        display.short_label().to_string()
    };

    let spinning = status.syncing || app.sync_footer.spin_until_idle;
    if spinning {
        ui.ctx().request_repaint();
    }

    // Prefer server `readable` (SI 1000) so premium cap shows **30 GB** not 27.9.
    let usage_info = status.space_used.as_ref().and_then(|usage| {
        let cap = usage.data_cap.exact;
        if cap == 0 {
            return None;
        }
        let frac = (usage.server_usage.exact as f64 / cap as f64).min(1.0) as f32;
        Some((usage.server_usage.readable.clone(), usage.data_cap.readable.clone(), frac))
    });

    let footer_top = ui.cursor().top();
    let footer_left = ui.max_rect().left();
    let footer_right = ui.max_rect().right();

    ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
    ui.add(Spacer::new(V_PAD));

    // Pref on → always draw the bar when metrics exist (no density frac gate).
    if show_usage {
        if let Some((_, _, frac)) = &usage_info {
            let bar_color = if *frac >= BAR_WARN_FRAC {
                t.fg()
                    .get_color(workspace_rs::theme::palette_v2::Palette::Yellow)
            } else {
                t.accent()
            };
            let (br, _) = ui
                .allocate_exact_size(vec2(crate::components::ui_width(ui), BAR_H), Sense::hover());
            // L/R pads as placed Spacers (F2-visible); track fills mid.
            let px = PAD_X.pts();
            Spacer::paint_at(ui, PAD_X, egui::Rect::from_min_size(br.min, vec2(px, BAR_H)));
            Spacer::paint_at(
                ui,
                PAD_X,
                egui::Rect::from_min_size(pos2(br.right() - px, br.top()), vec2(px, BAR_H)),
            );
            let track = egui::Rect::from_min_size(
                pos2(br.left() + px, br.center().y - BAR_H / 2.0),
                vec2((br.width() - 2.0 * px).max(0.0), BAR_H),
            );
            let r = BAR_H / 2.0; // full capsule
            ui.painter().rect_filled(track, r, t.neutral_bg_tertiary());
            // Clip fill so partial progress keeps capsule ends (no square corners).
            let fill_w = (track.width() * *frac).max(0.0);
            let fill = egui::Rect::from_min_size(track.min, vec2(fill_w, track.height()));
            ui.painter()
                .with_clip_rect(track)
                .rect_filled(fill, r, bar_color);
            ui.add(Spacer::new(V_PAD));
        }
    }

    // Status row: pad · dot · message · … · sync · settings · pad
    // Parent claims full-width band; L/R pads are placed Spacers (not paint offsets).
    let row_h = control_height();
    let (rect, _) =
        ui.allocate_exact_size(vec2(crate::components::ui_width(ui), row_h), Sense::hover());
    let px = PAD_X.pts();
    Spacer::paint_at(ui, PAD_X, egui::Rect::from_min_size(rect.min, vec2(px, row_h)));
    Spacer::paint_at(
        ui,
        PAD_X,
        egui::Rect::from_min_size(pos2(rect.right() - px, rect.top()), vec2(px, row_h)),
    );
    let cy = rect.center().y;
    let mut x = rect.left() + px;

    let dot_r = 4.0;
    ui.painter()
        .circle_filled(pos2(x + dot_r, cy), dot_r, display.color(t));
    x += 2.0 * dot_r + Space::Xs.pts();

    let icon_sz = control_height();
    let icons_w = icon_sz * 2.0 + Space::Xs.pts();
    let icon_left = rect.right() - px - icons_w;
    let msg_max = (icon_left - Space::Xs.pts() - x).max(0.0);
    if msg_max > 0.0 {
        let mut job = egui::text::LayoutJob {
            wrap: egui::text::TextWrapping {
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
            egui::text::TextFormat {
                font_id: TypeRole::Body.font_id(),
                color: t.neutral_fg_secondary(),
                ..Default::default()
            },
        );
        let msg_g = ui.fonts(|f| f.layout_job(job));
        ui.painter()
            .galley(pos2(x, cy - msg_g.size().y / 2.0), msg_g, t.neutral_fg_secondary());
    }

    // Sync control (spinning arrows) + settings.
    let ground = t.neutral_bg_secondary();
    let sync_rect =
        egui::Rect::from_min_size(pos2(icon_left, cy - icon_sz / 2.0), vec2(icon_sz, icon_sz));
    let gear_rect = egui::Rect::from_min_size(
        pos2(icon_left + icon_sz + Space::Xs.pts(), cy - icon_sz / 2.0),
        vec2(icon_sz, icon_sz),
    );

    // Manual paint sync button so we can rotate the glyph while spinning.
    let sync_id = ui.id().with("footer_sync");
    let sync_resp = ui.interact(sync_rect, sync_id, crate::components::sense_click());
    let sync_hover =
        ui.ctx()
            .animate_bool_with_time(sync_resp.id.with("hov"), sync_resp.hovered(), 0.08);
    if sync_hover > 0.0 {
        ui.painter().rect_filled(
            sync_rect,
            Radius::Control.corner(),
            t.wash_toward_neutral_fg(ground, FG_HOVER * sync_hover),
        );
    }
    let ink = t
        .neutral_fg_secondary()
        .lerp_to_gamma(t.neutral_fg(), sync_hover);
    let ag = ui.painter().layout_no_wrap(
        phosphor::ARROWS_CLOCKWISE.into(),
        phosphor_ui_font_id(),
        egui::Color32::PLACEHOLDER,
    );
    let apos = sync_rect.center() - ag.size() / 2.0;
    if spinning {
        let angle = (ui.input(|i| i.time) as f32 * std::f32::consts::TAU) % std::f32::consts::TAU;
        let shape = egui::epaint::TextShape::new(apos, ag, ink)
            .with_override_text_color(ink)
            .with_angle_and_anchor(angle, Align2::CENTER_CENTER);
        ui.painter().add(shape);
    } else {
        ui.painter().galley(apos, ag, ink);
    }
    tip_text(ui.ctx(), &sync_resp, "Sync now");
    if sync_resp.clicked() {
        app.sync_footer.spin_until_idle = true;
        queue.push(A::RequestSync);
    }

    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(gear_rect)
            .layout(egui::Layout::centered_and_justified(egui::Direction::TopDown)),
        |ui| {
            let gear = icon_button(ui, t, phosphor::GEAR, settings_on, ground);
            tip_text(ui.ctx(), &gear, "Settings (⌘,)");
            if gear.clicked() {
                if settings_on {
                    queue.push(A::CloseModal);
                } else {
                    queue.push(A::OpenSettings);
                }
            }
        },
    );

    ui.add(Spacer::new(V_PAD));

    // Hit = passive strip (status / usage). Place = full footer so card centers
    // on the sidebar with equal side air (not flush to the window).
    let footer_bot = ui.cursor().top();
    let footer_rect =
        egui::Rect::from_min_max(pos2(footer_left, footer_top), pos2(footer_right, footer_bot));
    let passive_right = icon_left - Space::Xs.pts();
    let passive_rect = egui::Rect::from_min_max(
        pos2(footer_left, footer_top),
        pos2(passive_right.max(footer_left + Space::Xl.pts()), footer_bot),
    );
    let hit = ui.interact(passive_rect, ui.id().with("sync_footer_passive"), Sense::hover());
    let gap = Space::Xs.pts();
    let frame_pad_x = Space::Sm.pts();
    let content_w = (footer_rect.width() - 2.0 * gap - 2.0 * frame_pad_x).max(Space::Xl.pts());
    let tier_free = status
        .space_used
        .as_ref()
        .map(|u| u.data_cap.exact <= FREE_TIER_USAGE_SIZE)
        .unwrap_or(true);
    let usage = usage_info
        .as_ref()
        .map(|(u, c, f)| (u.clone(), c.clone(), *f));
    let uname = username.clone();
    tip_card_placed(ui.ctx(), &hit, footer_rect, content_w, |ui| {
        ui.set_width(content_w);
        ui.set_max_width(content_w);
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        let t = ui.ctx().get_lb_theme();

        if let Some((used, cap, frac)) = usage {
            let pct = (frac * 100.0).round().clamp(0.0, 100.0) as i32;
            ui.label(
                TypeRole::Mono
                    .rich("Storage")
                    .color(t.neutral_fg_secondary()),
            );
            ui.label(
                TypeRole::Body
                    .rich(format!("{used} of {cap} ({pct}%)"))
                    .color(t.neutral_fg()),
            );
            ui.add(Spacer::new(Space::Sm));
        }

        ui.label(
            TypeRole::Mono
                .rich("Account")
                .color(t.neutral_fg_secondary()),
        );
        // Name · tier on one line (absolute x — no horizontal placer).
        let name_g =
            ui.painter()
                .layout_no_wrap(uname.clone(), TypeRole::Body.font_id(), t.neutral_fg());
        let tier_g = ui.painter().layout_no_wrap(
            if tier_free { "Free" } else { "Premium" }.into(),
            TypeRole::Mono.font_id(),
            t.neutral_fg_secondary(),
        );
        let name_w = name_g.size().x;
        let tier_w = tier_g.size().x;
        let lh = TypeRole::Body
            .line_height()
            .max(name_g.size().y)
            .max(tier_g.size().y);
        let gap = Space::Xs.pts();
        let row_w = name_w + gap + tier_w;
        let (row, _) = ui.allocate_exact_size(vec2(row_w, lh), Sense::hover());
        ui.painter().galley(
            pos2(row.left(), row.center().y - name_g.size().y / 2.0),
            name_g,
            t.neutral_fg(),
        );
        Spacer::paint_at(
            ui,
            Space::Xs,
            egui::Rect::from_min_size(pos2(row.left() + name_w, row.top()), vec2(gap, lh)),
        );
        ui.painter().galley(
            pos2(row.left() + name_w + gap, row.center().y - tier_g.size().y / 2.0),
            tier_g,
            t.neutral_fg_secondary(),
        );
    });
}
