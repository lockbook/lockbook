use crate::account_service::get_usage_helper;
use crate::{ServerError, ServerState};
use lazy_static::lazy_static;

use lockbook_shared::clock::get_time;
use lockbook_shared::lazy::LazyTree;
use lockbook_shared::server_file::ServerFile;
use prometheus::{register_int_gauge_vec, IntGaugeVec};
use prometheus_static_metric::make_static_metric;
use std::fmt::Debug;
use tracing::*;
use uuid::Uuid;

use crate::billing::billing_model::{BillingPlatform, SubscriptionProfile};
use crate::schema::ServerDb;
use lockbook_shared::file_like::FileLike;
use lockbook_shared::file_metadata::Owner;
use lockbook_shared::server_tree::ServerTree;
use lockbook_shared::tree_like::TreeLike;

pub struct UserInfo {
    total_documents: i64,
    total_bytes: i64,
    is_user_active: bool,
    is_user_sharer_or_sharee: bool,
}

make_static_metric! {
    pub struct MetricsStatistics: IntGauge {
        "type" => {
            total_users,
            share_feature_users,
            premium_users,
            active_users,
            deleted_users,
            total_documents,
            total_document_bytes,
        },
    }
}

lazy_static! {
    pub static ref METRICS_COUNTERS_VEC: IntGaugeVec = register_int_gauge_vec!(
        "lockbook_metrics_counters",
        "Lockbook's basic metrics of users and files derived from redis",
        &["type"]
    )
    .unwrap();
    pub static ref METRICS_STATISTICS: MetricsStatistics =
        MetricsStatistics::from(&METRICS_COUNTERS_VEC);
    pub static ref METRICS_USAGE_BY_USER_VEC: IntGaugeVec = register_int_gauge_vec!(
        "lockbook_metrics_usage_by_user",
        "Lockbook's total usage by user",
        &["username"]
    )
    .unwrap();
    pub static ref METRICS_PREMIUM_USERS_BY_PAYMENT_PLATFORM_VEC: IntGaugeVec =
        register_int_gauge_vec!(
            "lockbook_premium_users_by_payment_platform",
            "Lockbook's total number of premium users by payment platform",
            &["platform"]
        )
        .unwrap();
}

const STRIPE_LABEL_NAME: &str = "stripe";
const GOOGLE_PLAY_LABEL_NAME: &str = "google-play";
const APP_STORE_LABEL_NAME: &str = "app-store";

#[derive(Debug)]
pub enum MetricsError {}

pub const TWO_DAYS_IN_MILLIS: u128 = 1000 * 60 * 60 * 24 * 2;

pub fn start_metrics_worker(server_state: &ServerState) {
    let state_clone = server_state.clone();

    tokio::spawn(async move {
        info!("Started capturing metrics");

        if let Err(e) = start(state_clone).await {
            error!("interrupting metrics loop due to error: {:?}", e)
        }
    });
}

pub async fn start(state: ServerState) -> Result<(), ServerError<MetricsError>> {
    loop {
        info!("Metrics refresh started");

        let public_keys_and_usernames = state.index_db.lock()?.usernames.data().clone();

        let total_users_ever = public_keys_and_usernames.len() as i64;
        let mut total_documents = 0;
        let mut total_bytes = 0;
        let mut active_users = 0;
        let mut deleted_users = 0;
        let mut share_feature_users = 0;

        let mut premium_users = 0;
        let mut premium_stripe_users = 0;
        let mut premium_google_play_users = 0;
        let mut premium_app_store_users = 0;

        for (username, owner) in public_keys_and_usernames {
            {
                let mut db = state.index_db.lock()?;
                let maybe_user_info = get_user_info(&mut db, owner)?;

                let user_info = match maybe_user_info {
                    None => {
                        deleted_users += 1;
                        continue;
                    }
                    Some(user_info) => user_info,
                };

                if user_info.is_user_active {
                    active_users += 1;
                }
                if user_info.is_user_sharer_or_sharee {
                    share_feature_users += 1;
                }

                total_documents += user_info.total_documents;
                total_bytes += user_info.total_bytes;

                METRICS_USAGE_BY_USER_VEC
                    .with_label_values(&[&username])
                    .set(user_info.total_bytes);

                let billing_info = get_user_billing_info(&db, &owner)?;

                if billing_info.is_premium() {
                    premium_users += 1;

                    match billing_info.billing_platform {
                        None => {
                            return Err(internal!(
                        "Could not retrieve billing platform although it was used moments before."
                    ));
                        }
                        Some(billing_platform) => match billing_platform {
                            BillingPlatform::GooglePlay { .. } => premium_google_play_users += 1,
                            BillingPlatform::Stripe { .. } => premium_stripe_users += 1,
                            BillingPlatform::AppStore { .. } => premium_app_store_users += 1,
                        },
                    }
                }
                drop(db);
            }

            tokio::time::sleep(state.config.metrics.time_between_metrics).await;
        }

        METRICS_STATISTICS
            .total_users
            .set(total_users_ever - deleted_users);

        METRICS_STATISTICS.total_documents.set(total_documents);
        METRICS_STATISTICS.active_users.set(active_users);
        METRICS_STATISTICS.deleted_users.set(deleted_users);
        METRICS_STATISTICS.total_document_bytes.set(total_bytes);
        METRICS_STATISTICS
            .share_feature_users
            .set(share_feature_users);
        METRICS_STATISTICS.premium_users.set(premium_users);

        METRICS_PREMIUM_USERS_BY_PAYMENT_PLATFORM_VEC
            .with_label_values(&[STRIPE_LABEL_NAME])
            .set(premium_stripe_users);
        METRICS_PREMIUM_USERS_BY_PAYMENT_PLATFORM_VEC
            .with_label_values(&[GOOGLE_PLAY_LABEL_NAME])
            .set(premium_google_play_users);
        METRICS_PREMIUM_USERS_BY_PAYMENT_PLATFORM_VEC
            .with_label_values(&[APP_STORE_LABEL_NAME])
            .set(premium_app_store_users);

        tokio::time::sleep(state.config.metrics.time_between_metrics_refresh).await;
    }
}

