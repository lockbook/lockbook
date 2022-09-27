use crate::{Server, ServerError, ServerState};
use lazy_static::lazy_static;

use hmdb::transaction::Transaction;
use lockbook_shared::clock::get_time;
use prometheus::{register_int_gauge_vec, IntGaugeVec};
use prometheus_static_metric::make_static_metric;
use std::fmt::Debug;
use tracing::*;
use uuid::Uuid;

use crate::billing::billing_model::BillingPlatform;
use crate::transaction::Server as TransactionalServer;
use lockbook_shared::file_like::FileLike;
use lockbook_shared::file_metadata::Owner;
use lockbook_shared::server_tree::ServerTree;
use lockbook_shared::tree_like::{Stagable, TreeLike};

make_static_metric! {
    pub struct MetricsStatistics: IntGauge {
        "type" => {
            total_users,
            premium_users,
            active_users,
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

        let public_keys_and_usernames = state.index_db.usernames.get_all()?;
        METRICS_STATISTICS
            .total_users
            .set(public_keys_and_usernames.len() as i64);

        let mut total_documents = 0;
        let mut total_bytes = 0;
        let mut active_users = 0;

        let mut premium_users = 0;
        let mut premium_stripe_users = 0;
        let mut premium_google_play_users = 0;

        for (username, owner) in public_keys_and_usernames {
            let maybe_billing_platform = get_user_billing_platform(&state.index_db, &owner).await?;

            if let Some(billing_platform) = maybe_billing_platform {
                premium_users += 1;

                match billing_platform {
                    BillingPlatform::GooglePlay { .. } => premium_google_play_users += 1,
                    BillingPlatform::Stripe { .. } => premium_stripe_users += 1,
                }
            }

            let (user_total_documents, user_total_bytes, is_user_active) =
                get_user_info(&state, owner).await?;

            if is_user_active {
                active_users += 1;
            }
            total_documents += user_total_documents;
            total_bytes += user_total_bytes;

            METRICS_USAGE_BY_USER_VEC
                .with_label_values(&[&username])
                .set(user_total_bytes);

            tokio::time::sleep(state.config.metrics.time_between_metrics).await;
        }

        METRICS_STATISTICS.total_documents.set(total_documents);
        METRICS_STATISTICS.active_users.set(active_users);
        METRICS_STATISTICS.total_document_bytes.set(total_bytes);
        METRICS_STATISTICS.premium_users.set(premium_users);

        METRICS_PREMIUM_USERS_BY_PAYMENT_PLATFORM_VEC
            .with_label_values(&[STRIPE_LABEL_NAME])
            .set(premium_stripe_users);
        METRICS_PREMIUM_USERS_BY_PAYMENT_PLATFORM_VEC
            .with_label_values(&[GOOGLE_PLAY_LABEL_NAME])
            .set(premium_google_play_users);

        tokio::time::sleep(state.config.metrics.time_between_metrics_refresh).await;
    }
}

pub async fn get_user_billing_platform(
    db: &Server, owner: &Owner,
) -> Result<Option<BillingPlatform>, ServerError<MetricsError>> {
    let account = db
        .accounts
        .get(owner)?
        .ok_or_else(|| internal!("Could not get user's account during metrics {:?}", owner))?;

    Ok(account.billing_info.billing_platform)
}

pub async fn get_user_info(
    state: &ServerState, owner: Owner,
) -> Result<(i64, i64, bool), ServerError<MetricsError>> {
    state.index_db.transaction(|tx| {
        let mut tree = ServerTree::new(
            owner,
            &mut tx.owned_files,
            &mut tx.shared_files,
            &mut tx.file_children,
            &mut tx.metas,
        )?
        .to_lazy();

        let metadatas = tree.all_files()?;
        let mut ids = Vec::new();

        let time_two_days_ago = get_time().0 as u64 - TWO_DAYS_IN_MILLIS as u64;
        let is_user_active = metadatas
            .iter()
            .any(|metadata| metadata.version > time_two_days_ago);

        for id in tree.owned_ids() {
            if !tree.calculate_deleted(&id)? {
                ids.push(id);
            }
        }

        let (total_documents, total_bytes) = get_bytes_and_documents_count(tx, owner, ids)?;

        Ok((total_documents, total_bytes as i64, is_user_active))
    })?
}

fn get_bytes_and_documents_count(
    db: &mut TransactionalServer, owner: Owner, ids: Vec<Uuid>,
) -> Result<(i64, u64), ServerError<MetricsError>> {
    let mut total_documents = 0;
    let mut total_bytes = 0;

    for id in ids {
        let metadata = db.metas.get(&id).ok_or_else(|| {
            internal!("Could not get file metadata during metrics for {:?}", owner)
        })?;

        if metadata.is_document() {
            if metadata.document_hmac().is_some() {
                let usage = db.sizes.get(&id).ok_or_else(|| {
                    internal!("Could not get file usage during metrics for {:?}", owner)
                })?;

                total_bytes += usage;
            }

            total_documents += 1;
        }
    }

    Ok((total_documents, total_bytes))
}
