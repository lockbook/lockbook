use std::collections::HashMap;

use lazy_static::lazy_static;
use prometheus::IntGaugeVec;
use prometheus::register_int_gauge_vec;
use reqwest::Client;
use serde::Deserialize;
use tracing::*;

use crate::get;

lazy_static! {
    static ref INSTALLS: IntGaugeVec =
        register_int_gauge_vec!("flathub_installs", "Flathub install counts", &["period"]).unwrap();
    static ref INSTALLS_BY_COUNTRY: IntGaugeVec = register_int_gauge_vec!(
        "flathub_installs_by_country",
        "Flathub installs by country",
        &["country"]
    )
    .unwrap();
}

#[derive(Deserialize)]
struct Stats {
    installs_total: i64,
    installs_last_month: i64,
    installs_last_7_days: i64,
    #[serde(default)]
    installs_per_country: HashMap<String, i64>,
}

pub async fn refresh(client: &Client) {
    info!("refreshing flathub metrics");

    let url = "https://flathub.org/api/v2/stats/net.lockbook.Lockbook";
    let Some(stats) = get::<Stats>(client, url, "").await else {
        return;
    };

    INSTALLS
        .with_label_values(&["total"])
        .set(stats.installs_total);
    INSTALLS
        .with_label_values(&["last_month"])
        .set(stats.installs_last_month);
    INSTALLS
        .with_label_values(&["last_7_days"])
        .set(stats.installs_last_7_days);

    for (country, count) in &stats.installs_per_country {
        INSTALLS_BY_COUNTRY
            .with_label_values(&[country])
            .set(*count);
    }
}
