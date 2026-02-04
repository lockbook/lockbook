use std::thread;
use std::time::{Duration, Instant};

use egui::{Color32, TextWrapMode};
use lb::model::usage::bytes_to_human;
use lb::service::usage::UsageMetrics;
use workspace_rs::theme::icons::Icon;
use workspace_rs::widgets::{Button, ProgressBar};

use super::AccountUpdate;

pub struct SyncPanel {
    usage_msg_gained_hover: Option<Instant>,
    expanded_usage_msg_rect: egui::Rect,
}

impl SyncPanel {
    pub fn new() -> Self {
        Self { usage_msg_gained_hover: None, expanded_usage_msg_rect: egui::Rect::NOTHING }
    }
}

impl super::AccountScreen {
    pub fn show_usage_panel(&mut self, ui: &mut egui::Ui) {
        if self.settings.read().unwrap().sidebar_usage {
            if let Some(usage) = &self.lb_status.space_used {
                let usage = Usage::from(usage);
                egui::Frame::none().show(ui, |ui| {
                    let is_throttled_hover =
                        if let Some(hover_origin) = self.sync.usage_msg_gained_hover {
                            let throttle_duration = Duration::from_millis(100);
                            (Instant::now() - hover_origin) > throttle_duration
                        } else {
                            false
                        };

                    let text = if is_throttled_hover {
                        format!("{:.1}% used", usage.percent * 100.)
                    } else {
                        format!("{} out of {} used", usage.used, usage.available)
                    };

                    let text: egui::WidgetText = text.into();
                    let text = text.color(ui.visuals().text_color().linear_multiply(0.8));
                    let galley = text.into_galley(
                        ui,
                        Some(TextWrapMode::Extend),
                        ui.available_width(),
                        egui::TextStyle::Small,
                    );

                    let desired_size = egui::vec2(galley.size().x, galley.size().y);
                    let (rect, resp) = ui.allocate_at_least(desired_size, egui::Sense::click());

                    if self.sync.usage_msg_gained_hover.is_none()
                        && !self.sync.expanded_usage_msg_rect.eq(&rect)
                    {
                        self.sync.expanded_usage_msg_rect = rect;
                    }

                    ui.painter().galley(
                        rect.left_top(),
                        galley,
                        ui.style().interact(&resp).text_color(),
                    );

                    if self
                        .sync
                        .expanded_usage_msg_rect
                        .expand(5.0)
                        .contains(ui.input(|i| i.pointer.hover_pos().unwrap_or_default()))
                    {
                        if self.sync.usage_msg_gained_hover.is_none() {
                            self.sync.usage_msg_gained_hover = Some(Instant::now());
                        }
                    } else {
                        self.sync.usage_msg_gained_hover = None;
                    }

                    ui.add_space(8.0);

                    ProgressBar::new().percent(usage.percent).show(ui);
                });
            }
            ui.add_space(15.0);
        }
    }

    pub fn show_sync_btn(&mut self, ui: &mut egui::Ui) {
        let visuals_before_button = ui.style().clone();
        if self.lb_status.offline {
            ui.visuals_mut().widgets.active.bg_fill = Color32::GRAY;
        } else if self.lb_status.update_required || self.lb_status.out_of_space {
            ui.visuals_mut().widgets.active.bg_fill = ui.visuals().warn_fg_color;
        } else if self.lb_status.unexpected_sync_problem.is_some() {
            ui.visuals_mut().widgets.active.bg_fill = ui.visuals().error_fg_color;
        };

        // let text_stroke =
        //     egui::Stroke { color: ui.visuals().widgets.active.bg_fill, ..Default::default() };

        // ui.visuals_mut().widgets.inactive.fg_stroke = text_stroke;
        // ui.visuals_mut().widgets.hovered.fg_stroke = text_stroke;
        // ui.visuals_mut().widgets.active.fg_stroke = text_stroke;

        ui.visuals_mut().widgets.inactive.bg_fill =
            ui.visuals().widgets.active.bg_fill;
        // ui.visuals_mut().widgets.hovered.bg_fill =
        //     ui.visuals().widgets.active.bg_fill.gamma_multiply(0.2);

        // ui.visuals_mut().widgets.active.bg_fill =
        //    ui.visuals().widgets.active.bg_fill.gamma_multiply(0.3);

        let icon = if self.lb_status.offline {
            Icon::OFFLINE
        } else if self.lb_status.update_required || self.lb_status.out_of_space {
            Icon::SYNC_PROBLEM
        } else {
            Icon::SYNC
        };

        let sync_btn = Button::default()
            .text("Sync")
            .icon(&icon)
            .icon_alignment(egui::Align::RIGHT)
            .padding(egui::vec2(20.0, 7.0))
            .frame(true)
            .rounding(egui::Rounding::same(5.0))
            .is_loading(self.lb_status.syncing)
            .show(ui);

        if sync_btn.clicked() {
            self.workspace.tasks.queue_sync();
        }

        if sync_btn.hovered() {
            if let Some(msg) = self.lb_status.msg() {
                sync_btn.on_hover_text(msg);
            }
        }

        ui.set_style(visuals_before_button);
    }

    pub fn perform_final_sync(&self, ctx: &egui::Context) {
        let core = self.core.clone();
        let update_tx = self.update_tx.clone();
        let ctx = ctx.clone();

        thread::spawn(move || {
            if let Err(err) = core.sync(None) {
                eprintln!("error: final sync: {err:?}");
            }
            update_tx.send(AccountUpdate::FinalSyncAttemptDone).unwrap();
            ctx.request_repaint();
        });
    }
}

pub struct Usage {
    pub used: String,
    pub available: String,
    pub percent: f32,
}

impl From<&UsageMetrics> for Usage {
    fn from(metrics: &UsageMetrics) -> Self {
        let used = metrics.server_usage.exact;
        let available = metrics.data_cap.exact;

        Self {
            used: bytes_to_human(used),
            available: bytes_to_human(available),
            percent: used as f32 / available as f32,
        }
    }
}
