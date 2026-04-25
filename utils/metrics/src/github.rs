use std::collections::HashMap;

use chrono::NaiveDate;
use lazy_static::lazy_static;
use prometheus::{IntGaugeVec, register_int_gauge_vec};
use reqwest::Client;
use serde::Deserialize;
use tracing::*;

use crate::get;
use crate::metrics::{INSTALLS, Normalized, normalize_github_asset};

lazy_static! {
    static ref VIEWS: IntGaugeVec =
        register_int_gauge_vec!("github_views", "Repo views (14 day)", &["kind"]).unwrap();
    static ref CLONES: IntGaugeVec =
        register_int_gauge_vec!("github_clones", "Repo clones (14 day)", &["kind"]).unwrap();
    static ref REFERRERS: IntGaugeVec =
        register_int_gauge_vec!("github_referrers", "Traffic by referrer (14 day)", &["referrer", "kind"]).unwrap();
    static ref REPO_STATS: IntGaugeVec =
        register_int_gauge_vec!("github_repo", "Repo statistics", &["kind"]).unwrap();
}

#[derive(Deserialize)]
struct Release {
    published_at: String,
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    download_count: i64,
}

#[derive(Deserialize)]
struct Traffic {
    count: i64,
    uniques: i64,
}

#[derive(Deserialize)]
struct Referrer {
    referrer: String,
    count: i64,
    uniques: i64,
}

#[derive(Deserialize)]
struct Repo {
    stargazers_count: i64,
    forks_count: i64,
}

fn parse_github_date(date_str: &str) -> Option<NaiveDate> {
    // GitHub dates are ISO 8601: "2024-01-15T10:30:00Z"
    chrono::DateTime::parse_from_rfc3339(date_str)
        .ok()
        .map(|dt| dt.date_naive())
}

pub async fn refresh(client: &Client, token: &str, earliest_date: Option<NaiveDate>) {
    let repo = "lockbook/lockbook";
    let auth = format!("Bearer {token}");
    let api = "https://api.github.com";

    info!("refreshing github metrics");

    let mut downloads: HashMap<Normalized, i64> = HashMap::new();
    let mut page = 1;
    loop {
        let url = format!("{api}/repos/{repo}/releases?per_page=100&page={page}");
        match get::<Vec<Release>>(client, &url, &auth).await {
            Some(releases) if !releases.is_empty() => {
                for release in &releases {
                    if let Some(earliest) = earliest_date {
                        if let Some(published) = parse_github_date(&release.published_at) {
                            if published < earliest {
                                continue;
                            }
                        }
                    }
                    for asset in &release.assets {
                        if let Some(normalized) = normalize_github_asset(&asset.name) {
                            *downloads.entry(normalized).or_default() += asset.download_count;
                        }
                    }
                }
                page += 1;
            }
            _ => break,
        }
    }

    for (normalized, count) in downloads {
        INSTALLS
            .with_label_values(&["github", normalized.client, normalized.os, ""])
            .set(count);
    }

    if let Some(views) =
        get::<Traffic>(client, &format!("{api}/repos/{repo}/traffic/views"), &auth).await
    {
        VIEWS.with_label_values(&["total"]).set(views.count);
        VIEWS.with_label_values(&["unique"]).set(views.uniques);
    }

    if let Some(clones) =
        get::<Traffic>(client, &format!("{api}/repos/{repo}/traffic/clones"), &auth).await
    {
        CLONES.with_label_values(&["total"]).set(clones.count);
        CLONES.with_label_values(&["unique"]).set(clones.uniques);
    }

    if let Some(referrers) = get::<Vec<Referrer>>(
        client,
        &format!("{api}/repos/{repo}/traffic/popular/referrers"),
        &auth,
    )
    .await
    {
        for r in &referrers {
            REFERRERS.with_label_values(&[&r.referrer, "views"]).set(r.count);
            REFERRERS.with_label_values(&[&r.referrer, "uniques"]).set(r.uniques);
        }
    }

    if let Some(repo_info) = get::<Repo>(client, &format!("{api}/repos/{repo}"), &auth).await {
        REPO_STATS
            .with_label_values(&["stargazers"])
            .set(repo_info.stargazers_count);
        REPO_STATS
            .with_label_values(&["forks"])
            .set(repo_info.forks_count);
    }
}