pub fn get_user_billing_info(
    db: &ServerDb, owner: &Owner,
) -> Result<SubscriptionProfile, ServerError<MetricsError>> {
    let account = db
        .accounts
        .data()
        .get(owner)
        .ok_or_else(|| internal!("Could not get user's account during metrics {:?}", owner))?;

    Ok(account.billing_info.clone())
}

pub fn get_user_info(
    db: &mut ServerDb, owner: Owner,
) -> Result<Option<UserInfo>, ServerError<MetricsError>> {
    if db.owned_files.data().get(&owner).is_none() {
        return Ok(None);
    }

    let mut tree = ServerTree::new(
        owner,
        &mut db.owned_files,
        &mut db.shared_files,
        &mut db.file_children,
        &mut db.metas,
    )?
    .to_lazy();

    let mut ids = Vec::new();

    for id in tree.owned_ids() {
        if !tree.calculate_deleted(&id)? {
            ids.push(id);
        }
    }

    let is_user_sharer_or_sharee = tree
        .all_files()?
        .iter()
        .any(|k| k.owner() != owner || k.is_shared());

    let is_user_active = is_user_active(tree, &ids)?;

    let (total_documents, total_bytes) = get_bytes_and_documents_count(db, owner)?;

    Ok(Some(UserInfo {
        total_documents,
        total_bytes: total_bytes as i64,
        is_user_active,
        is_user_sharer_or_sharee,
    }))
}

fn get_bytes_and_documents_count(
    db: &mut ServerDb, owner: Owner,
) -> Result<(i64, u64), ServerError<MetricsError>> {
    // let mut total_documents = 0;
    // let mut total_bytes = 0;

    // for id in ids {
    //     let metadata = db.metas.data().get(&id).ok_or_else(|| {
    //         internal!("Could not get file metadata during metrics for {:?}", owner)
    //     })?;
    //     if metadata.is_document() {
    //         if metadata.document_hmac().is_some() {
    //             let usage = db.sizes.data().get(&id).ok_or_else(|| {
    //                 internal!("Could not get file usage during metrics for {:?}", owner)
    //             })?;

    //             total_bytes += usage;
    //         }

    //         total_documents += 1;
    //     }
    // }

    let total_bytes = get_usage_helper(db, &owner.0)
        .unwrap()
        .iter()
        .map(|f| f.size_bytes)
        .sum();
    let total_documents = db.owned_files.data().get(&owner).unwrap().len() as i64;

    Ok((total_documents, total_bytes))
}

fn is_user_active<T: lockbook_shared::tree_like::TreeLike<F = ServerFile>>(
    tree: LazyTree<T>, ids: &Vec<Uuid>,
) -> Result<bool, ServerError<MetricsError>> {
    let root = ids
        .iter()
        .find(|id| {
            let file = tree.find(id).unwrap();
            file.parent() == file.id()
        })
        .unwrap();
    let root_creation_timestamp = tree.find(root)?.clone().file.timestamped_value.timestamp;

    let latest_upsert = tree
        .all_files()?
        .iter()
        .max_by(|x, y| {
            x.file
                .timestamped_value
                .timestamp
                .cmp(&y.file.timestamped_value.timestamp)
        })
        .unwrap()
        .file
        .timestamped_value
        .timestamp;

    let time_diff = root_creation_timestamp - latest_upsert;
    let delay_buffer_time = 20;
    Ok(time_diff > delay_buffer_time)
}
