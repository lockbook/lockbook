use lb_rs::model::api::{
    AppStoreAccountState, GooglePlayAccountState, PaymentPlatform, SubscriptionInfo,
};
use lb_rs::model::usage::bytes_to_human;
use lb_rs::service::usage::{UsageItemMetric, UsageMetrics};

use crate::widgets::ProgressBar;

pub fn subscription(
    ui: &mut egui::Ui, maybe_sub_info: &Option<SubscriptionInfo>, metrics: &UsageMetrics,
    maybe_uncompressed: Option<&UsageItemMetric>,
) -> Option<SubscriptionResponse> {
    let stroke_color = ui.visuals().extreme_bg_color;
    let bg = ui.visuals().faint_bg_color;

    egui::Frame::none()
        .fill(bg)
        .stroke(egui::Stroke::new(2.0, stroke_color))
        .rounding(egui::Rounding::same(4.0))
        .inner_margin(12.0)
        .show(ui, |ui| {
            let resp = subscription_info(ui, maybe_sub_info);
            ui.add_space(12.0);
            usage_bar(ui, metrics, maybe_uncompressed);
            resp
        })
        .inner
}

fn subscription_info(
    ui: &mut egui::Ui, maybe_sub_info: &Option<SubscriptionInfo>,
) -> Option<SubscriptionResponse> {
    use PaymentPlatform::*;

    match maybe_sub_info {
        Some(info) => match &info.payment_platform {
            Stripe { card_last_4_digits } => draw_stripe(ui, card_last_4_digits),
            GooglePlay { account_state } => draw_google_play(ui, account_state),
            AppStore { account_state } => draw_app_store(ui, account_state),
        },
        None => {
            draw_free_tier(ui);
            None
        }
    }
}

fn draw_free_tier(ui: &mut egui::Ui) {
    ui.heading("Free");
}

fn draw_stripe(ui: &mut egui::Ui, last4: &str) -> Option<SubscriptionResponse> {
    ui.heading(format!("Stripe ({last4})"));
    None
}

fn draw_google_play(
    ui: &mut egui::Ui, account_state: &GooglePlayAccountState,
) -> Option<SubscriptionResponse> {
    ui.heading(format!("Google Play ({account_state:?})"));
    None
}

fn draw_app_store(
    ui: &mut egui::Ui, account_state: &AppStoreAccountState,
) -> Option<SubscriptionResponse> {
    ui.heading(format!("App Store ({account_state:?})"));
    None
}

fn usage_bar(
    ui: &mut egui::Ui, metrics: &UsageMetrics, maybe_uncompressed: Option<&UsageItemMetric>,
) {
    let used = metrics.server_usage.exact as f32;
    let available = metrics.data_cap.exact as f32;
    let human_usage = bytes_to_human(used as u64);
    let percent = used / available;

    ui.horizontal(|ui| {
        ui.columns(2, |uis| {
            uis[0].label(format!("{}    ({:.2} %)", human_usage, percent * 100.0));

            uis[1].with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                ui.label(bytes_to_human(available as u64));
            });
        });
    });

    ui.add_space(5.0);

    let pbar_resp = ProgressBar::new().percent(percent).show(ui);

    if let Some(uncompressed) = maybe_uncompressed {
        pbar_resp.on_hover_ui(|ui| {
            egui::Grid::new("compression_stats").show(ui, |ui| {
                let compr_ratio = match metrics.server_usage.exact {
                    0 => "0".to_string(),
                    _ => format!("{:.2}x", uncompressed.exact as f32 / used),
                };

                ui.label("Uncompressed Usage: ");
                ui.label(&uncompressed.readable);
                ui.end_row();

                ui.label("Compression Ratio: ");
                ui.label(&compr_ratio);
                ui.end_row();
            });
        });
    }
}

pub enum SubscriptionResponse {
    //Cancel,
}
