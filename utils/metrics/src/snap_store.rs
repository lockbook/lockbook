use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::NaiveDate;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::*;

use crate::metrics::INSTALLS;

pub struct SnapStoreConfig {
    pub macaroon: String,
}

#[derive(Serialize)]
struct MetricsRequest {
    filters: Vec<MetricsFilter>,
}

#[derive(Serialize)]
struct MetricsFilter {
    snap_id: String,
    metric_name: String,
    start: String,
    end: String,
}

#[derive(Deserialize)]
struct MetricsResponse {
    metrics: Vec<MetricData>,
}

#[derive(Deserialize)]
struct MetricData {
    status: String,
    series: Vec<Series>,
}

#[derive(Deserialize)]
struct Series {
    name: String,
    values: Vec<Option<i64>>,
}

#[derive(Serialize, Deserialize, Default)]
struct DailyReport {
    date: String,
    snaps: Vec<SnapEntry>,
}

#[derive(Serialize, Deserialize, Clone)]
struct SnapEntry {
    snap_name: String,
    new: i64,
    continued: i64,
    lost: i64,
}

pub struct SnapStoreState {
    data_dir: PathBuf,
    cumulative_new: HashMap<String, i64>, // snap_name -> total new installs
    earliest_date: Option<NaiveDate>,
}

impl SnapStoreState {
    pub fn new(data_dir: &Path) -> Self {
        let data_dir = data_dir.join("snap_store");
        fs::create_dir_all(&data_dir).expect("failed to create snap store data directory");

        Self {
            data_dir,
            cumulative_new: HashMap::new(),
            earliest_date: None,
        }
    }

    pub fn set_earliest_date(&mut self, date: NaiveDate) {
        self.earliest_date = Some(date);
    }

    fn report_path(&self, date: &str) -> PathBuf {
        self.data_dir.join(format!("{date}.json"))
    }

    fn has_report(&self, date: &str) -> bool {
        self.report_path(date).exists()
    }

    fn save_report(&self, report: &DailyReport) {
        let path = self.report_path(&report.date);
        let json = serde_json::to_string(report).expect("failed to serialize report");
        fs::write(path, json).expect("failed to write report");
    }

    fn load_all_reports(&mut self) {
        self.cumulative_new.clear();

        let entries = match fs::read_dir(&self.data_dir) {
            Ok(e) => e,
            Err(_) => return,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "json").unwrap_or(false) {
                if let Ok(contents) = fs::read_to_string(&path) {
                    if let Ok(report) = serde_json::from_str::<DailyReport>(&contents) {
                        for snap in report.snaps {
                            *self.cumulative_new.entry(snap.snap_name).or_default() += snap.new;
                        }
                    }
                }
            }
        }

        info!("loaded {} snaps from snap store history", self.cumulative_new.len());
    }

    pub fn update_metrics(&self) {
        for (snap_name, count) in &self.cumulative_new {
            let client_name = if snap_name == "lockbook" { "cli" } else { "linux" };
            INSTALLS
                .with_label_values(&["snap_store", client_name, "linux", ""])
                .set(*count);
        }
    }

    pub async fn refresh(&mut self, client: &Client, config: &SnapStoreConfig) {
        if self.earliest_date.is_none() {
            warn!("snap store refresh skipped: no earliest date set");
            return;
        }

        let yesterday = (chrono::Local::now() - chrono::Duration::days(1))
            .format("%Y-%m-%d")
            .to_string();

        if self.has_report(&yesterday) {
            return;
        }

        info!("refreshing snap store metrics for {yesterday}");

        if let Some(report) = fetch_daily_report(client, config, &yesterday).await {
            for snap in &report.snaps {
                *self.cumulative_new.entry(snap.snap_name.clone()).or_default() += snap.new;
            }
            self.save_report(&report);
        }
    }

    pub async fn backfill(&mut self, client: &Client, config: &SnapStoreConfig) {
        if self.earliest_date.is_none() {
            warn!("snap store backfill skipped: no earliest date set");
            return;
        }

        info!("starting snap store backfill");

        let earliest = self.earliest_date.unwrap();
        let mut date = chrono::Local::now().date_naive() - chrono::Duration::days(1);

        while date >= earliest {
            let date_str = date.format("%Y-%m-%d").to_string();

            if self.has_report(&date_str) {
                date -= chrono::Duration::days(1);
                continue;
            }

            info!("fetching snap store data for {date_str}");

            if let Some(report) = fetch_daily_report(client, config, &date_str).await {
                self.save_report(&report);
            }

            date -= chrono::Duration::days(1);

            // Rate limiting
            tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
        }

        self.load_all_reports();
        info!("snap store backfill complete");
    }
}

async fn get_snap_id(client: &Client, snap_name: &str) -> Option<String> {
    let url = format!("https://api.snapcraft.io/v2/snaps/info/{}", snap_name);

    let resp = client
        .get(&url)
        .header("Snap-Device-Series", "16")
        .send()
        .await
        .ok()?;

    if !resp.status().is_success() {
        warn!("failed to get snap info for {}: {}", snap_name, resp.status());
        return None;
    }

    let info: serde_json::Value = resp.json().await.ok()?;
    info["snap-id"].as_str().map(|s| s.to_string())
}

async fn fetch_daily_report(
    client: &Client,
    config: &SnapStoreConfig,
    date: &str,
) -> Option<DailyReport> {
    let snaps = ["lockbook", "lockbook-desktop"];
    let mut entries = Vec::new();

    for snap_name in snaps {
        let Some(snap_id) = get_snap_id(client, snap_name).await else {
            warn!("could not find snap_id for {snap_name}");
            continue;
        };

        let request = MetricsRequest {
            filters: vec![MetricsFilter {
                snap_id,
                metric_name: "daily_device_change".to_string(),
                start: date.to_string(),
                end: date.to_string(),
            }],
        };

        let resp = match client
            .post("https://dashboard.snapcraft.io/dev/api/snaps/metrics")
            .header("Authorization", format!("Macaroon {}", config.macaroon))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                warn!("failed to fetch snap metrics for {snap_name}: {e}");
                continue;
            }
        };

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            warn!("snap metrics API returned {status} for {snap_name}: {body}");
            continue;
        }

        let metrics: MetricsResponse = match resp.json().await {
            Ok(m) => m,
            Err(e) => {
                warn!("failed to parse snap metrics response for {snap_name}: {e}");
                continue;
            }
        };

        for metric in metrics.metrics {
            if metric.status != "OK" {
                continue;
            }

            let mut new = 0i64;
            let mut continued = 0i64;
            let mut lost = 0i64;

            for series in metric.series {
                let sum: i64 = series.values.iter().filter_map(|v| *v).sum();
                match series.name.as_str() {
                    "new" => new = sum,
                    "continued" => continued = sum,
                    "lost" => lost = sum,
                    _ => {}
                }
            }

            entries.push(SnapEntry {
                snap_name: snap_name.to_string(),
                new,
                continued,
                lost,
            });
        }
    }

    if entries.is_empty() {
        return None;
    }

    Some(DailyReport { date: date.to_string(), snaps: entries })
}
