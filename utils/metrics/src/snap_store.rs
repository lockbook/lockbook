use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration as StdDuration, Instant};

use base64::Engine;
use chrono::NaiveDate;
use macaroon::{Format, Macaroon};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::*;

use crate::metrics::INSTALLS;

const MACAROON_REFRESH_INTERVAL: StdDuration = StdDuration::from_secs(3600);

pub struct SnapStoreConfig {
    pub macaroon: String,
}

#[derive(Serialize, Deserialize)]
struct SnapcraftCredential {
    v: CredentialValue,
}

#[derive(Serialize, Deserialize)]
struct CredentialValue {
    r: String,
    d: String,
}

#[derive(Serialize)]
struct RefreshRequest<'a> {
    discharge_macaroon: &'a str,
}

#[derive(Deserialize)]
struct RefreshResponse {
    discharge_macaroon: String,
}

fn parse_authorization_header(macaroon: &str) -> Option<String> {
    let decoded = match base64::engine::general_purpose::STANDARD.decode(macaroon.trim()) {
        Ok(d) => d,
        Err(e) => {
            error!("failed to base64 decode macaroon: {e}");
            return None;
        }
    };
    let cred: SnapcraftCredential = match serde_json::from_slice(&decoded) {
        Ok(c) => c,
        Err(e) => {
            error!("failed to parse macaroon JSON: {e}");
            return None;
        }
    };

    // Parse and bind the discharge macaroon to the root
    let root = match Macaroon::deserialize(&cred.v.r) {
        Ok(m) => m,
        Err(e) => {
            error!("failed to deserialize root macaroon: {e}");
            return None;
        }
    };
    let mut discharge = match Macaroon::deserialize(&cred.v.d) {
        Ok(m) => m,
        Err(e) => {
            error!("failed to deserialize discharge macaroon: {e}");
            return None;
        }
    };

    // Bind discharge to root
    root.bind(&mut discharge);

    // Serialize the bound discharge
    let bound_discharge = match discharge.serialize(Format::V2) {
        Ok(s) => s,
        Err(e) => {
            error!("failed to serialize bound discharge: {e}");
            return None;
        }
    };

    Some(format!("Macaroon root=\"{}\",discharge=\"{}\"", cred.v.r, bound_discharge))
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
    credential: String,
    last_macaroon_refresh: Option<Instant>,
}

impl SnapStoreState {
    pub fn new(data_dir: &Path, macaroon: String) -> Self {
        let data_dir = data_dir.join("snap_store");
        fs::create_dir_all(&data_dir).expect("failed to create snap store data directory");

        Self {
            data_dir,
            cumulative_new: HashMap::new(),
            earliest_date: None,
            credential: macaroon,
            last_macaroon_refresh: None,
        }
    }

    pub async fn refresh_macaroon(&mut self, client: &Client) {
        info!("refreshing snap store discharge macaroon");

        let decoded = match base64::engine::general_purpose::STANDARD.decode(self.credential.trim())
        {
            Ok(d) => d,
            Err(e) => {
                error!("macaroon refresh: failed to base64 decode credential: {e}");
                return;
            }
        };

        let mut cred: SnapcraftCredential = match serde_json::from_slice(&decoded) {
            Ok(c) => c,
            Err(e) => {
                error!("macaroon refresh: failed to parse credential JSON: {e}");
                return;
            }
        };

        let req = RefreshRequest { discharge_macaroon: &cred.v.d };
        let resp = match client
            .post("https://login.ubuntu.com/api/v2/tokens/refresh")
            .header("Accept", "application/json")
            .header("Content-Type", "application/json")
            .header("User-Agent", "lb-metrics")
            .json(&req)
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                error!("macaroon refresh: POST to login.ubuntu.com failed: {e}");
                return;
            }
        };

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            error!("macaroon refresh API returned {status}: {body}");
            return;
        }

        let refresh_resp: RefreshResponse = match resp.json().await {
            Ok(r) => r,
            Err(e) => {
                error!("macaroon refresh: failed to parse response: {e}");
                return;
            }
        };

        cred.v.d = refresh_resp.discharge_macaroon;

        let new_json = match serde_json::to_vec(&cred) {
            Ok(j) => j,
            Err(e) => {
                error!("macaroon refresh: failed to serialize refreshed credential: {e}");
                return;
            }
        };

        self.credential = base64::engine::general_purpose::STANDARD.encode(&new_json);
        self.last_macaroon_refresh = Some(Instant::now());

        info!("snap store discharge macaroon refreshed");
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

    pub async fn refresh(&mut self, client: &Client) {
        // Refresh the discharge macaroon every hour to avoid 401s.
        let macaroon_stale = self
            .last_macaroon_refresh
            .map(|t| t.elapsed() >= MACAROON_REFRESH_INTERVAL)
            .unwrap_or(true);
        if macaroon_stale {
            self.refresh_macaroon(client).await;
        }

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

        if let Some(report) = fetch_daily_report(client, &self.credential, &yesterday).await {
            for snap in &report.snaps {
                *self.cumulative_new.entry(snap.snap_name.clone()).or_default() += snap.new;
            }
            self.save_report(&report);
        }
    }

    pub async fn backfill(&mut self, client: &Client) {
        if self.earliest_date.is_none() {
            warn!("snap store backfill skipped: no earliest date set");
            return;
        }

        info!("starting snap store backfill");

        let earliest = self.earliest_date.unwrap();
        let mut date = chrono::Local::now().date_naive() - chrono::Duration::days(1);
        let mut consecutive_failures = 0;

        loop {
            if date < earliest {
                break;
            }

            let date_str = date.format("%Y-%m-%d").to_string();

            if self.has_report(&date_str) {
                date -= chrono::Duration::days(1);
                consecutive_failures = 0;
                continue;
            }

            info!("fetching snap store data for {date_str}");

            match fetch_daily_report(client, &self.credential, &date_str).await {
                Some(report) => {
                    self.save_report(&report);
                    consecutive_failures = 0;
                }
                None => {
                    consecutive_failures += 1;
                    if consecutive_failures >= 3 {
                        info!("stopping backfill after 3 consecutive failures at {date_str}");
                        break;
                    }
                }
            }

            date -= chrono::Duration::days(1);

            // Rate limiting
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        }

        self.load_all_reports();
        info!("snap store backfill complete");
    }
}

async fn get_snap_id(client: &Client, snap_name: &str) -> Option<String> {
    let url = format!("https://api.snapcraft.io/v2/snaps/info/{}", snap_name);

    let resp = client
        .get(&url)
        .header("User-Agent", "lb-metrics")
        .header("Snap-Device-Series", "16")
        .header("Accept", "application/json")
        .send()
        .await
        .ok()?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        warn!("failed to get snap info for {snap_name}: {status} - {body}");
        return None;
    }

    let info: serde_json::Value = resp.json().await.ok()?;
    info["snap-id"].as_str().map(|s| s.to_string())
}

async fn fetch_daily_report(
    client: &Client,
    credential: &str,
    date: &str,
) -> Option<DailyReport> {
    let auth_header = match parse_authorization_header(credential) {
        Some(h) => h,
        None => {
            error!("failed to parse snapcraft credentials");
            return None;
        }
    };

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
            .header("Authorization", &auth_header)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .header("User-Agent", "lb-metrics")
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
