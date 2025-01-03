use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use egui::TextWrapMode;
use lb::model::errors::LbErr;
use workspace_rs::theme::icons::Icon;
use workspace_rs::widgets::{Button, ProgressBar};

use super::AccountUpdate;

pub struct SyncPanel {
    initial_status: Result<String, LbErr>,
    btn_lost_hover_after_sync: bool,
    lock: Arc<Mutex<()>>,
    usage_msg_gained_hover: Option<Instant>,
    expanded_usage_msg_rect: egui::Rect,
}

impl SyncPanel {
    pub fn new(initial_status: Result<String, LbErr>) -> Self {
        Self {
            initial_status,
            lock: Arc::new(Mutex::new(())),
            usage_msg_gained_hover: None,
            expanded_usage_msg_rect: egui::Rect::NOTHING,
            btn_lost_hover_after_sync: false,
        }
    }
}

impl super::AccountScreen {
    pub fn show_usage_panel(&mut self, ui: &mut egui::Ui) {
        if self.settings.read().unwrap().sidebar_usage {
            match &self.usage {
                Ok(usage) => {
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
                Err(_err) => {
                    // todo: should still display usage in offline
                }
            }
            ui.add_space(15.0);
        }
    }

    pub fn show_sync_btn(&mut self, ui: &mut egui::Ui) {
        let visuals_before_button = ui.style().clone();
        let text_stroke =
            egui::Stroke { color: ui.visuals().widgets.active.bg_fill, ..Default::default() };
        ui.visuals_mut().widgets.inactive.fg_stroke = text_stroke;
        ui.visuals_mut().widgets.hovered.fg_stroke = text_stroke;
        ui.visuals_mut().widgets.active.fg_stroke = text_stroke;

        ui.visuals_mut().widgets.inactive.bg_fill =
            ui.visuals().widgets.active.bg_fill.linear_multiply(0.1);
        ui.visuals_mut().widgets.hovered.bg_fill =
            ui.visuals().widgets.active.bg_fill.linear_multiply(0.2);

        ui.visuals_mut().widgets.active.bg_fill =
            ui.visuals().widgets.active.bg_fill.linear_multiply(0.3);

        let sync_btn = Button::default()
            .text("Sync")
            .icon(&Icon::SYNC)
            .icon_alignment(egui::Align::RIGHT)
            .padding(egui::vec2(10.0, 7.0))
            .frame(true)
            .rounding(egui::Rounding::same(5.0))
            .is_loading(self.workspace.status.syncing())
            .show(ui);

        if sync_btn.clicked() {
            self.workspace.perform_sync();
            self.sync.btn_lost_hover_after_sync = false;
        }

        if sync_btn.hover_pos().is_none() {
            self.sync.btn_lost_hover_after_sync = true;
        }

        let tooltip_msg = if !self.sync.btn_lost_hover_after_sync {
            self.workspace.status.sync_message.to_owned()
        } else {
            Some(format!("Updated {}", &self.workspace.status.dirtyness.last_synced))
        };

        if let Some(msg) = tooltip_msg {
            sync_btn.on_hover_ui_at_pointer(|ui| {
                ui.label(msg);
            });
        }

        ui.set_style(visuals_before_button);
    }

    pub fn show_sync_error_warn(&mut self, ui: &mut egui::Ui) {
        let msg = if let Err(err_msg) = &self.sync.initial_status {
            err_msg.kind.to_string()
        } else {
            let dirty_files_count = self.workspace.status.dirtyness.dirty_files.len();
            if dirty_files_count > 5 {
                format!(
                    "{} change{} needs to be synced",
                    dirty_files_count,
                    if dirty_files_count > 1 { "s" } else { "" }
                )
            } else {
                return;
            }
        };

        let color = if self.sync.initial_status.is_err() {
            ui.visuals().error_fg_color
        } else {
            ui.visuals().text_color()
        };

        egui::Frame::default()
            .fill(color.linear_multiply(0.1))
            .inner_margin(egui::Margin::symmetric(10.0, 7.0))
            .rounding(egui::Rounding::same(10.0))
            .show(ui, |ui| {
                ui.set_width(ui.available_size_before_wrap().x);
                ui.horizontal_wrapped(|ui| {
                    Icon::WARNING.color(color).show(ui);

                    ui.add_space(7.0);

                    let mut job = egui::text::LayoutJob::single_section(
                        msg,
                        egui::TextFormat::simple(egui::FontId::proportional(15.0), color),
                    );

                    job.wrap = egui::epaint::text::TextWrapping {
                        overflow_character: Some('…'),
                        max_rows: 1,
                        break_anywhere: true,
                        ..Default::default()
                    };
                    ui.label(job);
                });
            });
    }

    pub fn perform_final_sync(&self, ctx: &egui::Context) {
        let sync_lock = self.sync.lock.clone();
        let core = self.core.clone();
        let update_tx = self.update_tx.clone();
        let ctx = ctx.clone();

        thread::spawn(move || {
            let _lock = sync_lock.lock().unwrap();
            if let Err(err) = core.sync(None) {
                eprintln!("error: final sync: {:?}", err);
            }
            update_tx.send(AccountUpdate::FinalSyncAttemptDone).unwrap();
            ctx.request_repaint();
        });
    }
}
