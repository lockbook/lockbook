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

/// Crates to track on crates.io and the client type each rolls into.
/// `lockbook` is the CLI binary crate; `lb-rs` is the Rust library, its own
/// client type. crates.io has no OS attribution, so the os label is empty.
const CRATES: &[(&str, &str)] = &[("lockbook", "cli"), ("lb-rs", "lb-rs")];

async fn fetch_downloads(client: &Client, name: &str) -> Option<i64> {
    let url = format!("https://crates.io/api/v1/crates/{name}");
    let resp = get::<CrateResponse>(client, &url, "").await?;
    Some(resp.krate.downloads)
}

pub async fn refresh(client: &Client) {
    info!("refreshing crates.io metrics");

    for (crate_name, client_type) in CRATES {
        if let Some(downloads) = fetch_downloads(client, crate_name).await {
            INSTALLS
                .with_label_values(&["crates_io", client_type, "", ""])
                .set(downloads);
        }
    }
}
