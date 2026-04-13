use std::io::Read;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use flate2::read::GzDecoder;
use jsonwebtoken::Algorithm;
use jsonwebtoken::EncodingKey;
use jsonwebtoken::Header;
use jsonwebtoken::encode;
use lazy_static::lazy_static;
use prometheus::IntGaugeVec;
use prometheus::register_int_gauge_vec;
use reqwest::Client;
use serde::Serialize;
use tracing::*;

lazy_static! {
    static ref UNITS: IntGaugeVec = register_int_gauge_vec!(
        "app_store_units",
        "App Store units by product and type",
        &["product", "type", "country"]
    )
    .unwrap();
}

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

pub async fn refresh(client: &Client, config: &AppStoreConfig) {
    info!("refreshing app store metrics");

    let Some(token) = generate_token(config) else {
        error!("failed to generate App Store Connect JWT");
        return;
    };

    let yesterday = (chrono::Local::now() - chrono::Duration::days(1))
        .format("%Y-%m-%d")
        .to_string();

    let resp = match fetch_report(client, &token, &config.vendor_number, &yesterday).await {
        Some(r) => r,
        None => return,
    };

    let bytes = match resp.bytes().await {
        Ok(b) => b,
        Err(e) => {
            warn!("failed to read app store response: {e}");
            return;
        }
    };

    let mut decoder = GzDecoder::new(&bytes[..]);
    let mut tsv = String::new();
    if decoder.read_to_string(&mut tsv).is_err() {
        warn!("failed to decompress app store report");
        return;
    }

    parse_report(&tsv);
}

async fn fetch_report(
    client: &Client, token: &str, vendor_number: &str, date: &str,
) -> Option<reqwest::Response> {
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

    Some(resp)
}

fn parse_report(tsv: &str) {
    let mut lines = tsv.lines();
    let Some(header) = lines.next() else {
        return;
    };

    let columns: Vec<&str> = header.split('\t').collect();
    let col = |name| columns.iter().position(|c| *c == name);
    let (Some(title_idx), Some(units_idx), Some(type_idx), Some(country_idx)) =
        (col("Title"), col("Units"), col("Product Type Identifier"), col("Country Code"))
    else {
        warn!("unexpected report format: {header}");
        return;
    };

    let max_idx = [title_idx, units_idx, type_idx, country_idx]
        .into_iter()
        .max()
        .unwrap();

    for line in lines {
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() <= max_idx {
            continue;
        }

        let title = fields[title_idx];
        let product_type = fields[type_idx];
        let country = fields[country_idx];
        let units: i64 = fields[units_idx].parse().unwrap_or(0);

        UNITS
            .with_label_values(&[title, product_type, country])
            .set(units);
    }
}
