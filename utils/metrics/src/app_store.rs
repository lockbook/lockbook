use std::collections::HashMap;
use std::fs;
use std::io::Read as IoRead;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use chrono::{Duration, Local, NaiveDate};
use flate2::read::GzDecoder;
use jsonwebtoken::Algorithm;
use jsonwebtoken::EncodingKey;
use jsonwebtoken::Header;
use jsonwebtoken::encode;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::*;

use crate::metrics::{INSTALLS, Normalized, normalize_app_store};

pub struct AppStoreConfig {
    pub issuer_id: String,
    pub key_id: String,
    pub private_key: String,
    pub vendor_number: String,
}

#[derive(Serialize)]
struct Claims {
    iss: String,
    iat: u64,
    exp: u64,
    aud: String,
}

#[derive(Serialize, Deserialize, Default)]
struct DailyReport {
    date: String,
    units: Vec<DailyEntry>,
}

#[derive(Serialize, Deserialize, Clone)]
struct DailyEntry {
    product: String,
    product_type: String,
    country: String,
    units: i64,
}

pub struct AppStoreState {
    data_dir: PathBuf,
    backfill_complete: bool,
    cumulative: HashMap<(String, String, String), i64>, // (product, product_type, country) -> units
}

impl AppStoreState {
    pub fn new(data_dir: &Path) -> Self {
        let data_dir = data_dir.join("app_store");
        fs::create_dir_all(&data_dir).expect("failed to create app store data directory");

        Self { data_dir, backfill_complete: false, cumulative: HashMap::new() }
    }

    fn backfill_marker(&self) -> PathBuf {
        self.data_dir.join(".backfill_complete")
    }

    fn is_backfill_complete(&self) -> bool {
        self.backfill_marker().exists()
    }

    fn mark_backfill_complete(&self) {
        fs::write(self.backfill_marker(), "").expect("failed to write backfill marker");
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
        self.cumulative.clear();

        let entries = match fs::read_dir(&self.data_dir) {
            Ok(e) => e,
            Err(_) => return,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "json").unwrap_or(false) {
                if let Ok(contents) = fs::read_to_string(&path) {
                    if let Ok(report) = serde_json::from_str::<DailyReport>(&contents) {
                        for entry in report.units {
                            *self
                                .cumulative
                                .entry((entry.product, entry.product_type, entry.country))
                                .or_default() += entry.units;
                        }
                    }
                }
            }
        }

