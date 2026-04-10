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

async fn refresh_all(client: &Client, config: &env::Config) {
    info!("refreshing metrics");
    github::refresh(client, &config.github_token).await;
    flathub::refresh(client).await;
    app_store::refresh(client, &config.app_store).await;
    info!("metrics refresh complete");
}

#[tokio::main]
async fn main() {
    loggers::init();
    info!("lb-metrics started");
    let config = env::Config::from_env();
    let port = config.port;

    tokio::spawn(async move {
        let client = Client::new();
        loop {
            refresh_all(&client, &config).await;
            sleep(Duration::from_secs(300)).await;
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

    info!("lb-metrics listening on :{port}");
    warp::serve(metrics_route).run(([0, 0, 0, 0], port)).await;
}

mod app_store;
mod env;
mod flathub;
mod github;
mod loggers;

use std::time::Duration;

use prometheus::TextEncoder;
use prometheus::gather;
use reqwest::Client;
use serde::de::DeserializeOwned;
use tokio::time::sleep;
use tracing::*;
use warp::Filter;
use warp::path;
use warp::reply::with_header;
