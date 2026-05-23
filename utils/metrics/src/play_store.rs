use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::NaiveDate;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::*;

use crate::metrics::INSTALLS;

pub struct PlayStoreConfig {
    pub service_account_key: String,
    pub bucket: String,
}

#[derive(Serialize, Deserialize, Default)]
struct MonthlyReport {
    month: String, // YYYYMM
    installs: Vec<InstallEntry>,
}

#[derive(Serialize, Deserialize, Clone)]
struct InstallEntry {
    country: String,
    count: i64,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
}

#[derive(Deserialize)]
struct ListObjectsResponse {
    items: Option<Vec<GcsObject>>,
}

#[derive(Deserialize)]
struct GcsObject {
    name: String,
}

pub struct PlayStoreState {
    data_dir: PathBuf,
    cumulative: HashMap<String, i64>, // country -> installs
    earliest_date: Option<NaiveDate>,
}

impl PlayStoreState {
    pub fn new(data_dir: &Path) -> Self {
        let data_dir = data_dir.join("play_store");
        fs::create_dir_all(&data_dir).expect("failed to create play store data directory");

        Self { data_dir, cumulative: HashMap::new(), earliest_date: None }
    }

    pub fn set_earliest_date(&mut self, date: NaiveDate) {
        self.earliest_date = Some(date);
    }

    fn report_path(&self, month: &str) -> PathBuf {
        self.data_dir.join(format!("{month}.json"))
    }

    fn has_report(&self, month: &str) -> bool {
        self.report_path(month).exists()
    }

    fn save_report(&self, report: &MonthlyReport) {
        let path = self.report_path(&report.month);
        let json = serde_json::to_string(report).expect("failed to serialize report");
        fs::write(path, json).expect("failed to write report");
    }

    fn load_all_reports(&mut self) {
        self.cumulative.clear();

        let entries = match fs::read_dir(&self.data_dir) {
            Ok(e) => e,
            Err(_) => return,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "json").unwrap_or(false) {
                if let Ok(contents) = fs::read_to_string(&path) {
                    if let Ok(report) = serde_json::from_str::<MonthlyReport>(&contents) {
                        // Check if this month is within our date range
                        if !self.month_in_range(&report.month) {
                            continue;
                        }
                        for entry in report.installs {
                            *self.cumulative.entry(entry.country).or_default() += entry.count;
                        }
                    }
                }
            }
        }

        info!("loaded {} countries from Play Store history", self.cumulative.len());
    }

    fn month_in_range(&self, month: &str) -> bool {
        let Some(earliest) = self.earliest_date else {
            return true; // No constraint
        };

        // month is YYYYMM, convert to first day of that month
        if month.len() != 6 {
            return false;
        }

        let year: i32 = match month[0..4].parse() {
            Ok(y) => y,
            Err(_) => return false,
        };
        let month_num: u32 = match month[4..6].parse() {
            Ok(m) => m,
            Err(_) => return false,
        };

        let Some(month_start) = NaiveDate::from_ymd_opt(year, month_num, 1) else {
            return false;
        };

        // Include this month if it ends on or after our earliest date
        // (i.e., the month contains at least one day we care about)
        let month_end = if month_num == 12 {
            NaiveDate::from_ymd_opt(year + 1, 1, 1)
        } else {
            NaiveDate::from_ymd_opt(year, month_num + 1, 1)
        };

        match month_end {
            Some(end) => end > earliest,
            None => month_start >= earliest,
        }
    }

    pub fn update_metrics(&self) {
        for (country, count) in &self.cumulative {
            INSTALLS
                .with_label_values(&["play_store", "android", "android", country])
                .set(*count);
        }
    }

    pub async fn refresh(&mut self, client: &Client, config: &PlayStoreConfig) {
        if self.earliest_date.is_none() {
            warn!("play store refresh skipped: no earliest date set");
            return;
        }

        info!("refreshing play store metrics");

        let Some(token) = get_access_token(client, &config.service_account_key).await else {
            error!("failed to get GCS access token");
            return;
        };

        let files = list_csv_files(client, &token, &config.bucket).await;

        if files.is_empty() {
            warn!("no CSV files found in Play Store bucket");
            return;
        }

        let mut new_data = false;

        for file in &files {
            let Some(month) = extract_month_from_filename(file) else {
                continue;
            };

            if !self.month_in_range(&month) {
                continue;
            }

            if self.has_report(&month) {
                continue;
            }

            info!("fetching Play Store data for {month}");

            if let Some(csv) = fetch_csv(client, &token, &config.bucket, file).await {
                let installs = parse_installs_csv(&csv);
                let entries: Vec<_> = installs
                    .into_iter()
                    .map(|(country, count)| InstallEntry { country, count })
                    .collect();

                let report = MonthlyReport { month: month.clone(), installs: entries.clone() };
                self.save_report(&report);

                for entry in entries {
                    *self.cumulative.entry(entry.country).or_default() += entry.count;
                }

                new_data = true;
            }
        }

        if new_data {
            info!("play store metrics updated");
        }
    }

    pub async fn backfill(&mut self, client: &Client, config: &PlayStoreConfig) {
        if self.earliest_date.is_none() {
            warn!("play store backfill skipped: no earliest date set");
            return;
        }

        info!("starting play store backfill");

        let Some(token) = get_access_token(client, &config.service_account_key).await else {
            error!("failed to get GCS access token for backfill");
            return;
        };

        let files = list_csv_files(client, &token, &config.bucket).await;
        info!("found {} CSV files in Play Store bucket", files.len());

        for file in &files {
            let Some(month) = extract_month_from_filename(file) else {
                continue;
            };

            if !self.month_in_range(&month) {
                info!("skipping {month}: outside date range");
                continue;
            }

            if self.has_report(&month) {
                info!("already have {month}, skipping");
                continue;
            }

            info!("fetching Play Store data for {month}");

            if let Some(csv) = fetch_csv(client, &token, &config.bucket, file).await {
                let installs = parse_installs_csv(&csv);
                let entries: Vec<_> = installs
                    .into_iter()
                    .map(|(country, count)| InstallEntry { country, count })
                    .collect();

                let report = MonthlyReport { month: month.clone(), installs: entries };
                self.save_report(&report);
            }

            // Rate limiting
            tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
        }

        self.load_all_reports();
        info!("play store backfill complete");
    }
}

