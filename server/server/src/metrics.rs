use crate::keys::public_key;
use crate::{keys, ServerError, ServerState};
use lazy_static::lazy_static;
use libsecp256k1::PublicKey;
use lockbook_crypto::clock_service::get_time;
use lockbook_models::api::FileUsage;
use lockbook_models::file_metadata::{EncryptedFileMetadata, FileType};
use lockbook_models::tree::FileMetaExt;
use log::{error, info};
use prometheus::{register_int_gauge_vec, IntGaugeVec};
use prometheus_static_metric::make_static_metric;
use redis::{AsyncCommands, AsyncIter};
use redis_utils::converters::JsonGet;
use std::collections::HashSet;
use std::fmt::Debug;
use std::sync::Arc;
use std::time::Duration;
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

pub fn start_metrics_worker(server_state: &Arc<ServerState>) {
    let state_clone = server_state.clone();

    tokio::spawn(async move {
        info!("Started capturing metrics.");

        if let Err(e) = start(state_clone).await {
            error!("interrupting metrics loop due to error: {:?}", e)
        }
    });
}

pub async fn start(server_state: Arc<ServerState>) -> Result<(), ServerError<MetricsError>> {
    loop {
        info!("Metrics refresh started.");

        let public_keys_and_usernames = get_all_public_keys_and_usernames(&server_state).await?;
        METRICS_STATISTICS
            .total_users
            .set(public_keys_and_usernames.len() as i64);

        let mut total_documents = 0;
        let mut total_bytes = 0;
        let mut active_users = 0;

        println!("LEN: {}", public_keys_and_usernames.len());

        for (public_key, username) in public_keys_and_usernames {
            let mut con = server_state.index_db_pool.get().await?;
            let ids = get_owned(&mut con, &public_key).await?;
            let (metadatas, is_user_active) = get_metadatas_and_user_activity_state(
                &mut con,
                &public_key,
                &ids,
                &server_state.config.metrics.time_between_redis_calls,
            )
            .await?;

            if is_user_active {
                active_users += 1;
            }

            let bytes = calculate_total_document_bytes(
                &mut con,
                &metadatas,
                &server_state.config.metrics.time_between_redis_calls,
            )
            .await?;

            total_bytes += bytes;
            total_documents += metadatas.len();

            METRICS_USAGE_BY_USER_VEC
                .with_label_values(&[&username])
                .set(bytes);

            tokio::time::sleep(server_state.config.metrics.time_between_redis_calls).await;
        }

        METRICS_STATISTICS
            .total_documents
            .set(total_documents as i64);
        METRICS_STATISTICS.active_users.set(active_users);
        METRICS_STATISTICS.total_document_bytes.set(total_bytes);

        tokio::time::sleep(server_state.config.metrics.time_between_metrics_refresh).await;
    }
}

pub async fn get_all_public_keys_and_usernames(
    server_state: &Arc<ServerState>,
) -> Result<Vec<(PublicKey, String)>, ServerError<MetricsError>> {
    let mut con = server_state.index_db_pool.get().await?;

    let mut keys_iter: AsyncIter<String> = con.scan_match(public_key("*")).await?;

    let mut keys = HashSet::new();

    while let Some(item) = keys_iter.next_item().await {
        keys.insert(item);
        tokio::time::sleep(server_state.config.metrics.time_between_redis_calls).await;
    }

    let mut public_keys_and_usernames: Vec<(PublicKey, String)> = vec![];

    for key in keys {
        let public_key = con
            .maybe_json_get(&key)
            .await?
            .ok_or_else(|| internal!("Cannot retrieve public_key for key: {:?}", key))?;

        let mut parts = key.split(':');
        parts.next();
        let username = parts
            .next()
            .ok_or_else(|| internal!("Cannot find username in public_key key: {:?}", key))?
            .to_string();

        public_keys_and_usernames.push((public_key, username));
        tokio::time::sleep(server_state.config.metrics.time_between_redis_calls).await;
    }

    Ok(public_keys_and_usernames)
}

pub async fn get_owned(
    con: &mut deadpool_redis::Connection, public_key: &PublicKey,
) -> Result<Vec<Uuid>, ServerError<MetricsError>> {
    con.maybe_json_get(keys::owned_files(public_key))
        .await?
        .ok_or_else(|| internal!("Cannot retrieve owned_files for public_key: {:?}", public_key))
}

pub async fn get_metadatas_and_user_activity_state(
    con: &mut deadpool_redis::Connection, public_key: &PublicKey, ids: &[Uuid],
    time_between_redis_calls: &Duration,
) -> Result<(Vec<EncryptedFileMetadata>, bool), ServerError<MetricsError>> {
    let mut metadatas = vec![];

    for id in ids {
        let metadata: EncryptedFileMetadata = con
            .maybe_json_get(keys::file(*id))
            .await?
            .ok_or_else(|| internal!("Cannot retrieve encrypted file metadata for id: {:?}", id))?;

        metadatas.push(metadata);

        tokio::time::sleep(*time_between_redis_calls).await;
    }

    let time_two_days_ago = get_time().0 as u64 - TWO_DAYS_IN_MILLIS as u64;

    let is_user_active = metadatas.iter().any(|metadata| {
        // println!("metadata_version: {} content_version: {} time_two_days_ago: {} | {}",
        //          metadata.metadata_version,
        //          metadata.content_version,
        //          time_two_days_ago,
        //          metadata.metadata_version > time_two_days_ago || metadata.content_version > time_two_days_ago
        // );

        metadata.metadata_version > time_two_days_ago
            || metadata.content_version > time_two_days_ago
    });

    println!("IS ACTIVE: {}", is_user_active);

    Ok((
        metadatas.filter_not_deleted().map_err(|e| {
            internal!("Cannot filter deleted files for public_key: {:?}, err: {:?}", public_key, e)
        })?,
        is_user_active,
    ))
}

pub async fn calculate_total_document_bytes(
    con: &mut deadpool_redis::Connection, metadatas: &[EncryptedFileMetadata],
    time_between_redis_calls: &Duration,
) -> Result<i64, ServerError<MetricsError>> {
    let mut total_size: u64 = 0;

    for metadata in metadatas {
        if metadata.file_type == FileType::Document && metadata.content_version != 0 {
            let file_usage: FileUsage = match con.maybe_json_get(keys::size(metadata.id)).await? {
                Some(usage) => usage,
                None => continue,
            };

            total_size += file_usage.size_bytes;
            tokio::time::sleep(*time_between_redis_calls).await;
        }
    }

    Ok(total_size as i64)
}
