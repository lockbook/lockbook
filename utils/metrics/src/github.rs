use std::collections::HashMap;

use lazy_static::lazy_static;
use prometheus::{IntGaugeVec, register_int_gauge_vec};
use reqwest::Client;
use serde::Deserialize;
use tracing::*;

use crate::get;
use crate::metrics::INSTALLS;

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

pub async fn refresh(client: &Client, token: &str) {
    let repo = "lockbook/lockbook";
    let auth = format!("Bearer {token}");
    let api = "https://api.github.com";

    info!("refreshing github metrics");

    let mut downloads_by_asset: HashMap<String, i64> = HashMap::new();
    let mut page = 1;
    loop {
        let url = format!("{api}/repos/{repo}/releases?per_page=100&page={page}");
        match get::<Vec<Release>>(client, &url, &auth).await {
            Some(releases) if !releases.is_empty() => {
                for release in &releases {
                    for asset in &release.assets {
                        *downloads_by_asset.entry(asset.name.clone()).or_default() +=
                            asset.download_count;
                    }
                }
                page += 1;
            }
            _ => break,
        }
    }

    for (asset, count) in downloads_by_asset {
        INSTALLS.with_label_values(&["github", &asset, ""]).set(count);
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
