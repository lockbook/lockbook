use crate::billing::app_store_client::AppStoreClient;
use crate::billing::billing_model::{BillingPlatform, SubscriptionProfile};
use crate::billing::google_play_client::GooglePlayClient;
use crate::billing::stripe_client::StripeClient;
use crate::document_service::DocumentService;
use crate::schema::ServerDb;
use crate::{ServerError, ServerState};
use lazy_static::lazy_static;
use lb_rs::model::clock::get_time;
use lb_rs::model::file_like::FileLike;
use lb_rs::model::file_metadata::Owner;
use lb_rs::model::server_tree::ServerTree;
use lb_rs::model::tree_like::TreeLike;
use prometheus::{IntGaugeVec, register_int_gauge_vec};
use prometheus_static_metric::make_static_metric;
use std::fmt::Debug;
use tracing::*;

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

impl<S, A, G, D> ServerState<S, A, G, D>
where
    S: StripeClient,
    A: AppStoreClient,
    G: GooglePlayClient,
    D: DocumentService,
{
    pub fn start_metrics_worker(&self) {
        let state_clone = self.clone();

        tokio::spawn(async move {
            info!("Started capturing metrics");

            if let Err(e) = state_clone.start_metrics_loop().await {
                error!("interrupting metrics loop due to error: {:?}", e)
            }
        });
    }

    pub async fn start_metrics_loop(self) -> Result<(), ServerError<MetricsError>> {
        loop {
            info!("Metrics refresh started");

            let public_keys_and_usernames = self.index_db.lock().await.usernames.get().clone();

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
                    let mut db = self.index_db.lock().await;
                    let maybe_user_info = Self::get_user_info(&mut db, owner)?;

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

                    let billing_info = Self::get_user_billing_info(&db, &owner)?;

                    if billing_info.is_premium() {
                        premium_users += 1;

                        match billing_info.billing_platform {
                            None => {
                                return Err(internal!(
                                    "Could not retrieve billing platform although it was used moments before."
                                ));
                            }
                            Some(billing_platform) => match billing_platform {
                                BillingPlatform::GooglePlay { .. } => {
                                    premium_google_play_users += 1
                                }
                                BillingPlatform::Stripe { .. } => premium_stripe_users += 1,
                                BillingPlatform::AppStore { .. } => premium_app_store_users += 1,
                            },
                        }
                    }
                    drop(db);
                }

                tokio::time::sleep(self.config.metrics.time_between_metrics).await;
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

            info!("metrics refresh finished");
            tokio::time::sleep(self.config.metrics.time_between_metrics_refresh).await;
        }
    }
    pub fn get_user_billing_info(
        db: &ServerDb, owner: &Owner,
    ) -> Result<SubscriptionProfile, ServerError<MetricsError>> {
        let account =
            db.accounts.get().get(owner).ok_or_else(|| {
                internal!("Could not get user's account during metrics {:?}", owner)
            })?;

        Ok(account.billing_info.clone())
    }

    pub fn get_user_info(
        db: &mut ServerDb, owner: Owner,
    ) -> Result<Option<UserInfo>, ServerError<MetricsError>> {
        if db.owned_files.get().get(&owner).is_none() {
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

        for id in tree.ids() {
            if !tree.calculate_deleted(&id)? {
                ids.push(id);
            }
        }

        let is_user_sharer_or_sharee = tree
            .all_files()?
            .iter()
            .any(|k| k.owner() != owner || k.is_shared());

        let root_creation_timestamp =
            if let Some(root_creation_timestamp) = tree.all_files()?.iter().find(|f| f.is_root()) {
                root_creation_timestamp.file.timestamped_value.timestamp
            } else {
                return Ok(None);
            };

        let last_seen = *db
            .last_seen
            .get()
            .get(&owner)
            .unwrap_or(&(root_creation_timestamp as u64));

        let time_two_days_ago = get_time().0 as u64 - TWO_DAYS_IN_MILLIS as u64;
        let last_seen_since_account_creation = last_seen as i64 - root_creation_timestamp;
        let delay_buffer_time = 5000;
        let not_the_welcome_doc = last_seen_since_account_creation > delay_buffer_time;
        let is_user_active = not_the_welcome_doc && last_seen > time_two_days_ago;

        let total_bytes: u64 = Self::get_usage_helper(&mut tree)
            .unwrap_or_default()
            .iter()
            .map(|f| f.size_bytes)
            .sum();

        let total_documents = if let Some(owned_files) = db.owned_files.get().get(&owner) {
            owned_files.len() as i64
        } else {
            return Ok(None);
        };

        Ok(Some(UserInfo {
            total_documents,
            total_bytes: total_bytes as i64,
            is_user_active,
            is_user_sharer_or_sharee,
        }))
    }
}