fn extract_month_from_filename(filename: &str) -> Option<String> {
    // Format: stats/installs/installs_app.lockbook_YYYYMM_country.csv
    let parts: Vec<&str> = filename.split('_').collect();
    for part in parts {
        if part.len() == 6 && part.chars().all(|c| c.is_ascii_digit()) {
            return Some(part.to_string());
        }
    }
    None
}

async fn get_access_token(client: &Client, service_account_key: &str) -> Option<String> {
    let key: serde_json::Value = serde_json::from_str(service_account_key).ok()?;

    let client_email = key["client_email"].as_str()?;
    let private_key = key["private_key"].as_str()?;
    let token_uri = key["token_uri"]
        .as_str()
        .unwrap_or("https://oauth2.googleapis.com/token");

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();

    let claims = serde_json::json!({
        "iss": client_email,
        "scope": "https://www.googleapis.com/auth/devstorage.read_only",
        "aud": token_uri,
        "iat": now,
        "exp": now + 3600,
    });

    let header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256);
    let key = jsonwebtoken::EncodingKey::from_rsa_pem(private_key.as_bytes()).ok()?;
    let jwt = jsonwebtoken::encode(&header, &claims, &key).ok()?;

    let resp = client
        .post(token_uri)
        .form(&[("grant_type", "urn:ietf:params:oauth:grant-type:jwt-bearer"), ("assertion", &jwt)])
        .send()
        .await
        .ok()?;

    if !resp.status().is_success() {
        warn!("failed to get access token: {}", resp.status());
        return None;
    }

    let token: TokenResponse = resp.json().await.ok()?;
    Some(token.access_token)
}

async fn list_csv_files(client: &Client, token: &str, bucket: &str) -> Vec<String> {
    let url =
        format!("https://storage.googleapis.com/storage/v1/b/{}/o?prefix=stats/installs/", bucket);

    let resp = match client.get(&url).bearer_auth(token).send().await {
        Ok(r) => r,
        Err(e) => {
            warn!("failed to list GCS objects: {e}");
            return vec![];
        }
    };

    if !resp.status().is_success() {
        warn!("GCS list returned {}", resp.status());
        return vec![];
    }

    let list: ListObjectsResponse = match resp.json().await {
        Ok(l) => l,
        Err(e) => {
            warn!("failed to parse GCS list response: {e}");
            return vec![];
        }
    };

    list.items
        .unwrap_or_default()
        .into_iter()
        .map(|o| o.name)
        .filter(|n| n.ends_with("_country.csv"))
        .collect()
}

async fn fetch_csv(client: &Client, token: &str, bucket: &str, object: &str) -> Option<String> {
    let encoded_object = urlencoding::encode(object);
    let url = format!(
        "https://storage.googleapis.com/storage/v1/b/{}/o/{}?alt=media",
        bucket, encoded_object
    );

    let resp = client.get(&url).bearer_auth(token).send().await.ok()?;

    if !resp.status().is_success() {
        warn!("failed to fetch {object}: {}", resp.status());
        return None;
    }

    resp.text().await.ok()
}

fn parse_installs_csv(csv: &str) -> HashMap<String, i64> {
    let mut result = HashMap::new();
    let mut lines = csv.lines();

    // Skip header
    let Some(header) = lines.next() else {
        return result;
    };

    let columns: Vec<&str> = header.split(',').collect();
    let country_idx = columns.iter().position(|c| c.contains("Country"));
    let installs_idx = columns
        .iter()
        .position(|c| c.contains("Install") && c.contains("Device"));

    let (Some(country_idx), Some(installs_idx)) = (country_idx, installs_idx) else {
        warn!("unexpected CSV format: {header}");
        return result;
    };

    for line in lines {
        let fields: Vec<&str> = line.split(',').collect();
        if fields.len() <= country_idx.max(installs_idx) {
            continue;
        }

        let country = fields[country_idx].trim().to_string();
        let installs: i64 = fields[installs_idx].trim().parse().unwrap_or(0);

        *result.entry(country).or_default() += installs;
    }

    result
}
