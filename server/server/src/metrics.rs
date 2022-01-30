use std::cell::Ref;
use crate::keys::{owned_files, public_key};
use crate::{keys, ServerError, ServerState};
use futures::StreamExt;
use lazy_static::lazy_static;
use libsecp256k1::PublicKey;
use lockbook_models::file_metadata::EncryptedFileMetadata;
use lockbook_models::tree::FileMetadata;
use prometheus::{register_int_gauge_vec, IntGauge, IntGaugeVec};
use prometheus_static_metric::make_static_metric;
use redis::{cmd, AsyncCommands, AsyncIter};
use redis_utils::converters::JsonGet;
use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use serde::de::DeserializeOwned;
use uuid::Uuid;
use warp::Server;
use lockbook_models::api::FileUsage;

make_static_metric! {
    pub struct MetricsStatistics: IntGauge {
        "type" => {
            total_users,
            active_users,
            new_users,
            premium_users,
            total_documents,
            total_document_bytes,
        },
    }
}

lazy_static! {
    pub static ref METRICS_COUNTERS_VEC: IntGaugeVec =
        register_int_gauge_vec!("lockbook_metrics_counters", ".", &["type"]).unwrap();
    pub static ref METRICS_STATISTICS: MetricsStatistics =
        MetricsStatistics::from(&METRICS_COUNTERS_VEC);
}

// # required metrics
// X total users
// active percent
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

pub const TWO_DAYS_IN_MILLIS: u128 = 172800000;

pub async fn init(server_state: &Arc<ServerState>) {
    let state_clone = server_state.clone();

    println!("STARTING");

    tokio::spawn(async move { start(state_clone).await.unwrap() });
}

pub async fn start(server_state: Arc<ServerState>) -> Result<(), ServerError<MetricsError>> {
    println!("STARTING v2");

    loop {
        let public_keys = get_all_public_keys(&server_state).await?;
        METRICS_STATISTICS.total_users.set(public_keys.len() as i64);

        let all_file_ids = get_owned_files_id(&server_state, &public_keys).await?;
        METRICS_STATISTICS.total_documents.set(all_file_ids.len() as i64);

        let metadatas = get_file_metadata(&server_state, &all_file_ids).await?;
        let total_bytes = get_total_size(&server_state, &all_file_ids).await?;

        METRICS_STATISTICS.total_documents.set(metadatas.len() as i64);
        METRICS_STATISTICS.total_document_bytes.set(total_bytes as i64);

        println!("SET FINISHED");

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
    }

    let mut public_keys: Vec<PublicKey> = vec![];

    for key in public_keys_keys {

        let public_key = con.maybe_json_get(&key).await?.ok_or_else(|| {
            internal!(
                "Cannot retrieve public_key despite having a valid key: {:?}",
                key
            )
        })?;

        public_keys.push(public_key)
    }

    Ok(public_keys)
}

pub async fn get_owned_files_id(
    server_state: &Arc<ServerState>,
    public_keys: &[PublicKey],
) -> Result<Vec<Vec<Uuid>>, ServerError<MetricsError>> {
    let mut con = server_state.index_db_pool.get().await?;

    let mut owned_files = Vec::new();

    for public_key in public_keys {
        let user_owned_files: Vec<Uuid> = con
            .maybe_json_get(keys::owned_files(&public_key))
            .await?
            .ok_or_else(|| internal!(
                "Cannot retrieve owned_files despite having a valid public_key: {:?}",
                public_key
            ))?;

        owned_files.push(user_owned_files)
    }

    Ok(owned_files)
}

pub async fn get_file_metadata(
    server_state: &Arc<ServerState>,
    users_ids: &[Vec<Uuid>],
) -> Result<Vec<Vec<EncryptedFileMetadata>>, ServerError<MetricsError>> {
    let mut con = server_state.index_db_pool.get().await?;

    let mut metadatas = vec![];

    for user_ids in users_ids {
        let mut user_metadatas = vec![];

        for id in user_ids {
            let metadata: EncryptedFileMetadata = con.maybe_json_get(keys::file(*id)).await?.ok_or_else(|| internal!("Cannot retrieve encrypted file metadata despite having a valid id."), )?;

            user_metadatas.push(metadata)
        }

        metadatas.push(user_metadatas);
    }

    Ok(metadatas)
}

pub async fn get_total_size(
    server_state: &Arc<ServerState>,
    users_ids: &[Vec<Uuid>]
) -> Result<u64, ServerError<MetricsError>> {
    let mut con = server_state.index_db_pool.get().await?;

    let mut total_size: u64 = 0;

    for user_ids in users_ids {
        for id in user_ids {
            let file_usage: FileUsage = con.maybe_json_get(keys::size(*id)).await?.ok_or_else(|| internal!("Cannot retrieve encrypted file metadata despite having a valid id."), )?;

            total_size += file_usage.size_bytes;
        }
    }

    Ok(total_size)
}

pub async fn do_relevant_stats(
    public_keys: &[PublicKey],
    users_documents: &[Vec<EncryptedFileMetadata>]
) -> Result<(), ServerError<MetricsError>> {
    let mut active_users_count = 0;
    let mut new_users_count = 0;

    let time_two_days_ago = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|e| internal!("{:?}", e))?.as_millis() - TWO_DAYS_IN_MILLIS;

    for documents in users_documents {
        if documents.iter().any(|metadata| metadata.metadata_version as u128 > time_two_days_ago) {
            active_users_count += 1;
        }
    }

    Ok(())
}

// pub async fn redis_map<U: Debug, C, V: DeserializeOwned>(server_state: &ServerState, partial_keys: C, key_gen: impl Fn(Ref<U>) -> String)
//     -> Result<Vec<V>, ServerError<MetricsError>>
// where C: Iterator<Item=U>
// {
//     let mut con = server_state.index_db_pool.get().await?;
//
//     let mut values: Vec<V> = vec![];
//
//     for partial_key in partial_keys {
//         let key = key_gen(partial_key);
//
//         let value: V = con.maybe_json_get(&key).await?.ok_or_else(||
//             internal!("Cannot retrieve key's value from redis: {}.", key),
//         )?;
//
//         values.push(value)
//     }
//
//     Ok(values)
// }

// pub async fn get_total_document_size(server_state: &Arc<ServerState>, document_size)
