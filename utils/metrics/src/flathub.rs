use std::collections::HashMap;

use reqwest::Client;
use serde::Deserialize;
use tracing::*;

use crate::get;
use crate::metrics::INSTALLS;

#[derive(Deserialize)]
struct Stats {
    installs_total: i64,
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
        .with_label_values(&["flathub", "lockbook", ""])
        .set(stats.installs_total);

    for (country, count) in &stats.installs_per_country {
        INSTALLS
            .with_label_values(&["flathub", "lockbook", country])
            .set(*count);
    }
}
