use std::sync::mpsc;

use egui_extras::{Size, StripBuilder};
use lb::model::api::{PaymentMethod, StripeAccountTier, SubscriptionInfo};
use lb::service::usage::{UsageItemMetric, UsageMetrics};
use workspace_rs::theme::icons::Icon;
use workspace_rs::theme::palette::ThemePalette;
use workspace_rs::widgets::{separator, subscription};

use super::SettingsResponse;

pub struct UsageSettings {
    pub info: Option<UsageSettingsInfo>,
    pub info_rx: mpsc::Receiver<UsageSettingsInfo>,
    pub upgrading: Option<Upgrading>,
}

pub struct UsageSettingsInfo {
    pub sub_info_result: Result<Option<SubscriptionInfo>, String>,
    pub metrics_result: Result<UsageMetrics, String>,
    pub uncompressed_result: Result<UsageItemMetric, String>,
}

pub struct Upgrading {
    update_rx: mpsc::Receiver<Result<UsageSettingsInfo, String>>,
    update_tx: mpsc::Sender<Result<UsageSettingsInfo, String>>,
    stage: UpgradingStage,
    card: CardInput,
    payment_method: Option<PaymentMethod>,
    done: Option<Result<(), String>>,
}

#[derive(Default)]
struct CardInput {
    number: String,
    cvc: String,
    exp_month: String,
    exp_year: String,
    error: Option<String>,
}

#[derive(PartialOrd, Eq, PartialEq)]
enum UpgradingStage {
    EnterPaymentInfo,
    ConfirmPaymentMethod,
    Paying,
}

enum Action {
    Prev,
    Next,
}

impl super::SettingsModal {
    pub fn show_usage_tab(&mut self, ui: &mut egui::Ui) -> Option<SettingsResponse> {
        let mut resp = None;

        if let Some(u) = &mut self.usage.upgrading {
            while let Ok(result) = u.update_rx.try_recv() {
                match result {
                    Ok(new_usage_info) => {
                        self.usage.info = Some(new_usage_info);
                        u.done = Some(Ok(()));
                        resp = Some(SettingsResponse::SuccessfullyUpgraded);
                    }
                    Err(err) => u.done = Some(Err(err)),
                }
            }

            u.show_header(ui);

            match u.stage {
                UpgradingStage::EnterPaymentInfo => match u.show_payment_selection(ui) {
                    Some(Action::Prev) => self.usage.upgrading = None,
                    Some(Action::Next) => u.validate_method(),
                    None => {}
                },
                UpgradingStage::ConfirmPaymentMethod => match u.show_confirm_payment(ui) {
                    Some(Action::Prev) => u.stage = UpgradingStage::EnterPaymentInfo,
                    Some(Action::Next) => {
                        u.stage = UpgradingStage::Paying;

                        let core = self.core.clone();
                        let method = u.payment_method.take().unwrap();
                        let update_tx = u.update_tx.clone();
                        let ctx = ui.ctx().clone();
                        std::thread::spawn(move || {
                            match core.upgrade_account_stripe(StripeAccountTier::Premium(method)) {
                                Ok(()) => {
                                    let sub_info_result = core
                                        .get_subscription_info()
                                        .map_err(|err| format!("{err:?}")); // TODO

                                    let metrics_result =
                                        core.get_usage().map_err(|err| format!("{err:?}")); // TODO
                                    let uncompressed_result = core
                                        .get_uncompressed_usage()
                                        .map_err(|err| format!("{err:?}")); // TODO

                                    let new_usage_data = UsageSettingsInfo {
                                        sub_info_result,
                                        metrics_result,
                                        uncompressed_result,
                                    };

                                    update_tx.send(Ok(new_usage_data)).unwrap();
                                }
                                Err(err) => update_tx.send(Err(format!("{err:?}"))).unwrap(),
                            }
                            ctx.request_repaint();
                        });
                    }
                    None => {}
                },
                UpgradingStage::Paying => {
                    if let Some(()) = u.show_paying(ui) {
                        self.usage.upgrading = None;
                    }
                }
            }
        } else if let Some(info) = &self.usage.info {
            let metrics = match &info.metrics_result {
                Ok(m) => m,
                Err(err) => {
                    ui.label(err);
                    return None;
                }
            };

            let uncompressed = match &info.uncompressed_result {
                Ok(v) => v,
                Err(err) => {
                    ui.label(err);
                    return None;
                }
            };

            match &info.sub_info_result {
                Ok(maybe_info) => {
                    subscription(ui, maybe_info, metrics, Some(uncompressed));

                    if maybe_info.is_none() {
                        ui.add_space(25.0);
                        ui.separator();
                        ui.add_space(25.0);

                        ui.heading("Become a Premium user!");
                        ui.add_space(7.0);

                        ui.label("Expand your storage to 30 GB for just $2.99 / month.");
                        ui.add_space(10.0);

                        if ui.button("Upgrade").clicked() {
                            self.usage.upgrading = Some(Upgrading::new());
                            ui.ctx().request_repaint();
                        }
                    }
                }
                Err(err) => {
                    ui.label(err);
                }
            };
        } else {
            while let Ok(usage_info) = self.usage.info_rx.try_recv() {
                self.usage.info = Some(usage_info);
                ui.ctx().request_repaint();
            }
            ui.centered_and_justified(|ui| ui.spinner());
        }

        resp
    }
}

