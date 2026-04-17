mod app_store;
mod crates_io;
mod env;
mod flathub;
mod github;
mod loggers;
mod metrics;
mod play_store;
mod snap_store;

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
use play_store::PlayStoreState;
use snap_store::SnapStoreState;

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

    // Initialize Play Store and Snap Store state with date range from App Store
    let mut play_store_state = PlayStoreState::new(&config.data_dir);
    let mut snap_store_state =
        SnapStoreState::new(&config.data_dir, config.snap_store.macaroon.clone());

    // Refresh the snap store discharge macaroon up-front so startup surfaces
    // any auth issues immediately, and so backfill runs with a fresh token.
    snap_store_state.refresh_macaroon(&client).await;

    if let Some(earliest) = app_store_state.earliest_date() {
        info!("using App Store earliest date: {earliest}");
        play_store_state.set_earliest_date(earliest);
        play_store_state.backfill(&client, &config.play_store).await;
        snap_store_state.set_earliest_date(earliest);
        snap_store_state.backfill(&client).await;
    } else {
        warn!("no App Store data found, Play Store and Snap Store metrics will be skipped");
    }

    // Initial metrics refresh
    let earliest_date = app_store_state.earliest_date();
    info!("performing initial metrics refresh");
    github::refresh(&client, &config.github_token, earliest_date).await;
    flathub::refresh(&client).await;
    crates_io::refresh(&client).await;
    app_store_state.update_metrics();
    play_store_state.update_metrics();
    snap_store_state.update_metrics();

    info!("backfill complete, starting metrics server");

    let app_store_state = Arc::new(Mutex::new(app_store_state));
    let play_store_state = Arc::new(Mutex::new(play_store_state));
    let snap_store_state = Arc::new(Mutex::new(snap_store_state));

    // Spawn refresh loop
    let refresh_app_store = app_store_state.clone();
    let refresh_play_store = play_store_state.clone();
    let refresh_snap_store = snap_store_state.clone();
    tokio::spawn(async move {
        loop {
            sleep(Duration::from_secs(300)).await;

            info!("refreshing metrics");
            github::refresh(&client, &config.github_token, earliest_date).await;
            flathub::refresh(&client).await;
            crates_io::refresh(&client).await;

            let mut app_state = refresh_app_store.lock().await;
            app_state.refresh(&client, &config.app_store).await;
            app_state.update_metrics();

            let mut play_state = refresh_play_store.lock().await;
            play_state.refresh(&client, &config.play_store).await;
            play_state.update_metrics();

            let mut snap_state = refresh_snap_store.lock().await;
            snap_state.refresh(&client).await;
            snap_state.update_metrics();

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
