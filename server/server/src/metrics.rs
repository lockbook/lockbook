use crate::keys::public_key;
use crate::{keys, ServerError, ServerState};
use lazy_static::lazy_static;
use libsecp256k1::PublicKey;
use lockbook_models::api::FileUsage;
use lockbook_models::file_metadata::{EncryptedFileMetadata, FileType};
use log::error;
use prometheus::{register_int_gauge_vec, IntGaugeVec};
use prometheus_static_metric::make_static_metric;
use redis::{AsyncCommands, AsyncIter};
use redis_utils::converters::JsonGet;
use std::collections::HashSet;
use std::fmt::Debug;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
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
}

// # required metrics
// X total users
// active percent (just active users / total users in metrics.lockbook.com)
// active users
// new users
// X total documents
// X total bytes
// premium users and percentage

#[derive(Debug)]
pub enum MetricsError {}

// pub struct MetricsPreProcessingInfo<'a> {
//     pub public_keys: Vec<PublicKey>,
//     pub ids: HashMap<&'a PublicKey, Uuid>,
//     pub metadatas: HashMap<&'a PublicKey, EncryptedFileMetadata>
// }

pub const TWO_DAYS_IN_MILLIS: u128 = 1000 * 60 * 60 * 24 * 2;

pub fn start_metrics_worker(server_state: &Arc<ServerState>) {
    let state_clone = server_state.clone();

    tokio::spawn(async move {
        if let Err(e) = start(state_clone).await {
            error!("Metrics error: {:?}", e)
        }
    });
}

pub async fn start(server_state: Arc<ServerState>) -> Result<(), ServerError<MetricsError>> {
    loop {
        let public_keys = get_all_public_keys(&server_state).await?;
        METRICS_STATISTICS.total_users.set(public_keys.len() as i64);

        let mut total_documents = 0;
        let mut total_bytes = 0;
        let mut active_users = 0;

        for public_key in public_keys {
            let mut con = server_state.index_db_pool.get().await?;

            let ids = get_users_owned_files(&mut con, &public_key).await?;
            total_documents += ids.len();

            let metadatas = get_user_file_metadatas(&mut con, &ids).await?;
            let bytes = calculate_total_document_bytes(&mut con, &metadatas).await?;
            total_bytes += bytes;

            if is_user_active(&metadatas).await? {
                active_users += 1;
            }

            tokio::time::sleep(server_state.config.metrics.duration_between_user_metrics).await;
        }

        METRICS_STATISTICS
            .total_documents
            .set(total_documents as i64);
        METRICS_STATISTICS.active_users.set(active_users);
        METRICS_STATISTICS.total_document_bytes.set(total_bytes);

        tokio::time::sleep(server_state.config.metrics.duration_between_metrics_refresh).await;
    }
}

pub async fn get_all_public_keys(
    server_state: &Arc<ServerState>,
) -> Result<Vec<PublicKey>, ServerError<MetricsError>> {
    let mut con = server_state.index_db_pool.get().await?;

    let mut public_keys_k: AsyncIter<String> = con.scan_match(public_key("*")).await?;

    let mut public_keys_keys = HashSet::new();
    while let Some(item) = public_keys_k.next_item().await {
        public_keys_keys.insert(item);
        tokio::time::sleep(
            server_state
                .config
                .metrics
                .duration_between_getting_pub_key_key_metrics,
        )
        .await;
    }

    let mut public_keys: Vec<PublicKey> = vec![];

    for key in public_keys_keys {
        let public_key = con.maybe_json_get(&key).await?.ok_or_else(|| {
            internal!(
                "Cannot retrieve public_key despite having a valid key: {:?}",
                key
            )
        })?;

        public_keys.push(public_key);
        tokio::time::sleep(
            server_state
                .config
                .metrics
                .duration_between_getting_pub_key_metrics,
        )
        .await;
    }

    Ok(public_keys)
}

pub async fn get_users_owned_files(
    con: &mut deadpool_redis::Connection,
    public_key: &PublicKey,
) -> Result<Vec<Uuid>, ServerError<MetricsError>> {
    con.maybe_json_get(keys::owned_files(public_key))
        .await?
        .ok_or_else(|| {
            internal!(
                "Cannot retrieve owned_files despite having a valid public_key: {:?}",
                public_key
            )
        })
}

pub async fn get_user_file_metadatas(
    con: &mut deadpool_redis::Connection,
    ids: &[Uuid],
) -> Result<Vec<EncryptedFileMetadata>, ServerError<MetricsError>> {
    let mut metadatas = vec![];

    for id in ids {
        let metadata: EncryptedFileMetadata =
            con.maybe_json_get(keys::file(*id)).await?.ok_or_else(|| {
                internal!("Cannot retrieve encrypted file metadata despite having a valid id.")
            })?;

        metadatas.push(metadata)
    }

    Ok(metadatas)
}

pub async fn calculate_total_document_bytes(
    con: &mut deadpool_redis::Connection,
    metadatas: &[EncryptedFileMetadata],
) -> Result<i64, ServerError<MetricsError>> {
    let mut total_size: u64 = 0;

    for metadata in metadatas {
        if metadata.file_type == FileType::Document {
            let maybe_file_usage: Option<FileUsage> =
                con.maybe_json_get(keys::size(metadata.id)).await?;

            if let Some(usage) = maybe_file_usage {
                total_size += usage.size_bytes;
            }
        }
    }

    Ok(total_size as i64)
}

pub async fn is_user_active(
    metadatas: &[EncryptedFileMetadata],
) -> Result<bool, ServerError<MetricsError>> {
    let time_two_days_ago = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| internal!("{:?}", e))?
        .as_millis()
        - TWO_DAYS_IN_MILLIS;

    let is_active = metadatas.iter().any(|metadata| {
        metadata.metadata_version as u128 > time_two_days_ago
            || metadata.content_version as u128 > time_two_days_ago
    });

    Ok(is_active)
}