impl Upgrading {
    fn new() -> Self {
        let (update_tx, update_rx) = mpsc::channel();

        Self {
            update_rx,
            update_tx,
            stage: UpgradingStage::EnterPaymentInfo,
            card: CardInput::default(),
            payment_method: None,
            done: None,
        }
    }

    fn show_header(&self, ui: &mut egui::Ui) {
        use UpgradingStage::*;

        ui.columns(3, |uis| {
            self.header_label(&mut uis[0], EnterPaymentInfo, "Enter Info", Icon::INFO);
            self.header_label(&mut uis[1], ConfirmPaymentMethod, "Confirm", Icon::THUMBS_UP);
            self.header_label(&mut uis[2], Paying, "Payment", Icon::MONEY);
        });

        separator(ui);
    }

    fn header_label(&self, ui: &mut egui::Ui, v: UpgradingStage, text: &str, icon: Icon) {
        ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
            let is_active = self.stage == v;

            let icon =
                if v < self.stage { Icon::DONE.color(ThemePalette::DARK.green) } else { icon };

            ui.add_space(10.0);
            icon.size(24.0).weak(!is_active).show(ui);
            ui.add_space(10.0);

            if is_active {
                ui.label(text);
            } else {
                ui.weak(text);
            }

            ui.add_space(10.0);
        });
    }

    fn show_payment_selection(&mut self, ui: &mut egui::Ui) -> Option<Action> {
        const INPUT_MARGIN: egui::Vec2 = egui::vec2(8.0, 8.0);

        let mut resp = None;

        let text_height = ui.text_style_height(&egui::TextStyle::Body);

        let show_action_buttons = |ui: &mut egui::Ui| {
            ui.columns(2, |uis| {
                if uis[0].button("Cancel").clicked() {
                    resp = Some(Action::Prev);
                }
                if uis[1].button("Next").clicked() {
                    resp = Some(Action::Next);
                }
            });
        };

        let show_input = |ui: &mut egui::Ui| {
            ui.add_space(20.0);
            ui.label("Please enter your credit card information:");
            ui.add_space(20.0);

            ui.horizontal(|ui| {
                ui.set_height(text_height + INPUT_MARGIN.y * 2.0);

                StripBuilder::new(ui)
                    .size(Size::remainder())
                    .size(Size::exact(55.0))
                    .size(Size::exact(55.0))
                    .size(Size::exact(55.0))
                    .horizontal(|mut strip| {
                        strip.cell(|ui| {
                            ui.add(
                                egui::TextEdit::singleline(&mut self.card.number)
                                    .margin(INPUT_MARGIN)
                                    .hint_text("Card Number"),
                            );
                        });
                        strip.cell(|ui| {
                            ui.add(
                                egui::TextEdit::singleline(&mut self.card.exp_month)
                                    .margin(INPUT_MARGIN)
                                    .hint_text("MM"),
                            );
                        });
                        strip.cell(|ui| {
                            ui.add(
                                egui::TextEdit::singleline(&mut self.card.exp_year)
                                    .margin(INPUT_MARGIN)
                                    .hint_text("YY"),
                            );
                        });
                        strip.cell(|ui| {
                            ui.add(
                                egui::TextEdit::singleline(&mut self.card.cvc)
                                    .margin(INPUT_MARGIN)
                                    .hint_text("CVC"),
                            );
                        });
                    });
            });

            if let Some(msg) = &self.card.error {
                ui.add_space(20.0);
                ui.label(msg);
            }
        };

        StripBuilder::new(ui)
            .size(Size::remainder())
            .size(Size::exact(40.0))
            .vertical(|mut strip| {
                strip.cell(show_input);
                strip.cell(show_action_buttons);
            });

        resp
    }

    fn validate_method(&mut self) {
        let CardInput { number, exp_month, exp_year, cvc, error } = &mut self.card;

        let number = number.to_string();
        if number.is_empty() {
            *error = Some("Invalid card number".to_string());
            return;
        }

        let exp_month = match exp_month.parse() {
            Ok(exp_month) => exp_month,
            Err(_err) => {
                *error = Some("Invalid expiry month".to_string());
                return;
            }
        };

        let exp_year = match exp_year.parse() {
            Ok(exp_year) => exp_year,
            Err(_err) => {
                *error = Some("Invalid expiry year".to_string());
                return;
            }
        };

        let cvc = cvc.to_string();
        if number.is_empty() {
            *error = Some("CVC is empty".to_string());
            return;
        }

        self.payment_method = Some(PaymentMethod::NewCard { number, exp_month, exp_year, cvc });
        self.stage = UpgradingStage::ConfirmPaymentMethod;
    }

    fn show_confirm_payment(&self, ui: &mut egui::Ui) -> Option<Action> {
        let mut resp = None;

        StripBuilder::new(ui)
            .size(Size::remainder())
            .size(Size::exact(40.0))
            .vertical(|mut strip| {
                strip.cell(|ui| {
                    if let Some(PaymentMethod::NewCard { number, .. }) = &self.payment_method {
                        ui.vertical_centered(|ui| {
                            ui.add_space(50.0);
                            ui.label(format!(
                                "Use card (ending in {})",
                                &number[number.len() - 4..]
                            ));

                            ui.add_space(10.0);
                            ui.label("to pay $2.99 / month");

                            ui.add_space(10.0);
                            ui.label("for 30 GB of storage?");
                        });
                    }
                });
                strip.cell(|ui| {
                    ui.columns(2, |uis| {
                        if uis[0].button("Back").clicked() {
                            resp = Some(Action::Prev);
                        }
                        if uis[1].button("Confirm").clicked() {
                            resp = Some(Action::Next);
                        }
                    });
                });
            });

        resp
    }

    fn show_paying(&self, ui: &mut egui::Ui) -> Option<()> {
        let mut resp = None;

        ui.vertical_centered(|ui| {
            ui.add_space(50.0);
            match &self.done {
                Some(result) => match result {
                    Ok(()) => {
                        ui.label("Success!");
                        ui.add_space(10.0);
                        if ui.button("Done").clicked() {
                            resp = Some(());
                        }
                    }
                    Err(err) => {
                        ui.label(err);
                    }
                },
                None => {
                    ui.spinner();
                    ui.add_space(10.0);
                    ui.label("Paying...");
                }
            };
        });

        resp
    }
}