        info!("loaded {} product/country combinations from history", self.cumulative.len());
    }

    pub fn update_metrics(&self) {
        if !self.backfill_complete {
            return;
        }

        // Aggregate by (normalized, country), filtering out non-app products
        let mut by_normalized_country: HashMap<(Normalized, String), i64> = HashMap::new();
        for ((product, product_type, country), units) in &self.cumulative {
            if let Some(normalized) = normalize_app_store(product, product_type) {
                *by_normalized_country
                    .entry((normalized, country.clone()))
                    .or_default() += units;
            }
        }

        for ((normalized, country), units) in by_normalized_country {
            INSTALLS
                .with_label_values(&["app_store", normalized.client, normalized.os, &country])
                .set(units);
        }
    }

    pub fn earliest_date(&self) -> Option<NaiveDate> {
        let entries = fs::read_dir(&self.data_dir).ok()?;

        let mut earliest: Option<NaiveDate> = None;

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "json").unwrap_or(false) {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    if let Ok(date) = NaiveDate::parse_from_str(stem, "%Y-%m-%d") {
                        earliest = Some(match earliest {
                            Some(e) if date < e => date,
                            Some(e) => e,
                            None => date,
                        });
                    }
                }
            }
        }

        earliest
    }

    pub async fn backfill(&mut self, client: &Client, config: &AppStoreConfig) {
        if self.is_backfill_complete() {
            info!("app store backfill already complete, loading historical data");
            self.load_all_reports();
            self.backfill_complete = true;
            return;
        }

        info!("starting app store backfill");

        let Some(token) = generate_token(config) else {
            error!("failed to generate token for backfill");
            return;
        };

        let mut date = Local::now().date_naive() - Duration::days(1);
        let mut consecutive_failures = 0;

        loop {
            let date_str = date.format("%Y-%m-%d").to_string();

            if self.has_report(&date_str) {
                info!("already have report for {date_str}, skipping");
                date -= Duration::days(1);
                consecutive_failures = 0;
                continue;
            }

            info!("fetching report for {date_str}");

            match fetch_and_parse_report(client, &token, &config.vendor_number, &date_str).await {
                Some(units) => {
                    let entries: Vec<_> = units
                        .into_iter()
                        .map(|((product, product_type, country), units)| DailyEntry {
                            product,
                            product_type,
                            country,
                            units,
                        })
                        .collect();
                    let report = DailyReport { date: date_str, units: entries };
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

            date -= Duration::days(1);

            // Rate limiting
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        }

        self.mark_backfill_complete();
        self.load_all_reports();
        self.backfill_complete = true;
        info!("app store backfill complete");
    }

    pub async fn refresh(&mut self, client: &Client, config: &AppStoreConfig) {
        if !self.backfill_complete {
            return;
        }

        let yesterday = (Local::now() - Duration::days(1)).format("%Y-%m-%d").to_string();

        if self.has_report(&yesterday) {
            return;
        }

        info!("refreshing app store metrics for {yesterday}");

        let Some(token) = generate_token(config) else {
            error!("failed to generate App Store Connect JWT");
            return;
        };

        if let Some(units) = fetch_and_parse_report(client, &token, &config.vendor_number, &yesterday).await {
            let entries: Vec<_> = units
                .iter()
                .map(|((product, product_type, country), &units)| DailyEntry {
                    product: product.clone(),
                    product_type: product_type.clone(),
                    country: country.clone(),
                    units,
                })
                .collect();
            let report = DailyReport { date: yesterday, units: entries };
            self.save_report(&report);

            // Add to cumulative
            for ((product, product_type, country), count) in units {
                *self
                    .cumulative
                    .entry((product, product_type, country))
                    .or_default() += count;
            }
        }
    }

}

fn generate_token(config: &AppStoreConfig) -> Option<String> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs();

    let claims = Claims {
        iss: config.issuer_id.clone(),
        iat: now,
        exp: now + 1200,
        aud: "appstoreconnect-v1".to_string(),
    };

    let header =
        Header { alg: Algorithm::ES256, kid: Some(config.key_id.clone()), ..Default::default() };

    let key = match EncodingKey::from_ec_pem(config.private_key.as_bytes()) {
        Ok(k) => k,
        Err(e) => {
            error!("failed to parse App Store private key: {e}");
            return None;
        }
    };

    match encode(&header, &claims, &key) {
        Ok(token) => Some(token),
        Err(e) => {
            error!("failed to encode App Store JWT: {e}");
            None
        }
    }
}

async fn fetch_and_parse_report(
    client: &Client,
    token: &str,
    vendor_number: &str,
    date: &str,
) -> Option<HashMap<(String, String, String), i64>> {
    let resp = client
        .get("https://api.appstoreconnect.apple.com/v1/salesReports")
        .header("Authorization", format!("Bearer {token}"))
        .query(&[
            ("filter[reportType]", "SALES"),
            ("filter[reportSubType]", "SUMMARY"),
            ("filter[frequency]", "DAILY"),
            ("filter[reportDate]", date),
            ("filter[vendorNumber]", vendor_number),
        ])
        .send()
        .await
        .ok()?;

    if !resp.status().is_success() {
        warn!("app store API returned {} for {date}", resp.status());
        return None;
    }

    let bytes = resp.bytes().await.ok()?;

    let mut decoder = GzDecoder::new(&bytes[..]);
    let mut tsv = String::new();
    if decoder.read_to_string(&mut tsv).is_err() {
        warn!("failed to decompress app store report for {date}");
        return None;
    }

    Some(parse_report(&tsv))
}

fn parse_report(tsv: &str) -> HashMap<(String, String, String), i64> {
    let mut result = HashMap::new();

    let mut lines = tsv.lines();
    let Some(header) = lines.next() else {
        return result;
    };

    let columns: Vec<&str> = header.split('\t').collect();
    let col = |name| columns.iter().position(|c| *c == name);
    let (Some(title_idx), Some(units_idx), Some(country_idx), Some(type_idx)) = (
        col("Title"),
        col("Units"),
        col("Country Code"),
        col("Product Type Identifier"),
    ) else {
        warn!("unexpected report format: {header}");
        return result;
    };

    let max_idx = [title_idx, units_idx, country_idx, type_idx]
        .into_iter()
        .max()
        .unwrap();

    for line in lines {
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() <= max_idx {
            continue;
        }

        let title = fields[title_idx].to_string();
        let product_type = fields[type_idx].to_string();
        let country = fields[country_idx].to_string();
        let units: i64 = fields[units_idx].parse().unwrap_or(0);

        *result.entry((title, product_type, country)).or_default() += units;
    }

    result
}
