use crate::{ServerError, ServerState};
use lazy_static::lazy_static;

use lockbook_shared::api::FileUsage;
use lockbook_shared::clock::get_time;
use lockbook_shared::file_metadata::Owner;
use prometheus::{register_int_gauge_vec, IntGaugeVec};
use prometheus_static_metric::make_static_metric;
use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use tracing::*;

use uuid::Uuid;

make_static_metric! {
    pub struct MetricsStatistics: IntGauge {
        "type" => {
            total_users,
            active_users,
            total_documents,
            total_document_bytes,
        },
    }
}

lazy_static! {
    pub static ref METRICS_COUNTERS_VEC: IntGaugeVec = register_int_gauge_vec!(
        "lockbook_metrics_counters",
        "Lockbook's basic metrics of users and files derived from redis.",
        &["type"]
    )
    .unwrap();
    pub static ref METRICS_STATISTICS: MetricsStatistics =
        MetricsStatistics::from(&METRICS_COUNTERS_VEC);
    pub static ref METRICS_USAGE_BY_USER_VEC: IntGaugeVec = register_int_gauge_vec!(
        "lockbook_metrics_usage_by_user",
        "Lockbook's total usage by user.",
        &["username"]
    )
    .unwrap();
}

#[derive(Debug)]
pub enum MetricsError {}

pub const TWO_DAYS_IN_MILLIS: u128 = 1000 * 60 * 60 * 24 * 2;

pub fn start_metrics_worker(server_state: &ServerState) {
    let state_clone = server_state.clone();

    tokio::spawn(async move {
        info!("Started capturing metrics.");

        if let Err(e) = start(state_clone).await {
            error!("interrupting metrics loop due to error: {:?}", e)
        }
    });
}

pub async fn start(state: ServerState) -> Result<(), ServerError<MetricsError>> {
    loop {
        info!("Metrics refresh started.");

        let public_keys_and_usernames = state.index_db.usernames.get_all()?;
        METRICS_STATISTICS
            .total_users
            .set(public_keys_and_usernames.len() as i64);

        let mut total_documents = 0;
        let mut total_bytes = 0;
        let mut active_users = 0;

        for (username, public_key) in public_keys_and_usernames {
            let ids = state
                .index_db
                .owned_files
                .get(&public_key)?
                .ok_or_else(|| {
                    internal!(
                        "Could not get owned files for public key during metrics {:?}",
                        public_key
                    )
                })?;
            let (metadatas, is_user_active) =
                get_metadatas_and_user_activity_state(&state, &public_key, &ids).await?;

            if is_user_active {
                active_users += 1;
            }

            let bytes = calculate_total_document_bytes(&state, &metadatas).await?;

            total_bytes += bytes;
            total_documents += metadatas.len();

            METRICS_USAGE_BY_USER_VEC
                .with_label_values(&[&username])
                .set(bytes);

            tokio::time::sleep(state.config.metrics.time_between_redis_calls).await;
        }

        METRICS_STATISTICS
            .total_documents
            .set(total_documents as i64);
        METRICS_STATISTICS.active_users.set(active_users);
        METRICS_STATISTICS.total_document_bytes.set(total_bytes);

        tokio::time::sleep(state.config.metrics.time_between_metrics_refresh).await;
    }
}

pub async fn get_metadatas_and_user_activity_state(
    state: &ServerState, public_key: &Owner, ids: &HashSet<Uuid>,
) -> Result<(EncryptedFiles, bool), ServerError<MetricsError>> {
    let mut metadatas = HashMap::new();

    for id in ids {
        let metadata = state
            .index_db
            .metas
            .get(id)?
            .ok_or_else(|| internal!("id missing during metrics lookup: {}", id))?;

        metadatas.push(metadata);

        tokio::time::sleep(state.config.metrics.time_between_redis_calls).await;
    }

    let time_two_days_ago = get_time().0 as u64 - TWO_DAYS_IN_MILLIS as u64;

    let is_user_active = metadatas.values().any(|metadata| {
        metadata.metadata_version > time_two_days_ago
            || metadata.content_version > time_two_days_ago
    });

    Ok((
        metadatas.filter_not_deleted().map_err(|e| {
            internal!("Cannot filter deleted files for public_key: {:?}, err: {:?}", public_key, e)
        })?,
        is_user_active,
    ))
}

pub async fn calculate_total_document_bytes(
    state: &ServerState, metadatas: &EncryptedFiles,
) -> Result<i64, ServerError<MetricsError>> {
    let mut total_size: u64 = 0;

    for metadata in metadatas.values() {
        if metadata.is_document() && metadata.content_version != 0 {
            let file_usage: FileUsage = match state.index_db.sizes.get(&metadata.id)? {
                Some(size_bytes) => FileUsage { file_id: metadata.id, size_bytes },
                None => continue,
            };

            total_size += file_usage.size_bytes;
            tokio::time::sleep(state.config.metrics.time_between_redis_calls).await;
        }
    }

    Ok(total_size as i64)
}
