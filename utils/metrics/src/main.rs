mod app_store;
mod env;
mod flathub;
mod github;
mod loggers;
mod metrics;

use std::sync::Arc;
use std::time::Duration;

use prometheus::TextEncoder;
use prometheus::gather;
use reqwest::Client;
use serde::de::DeserializeOwned;
use tokio::sync::Mutex;
use tokio::time::sleep;
use tracing::*;
use warp::Filter;
use warp::path;
use warp::reply::with_header;

use app_store::AppStoreState;

async fn get<T: DeserializeOwned>(client: &Client, url: &str, auth: &str) -> Option<T> {
    let mut req = client.get(url).header("User-Agent", "lb-metrics");
    if !auth.is_empty() {
        req = req.header("Authorization", auth);
    }
    let resp = req.send().await.ok()?;

    if !resp.status().is_success() {
        warn!("GET {url} returned {}", resp.status());
        return None;
    }

    resp.json().await.ok()
}

#[tokio::main]
async fn main() {
    loggers::init();
    info!("lb-metrics started");

    let config = env::Config::from_env();
    let port = config.port;
    let client = Client::new();

    // Initialize App Store state and run backfill
    let mut app_store_state = AppStoreState::new(&config.data_dir);
    app_store_state.backfill(&client, &config.app_store).await;

    // Initial metrics refresh
    info!("performing initial metrics refresh");
    github::refresh(&client, &config.github_token).await;
    flathub::refresh(&client).await;
    app_store_state.update_metrics();

    info!("backfill complete, starting metrics server");

    let app_store_state = Arc::new(Mutex::new(app_store_state));

    // Spawn refresh loop
    let refresh_state = app_store_state.clone();
    tokio::spawn(async move {
        loop {
            sleep(Duration::from_secs(300)).await;

            info!("refreshing metrics");
            github::refresh(&client, &config.github_token).await;
            flathub::refresh(&client).await;

            let mut state = refresh_state.lock().await;
            state.refresh(&client, &config.app_store).await;
            state.update_metrics();

            info!("metrics refresh complete");
        }
    });

    let metrics_route = path("metrics").and(warp::get()).map(|| {
        let encoder = TextEncoder::new();
        match encoder.encode_to_string(&gather()) {
            Ok(body) => with_header(body, "content-type", "text/plain; charset=utf-8"),
            Err(e) => {
                error!("failed to encode metrics: {e}");
                with_header(format!("error: {e}"), "content-type", "text/plain; charset=utf-8")
            }
        }
    });

    info!("lb-metrics listening on 127.0.0.1:{port}");
    warp::serve(metrics_route).run(([127, 0, 0, 1], port)).await;
}
