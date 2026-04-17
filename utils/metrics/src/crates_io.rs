use reqwest::Client;
use serde::Deserialize;
use tracing::*;

use crate::get;
use crate::metrics::INSTALLS;

#[derive(Deserialize)]
struct CrateResponse {
    #[serde(rename = "crate")]
    krate: CrateInfo,
}

#[derive(Deserialize)]
struct CrateInfo {
    downloads: i64,
}

async fn fetch_downloads(client: &Client, name: &str) -> Option<i64> {
    let url = format!("https://crates.io/api/v1/crates/{name}");
    let resp = get::<CrateResponse>(client, &url, "").await?;
    Some(resp.krate.downloads)
}

pub async fn refresh(client: &Client) {
    info!("refreshing crates.io metrics");

    // `lockbook` crate downloads roll up into the CLI client (the lockbook
    // binary crate is the CLI). crates.io has no OS attribution, so leave
    // the os label empty.
    if let Some(downloads) = fetch_downloads(client, "lockbook").await {
        INSTALLS
            .with_label_values(&["crates_io", "cli", "", ""])
            .set(downloads);
    }

    // `lb-rs` is the Rust library client — its own client type.
    if let Some(downloads) = fetch_downloads(client, "lb-rs").await {
        INSTALLS
            .with_label_values(&["crates_io", "lb-rs", "", ""])
            .set(downloads);
    }
}
