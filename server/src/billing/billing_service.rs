use crate::ServerError::ClientError;
use crate::billing::app_store_model::{NotificationChange, Subtype};
use crate::billing::billing_model::{
    AppStoreUserInfo, BillingPlatform, GooglePlayUserInfo, StripeUserInfo,
};
use crate::billing::billing_service::LockBillingWorkflowError::{
    ExistingRequestPending, UserNotFound,
};
use crate::billing::google_play_model::NotificationType;
use crate::document_service::DocumentService;
use crate::schema::Account;
use crate::{RequestContext, ServerError, ServerState};
use base64::DecodeError;
use db_rs::Db;
use lb_rs::model::api::{
    AdminSetUserTierError, AdminSetUserTierInfo, AdminSetUserTierRequest, AdminSetUserTierResponse,
    AppStoreAccountState, CancelSubscriptionError, CancelSubscriptionRequest,
    CancelSubscriptionResponse, FREE_TIER_USAGE_SIZE, GetSubscriptionInfoError,
    GetSubscriptionInfoRequest, GetSubscriptionInfoResponse, GooglePlayAccountState,
    PaymentPlatform, StripeAccountState, SubscriptionInfo, UpgradeAccountAppStoreError,
    UpgradeAccountAppStoreRequest, UpgradeAccountAppStoreResponse, UpgradeAccountGooglePlayError,
    UpgradeAccountGooglePlayRequest, UpgradeAccountGooglePlayResponse, UpgradeAccountStripeError,
    UpgradeAccountStripeRequest, UpgradeAccountStripeResponse,
};
use lb_rs::model::clock::get_time;
use lb_rs::model::file_metadata::Owner;
use lb_rs::model::server_tree::ServerTree;
use lb_rs::model::tree_like::TreeLike;
use libsecp256k1::PublicKey;
use std::collections::HashMap;
use std::fmt::Debug;
use std::ops::DerefMut;
use tracing::*;
use warp::http::HeaderValue;
use warp::hyper::body::Bytes;

use super::app_store_client::AppStoreClient;
use super::google_play_client::GooglePlayClient;
use super::stripe_client::StripeClient;

impl<S, A, G, D> ServerState<S, A, G, D>
where
    S: StripeClient,
    A: AppStoreClient,
    G: GooglePlayClient,
    D: DocumentService,
{
    async fn lock_subscription_profile(
        &self, public_key: &PublicKey,
    ) -> Result<Account, ServerError<LockBillingWorkflowError>> {
        let owner = Owner(*public_key);
        let mut db = self.index_db.lock().await;
        let tx = db.begin_transaction()?;
        let mut account = db
            .accounts
            .get()
            .get(&owner)
            .ok_or(ClientError(UserNotFound))?
            .clone();

        let current_time = get_time().0 as u64;

        if current_time - account.billing_info.last_in_payment_flow
            < self.config.billing.millis_between_user_payment_flows
        {
            warn!(
                ?owner,
                "User/Webhook is already in payment flow, or not enough time that has elapsed since a failed attempt"
            );

            return Err(ClientError(ExistingRequestPending));
        }

        account.billing_info.last_in_payment_flow = current_time;
        db.accounts.insert(owner, account.clone())?;

        debug!(?owner, "User successfully entered payment flow");

        tx.drop_safely()?;
        Ok(account)
    }

    async fn release_subscription_profile<T: Debug>(
        &self, public_key: PublicKey, mut account: Account,
    ) -> Result<(), ServerError<T>> {
        account.billing_info.last_in_payment_flow = 0;
        self.index_db
            .lock()
            .await
            .accounts
            .insert(Owner(public_key), account)?;
        Ok(())
    }

    pub async fn upgrade_account_app_store(
        &self, context: RequestContext<UpgradeAccountAppStoreRequest>,
    ) -> Result<UpgradeAccountAppStoreResponse, ServerError<UpgradeAccountAppStoreError>> {
        let request = &context.request;

        let mut account = self.lock_subscription_profile(&context.public_key).await?;

        debug!("Upgrading the account of a user through app store billing");

        {
            let db = self.index_db.lock().await;
            if db
                .app_store_ids
                .get()
                .get(&request.app_account_token)
                .is_some()
            {
                return Err(ClientError(UpgradeAccountAppStoreError::AppStoreAccountAlreadyLinked));
            }
        }

        if account.billing_info.is_premium() {
            return Err(ClientError(UpgradeAccountAppStoreError::AlreadyPremium));
        }

        let (expires, account_state) = Self::verify_details(
            &self.app_store_client,
            &self.config.billing.apple,
            &request.app_account_token,
            &request.original_transaction_id,
        )
        .await?;

        debug!("Successfully verified app store subscription");

        account.billing_info.billing_platform = Some(BillingPlatform::AppStore(AppStoreUserInfo {
            account_token: request.app_account_token.clone(),
            original_transaction_id: request.original_transaction_id.clone(),
            subscription_product_id: self.config.billing.apple.subscription_product_id.clone(),
            expiration_time: expires,
            account_state,
        }));

        self.index_db
            .lock()
            .await
            .app_store_ids
            .insert(request.app_account_token.clone(), Owner(context.public_key))?;

        self.release_subscription_profile::<UpgradeAccountAppStoreError>(
            context.public_key,
            account,
        )
        .await?;

        Ok(UpgradeAccountAppStoreResponse {})
    }

    pub async fn upgrade_account_google_play(
        &self, context: RequestContext<UpgradeAccountGooglePlayRequest>,
    ) -> Result<UpgradeAccountGooglePlayResponse, ServerError<UpgradeAccountGooglePlayError>> {
        let request = &context.request;

        let mut account = self.lock_subscription_profile(&context.public_key).await?;

        if account.billing_info.is_premium() {
            return Err(ClientError(UpgradeAccountGooglePlayError::AlreadyPremium));
        }

        debug!("Upgrading the account of a user through google play billing");

        self.google_play_client
            .acknowledge_subscription(&self.config, &request.purchase_token)
            .await?;

        debug!("Acknowledged a user's google play subscription");

        let expiry_info = self
            .google_play_client
            .get_subscription(&self.config, &request.purchase_token)
            .await?;

        account.billing_info.billing_platform = Some(BillingPlatform::new_play_sub(
            &self.config,
            &request.purchase_token,
            expiry_info,
        )?);

        self.index_db
            .lock()
            .await
            .google_play_ids
            .insert(request.account_id.clone(), Owner(context.public_key))?;

        self.release_subscription_profile::<UpgradeAccountGooglePlayError>(
            context.public_key,
            account,
        )
        .await?;

        debug!("Successfully upgraded a user through a google play subscription. public_key");

        Ok(UpgradeAccountGooglePlayResponse {})
    }

    pub async fn upgrade_account_stripe(
        &self, context: RequestContext<UpgradeAccountStripeRequest>,
    ) -> Result<UpgradeAccountStripeResponse, ServerError<UpgradeAccountStripeError>> {
        let request = &context.request;

        debug!("Attempting to upgrade the account tier of to premium");

        let mut account = self.lock_subscription_profile(&context.public_key).await?;

        if account.billing_info.is_premium() {
            return Err(ClientError(UpgradeAccountStripeError::AlreadyPremium));
        }

        let maybe_user_info = account
            .billing_info
            .billing_platform
            .and_then(|info| match info {
                BillingPlatform::Stripe(stripe_info) => Some(stripe_info),
                _ => None,
            });

        let user_info = self
            .create_subscription(&context.public_key, &request.account_tier, maybe_user_info)
            .await?;

        account.billing_info.billing_platform = Some(BillingPlatform::Stripe(user_info));
        self.release_subscription_profile::<UpgradeAccountStripeError>(context.public_key, account)
            .await?;

        debug!("Successfully upgraded the account tier of from free to premium");

        Ok(UpgradeAccountStripeResponse {})
    }

    pub async fn get_subscription_info(
        &self, context: RequestContext<GetSubscriptionInfoRequest>,
    ) -> Result<GetSubscriptionInfoResponse, ServerError<GetSubscriptionInfoError>> {
        let platform = self
            .index_db
            .lock()
            .await
            .accounts
            .get()
            .get(&Owner(context.public_key))
            .ok_or(ClientError(GetSubscriptionInfoError::UserNotFound))?
            .billing_info
            .billing_platform
            .clone();

        let subscription_info = platform.map(|info| match info {
            BillingPlatform::Stripe(info) => SubscriptionInfo {
                payment_platform: PaymentPlatform::Stripe { card_last_4_digits: info.last_4 },
                period_end: info.expiration_time,
            },
            BillingPlatform::GooglePlay(info) => SubscriptionInfo {
                payment_platform: PaymentPlatform::GooglePlay { account_state: info.account_state },
                period_end: info.expiration_time,
            },
            BillingPlatform::AppStore(info) => SubscriptionInfo {
                payment_platform: PaymentPlatform::AppStore { account_state: info.account_state },
                period_end: info.expiration_time,
            },
        });

        Ok(GetSubscriptionInfoResponse { subscription_info })
    }

    pub async fn cancel_subscription(
        &self, context: RequestContext<CancelSubscriptionRequest>,
    ) -> Result<CancelSubscriptionResponse, ServerError<CancelSubscriptionError>> {
        let mut account = self.lock_subscription_profile(&context.public_key).await?;

        if account.billing_info.data_cap() == FREE_TIER_USAGE_SIZE {
            return Err(ClientError(CancelSubscriptionError::NotPremium));
        }

        {
            let mut lock = self.index_db.lock().await;
            let db = lock.deref_mut();

            let mut tree = ServerTree::new(
                Owner(context.public_key),
                &mut db.owned_files,
                &mut db.shared_files,
                &mut db.file_children,
                &mut db.metas,
            )?
            .to_lazy();

            let usage: u64 = Self::get_usage_helper(&mut tree)?
                .iter()
                .map(|a| a.size_bytes)
                .sum();

            if usage > FREE_TIER_USAGE_SIZE {
                debug!("Cannot downgrade user to free since they are over the data cap");
                return Err(ClientError(CancelSubscriptionError::UsageIsOverFreeTierDataCap));
            }
        }

        match account.billing_info.billing_platform {
            None => {
                return Err(internal!(
                    "A user somehow has premium tier usage, but no billing information on redis. public_key: {:?}",
                    context.public_key
                ));
            }
            Some(BillingPlatform::GooglePlay(ref mut info)) => {
                debug!("Canceling google play subscription of user");

                if let GooglePlayAccountState::Canceled = &info.account_state {
                    return Err(ClientError(CancelSubscriptionError::AlreadyCanceled));
                }

                self.google_play_client
                    .cancel_subscription(&self.config, &info.purchase_token)
                    .await?;

                info.account_state = GooglePlayAccountState::Canceled;
                debug!("Successfully canceled google play subscription of user");
            }
            Some(BillingPlatform::Stripe(ref mut info)) => {
                debug!("Canceling stripe subscription of user");

                self.stripe_client
                    .cancel_subscription(&info.subscription_id.parse()?)
                    .await
                    .map_err::<ServerError<CancelSubscriptionError>, _>(|err| {
                        internal!("{:?}", err)
                    })?;

                info.account_state = StripeAccountState::Canceled;

                debug!("Successfully canceled stripe subscription");
            }
            Some(BillingPlatform::AppStore(_)) => {
                return Err(ClientError(CancelSubscriptionError::CannotCancelForAppStore));
            }
        }

        self.release_subscription_profile::<CancelSubscriptionError>(context.public_key, account)
            .await?;

        Ok(CancelSubscriptionResponse {})
    }

    pub async fn admin_set_user_tier(
        &self, context: RequestContext<AdminSetUserTierRequest>,
    ) -> Result<AdminSetUserTierResponse, ServerError<AdminSetUserTierError>> {
        let request = &context.request;

        {
            let db = self.index_db.lock().await;

            if !Self::is_admin::<AdminSetUserTierError>(
                &db,
                &context.public_key,
                &self.config.admin.admins,
            )? {
                return Err(ClientError(AdminSetUserTierError::NotPermissioned));
            }
        }

        let public_key = self
            .index_db
            .lock()
            .await
            .usernames
            .get()
            .get(&request.username)
            .ok_or(ClientError(AdminSetUserTierError::UserNotFound))?
            .0;
        let mut account = self.lock_subscription_profile(&public_key).await?;

        let billing_config = &self.config.billing;

        account.billing_info.billing_platform = match &request.info {
            AdminSetUserTierInfo::Stripe {
                customer_id,
                customer_name,
                payment_method_id,
                last_4,
                subscription_id,
                expiration_time,
                account_state,
            } => Some(BillingPlatform::Stripe(StripeUserInfo {
                customer_id: customer_id.to_string(),
                customer_name: *customer_name,
                price_id: billing_config.stripe.premium_price_id.to_string(),
                payment_method_id: payment_method_id.to_string(),
                last_4: last_4.to_string(),
                subscription_id: subscription_id.to_string(),
                expiration_time: *expiration_time,
                account_state: account_state.clone(),
            })),
            AdminSetUserTierInfo::GooglePlay { purchase_token, expiration_time, account_state } => {
                Some(BillingPlatform::GooglePlay(GooglePlayUserInfo {
                    purchase_token: purchase_token.clone(),
                    subscription_product_id: billing_config
                        .google
                        .premium_subscription_product_id
                        .to_string(),
                    subscription_offer_id: billing_config
                        .google
                        .premium_subscription_offer_id
                        .to_string(),
                    expiration_time: *expiration_time,
                    account_state: account_state.clone(),
                }))
            }
            AdminSetUserTierInfo::AppStore {
                account_token,
                original_transaction_id,
                expiration_time,
                account_state,
            } => Some(BillingPlatform::AppStore(AppStoreUserInfo {
                account_token: account_token.to_string(),
                original_transaction_id: original_transaction_id.to_string(),
                subscription_product_id: billing_config.apple.subscription_product_id.to_string(),
                expiration_time: *expiration_time,
                account_state: account_state.clone(),
            })),
            AdminSetUserTierInfo::Free => None,
        };

        account.username = request.username.clone();

        self.release_subscription_profile::<AdminSetUserTierError>(public_key, account)
            .await?;

        Ok(AdminSetUserTierResponse {})
    }

    async fn save_subscription_profile<
        T: Debug,
        F: Fn(&mut Account) -> Result<(), ServerError<T>>,
    >(
        &self, public_key: &PublicKey, update_subscription_profile: F,
    ) -> Result<(), ServerError<T>> {
        let millis_between_lock_attempts = self.config.billing.time_between_lock_attempts;
        loop {
            match self.lock_subscription_profile(public_key).await {
                Ok(ref mut sub_profile) => {
                    update_subscription_profile(sub_profile)?;
                    self.release_subscription_profile(*public_key, sub_profile.clone())
                        .await?;
                    break;
                }
                Err(ClientError(ExistingRequestPending)) => {
                    tokio::time::sleep(millis_between_lock_attempts).await;
                    continue;
                }
                Err(err) => {
                    return Err(internal!("Cannot get billing lock in webhooks: {:#?}", err));
                }
            }
        }

        Ok(())
    }

    pub async fn stripe_webhooks(
        &self, request_body: Bytes, stripe_sig: HeaderValue,
    ) -> Result<(), ServerError<StripeWebhookError>> {
        let event = self.verify_request_and_get_event(&request_body, stripe_sig)?;

        let event_type = event.type_;
        debug!(?event_type, "Verified stripe request");

        match (&event.type_, &event.data.object) {
            (stripe::EventType::InvoicePaymentFailed, stripe::EventObject::Invoice(invoice)) => {
                if let Some(stripe::InvoiceBillingReason::SubscriptionCycle) =
                    invoice.billing_reason
                {
                    let public_key = self.get_public_key_from_invoice(invoice).await?;
                    let owner = Owner(public_key);

                    debug!(
                        ?owner,
                        "User's tier is being reduced due to failed renewal payment in stripe"
                    );

                    self.save_subscription_profile(&public_key, |account| {
                        if let Some(BillingPlatform::Stripe(ref mut info)) =
                            account.billing_info.billing_platform
                        {
                            info.account_state = StripeAccountState::InvoiceFailed;

                            Ok(())
                        } else {
                            Err(internal!(
                                "Cannot get any billing info for user. public_key: {:?}",
                                public_key
                            ))
                        }
                    })
                    .await?;
                }
            }
            (stripe::EventType::InvoicePaid, stripe::EventObject::Invoice(invoice)) => {
                if let Some(stripe::InvoiceBillingReason::SubscriptionCycle) =
                    invoice.billing_reason
                {
                    let public_key = self.get_public_key_from_invoice(invoice).await?;
                    let owner = Owner(public_key);

                    debug!(
                        ?owner,
                        "User's subscription period_end is being changed after successful renewal",
                    );

                    let subscription_period_end = match &invoice.subscription {
                        Some(stripe::Expandable::Object(subscription)) => {
                            subscription.current_period_end
                        }
                        Some(stripe::Expandable::Id(subscription_id)) => {
                            self.stripe_client
                                .get_subscription(subscription_id)
                                .await
                                .map_err::<ServerError<StripeWebhookError>, _>(|err| {
                                    internal!("{:?}", err)
                                })?
                                .current_period_end
                        }
                        None => {
                            return Err(internal!(
                                "The subscription should be included in this invoice: {:?}",
                                invoice
                            ));
                        }
                    };

                    self.save_subscription_profile(&public_key, |account| {
                        if let Some(BillingPlatform::Stripe(ref mut info)) =
                            account.billing_info.billing_platform
                        {
                            info.account_state = StripeAccountState::Ok;
                            info.expiration_time = subscription_period_end as u64;

                            Ok(())
                        } else {
                            Err(internal!(
                                "Cannot get any billing info for user. public_key: {:?}",
                                public_key
                            ))
                        }
                    })
                    .await?;
                }
            }
            (_, _) => {
                return Err(internal!("Unexpected stripe event: {:?}", event.type_));
            }
        }

        Ok(())
    }

    pub async fn google_play_notification_webhooks(
        &self, request_body: Bytes, query_parameters: HashMap<String, String>,
    ) -> Result<(), ServerError<GooglePlayWebhookError>> {
        let notification = self
            .verify_request_and_get_notification(request_body, query_parameters)
            .await?;

        if let Some(sub_notif) = notification.subscription_notification {
            debug!(?sub_notif, "Notification is for a subscription");

            let subscription = self
                .google_play_client
                .get_subscription(&self.config, &sub_notif.purchase_token)
                .await
                .map_err(|e| internal!("{:#?}", e))?;

            let notification_type = sub_notif.notification_type();
            if let NotificationType::SubscriptionPurchased = notification_type {
                return Ok(());
            }

            let public_key = self
                .get_public_key_from_subnotif(&sub_notif, &subscription, &notification_type)
                .await?;
            let owner = Owner(public_key);

            debug!(
                ?owner,
                ?notification_type,
                "Updating google play user's subscription profile to match new subscription state"
            );

            self.save_subscription_profile(&public_key, |account| {
                if let Some(BillingPlatform::GooglePlay(ref mut info)) =
                    account.billing_info.billing_platform
                {
                    match notification_type {
                        NotificationType::SubscriptionRecovered
                        | NotificationType::SubscriptionRestarted
                        | NotificationType::SubscriptionRenewed => {
                            info.account_state = GooglePlayAccountState::Ok;
                            info.expiration_time = Self::get_subscription_period_end(
                                &subscription,
                                &notification_type,
                                public_key,
                            )?;
                        }
                        NotificationType::SubscriptionInGracePeriod => {
                            info.account_state = GooglePlayAccountState::GracePeriod;
                        }
                        NotificationType::SubscriptionOnHold => {
                            info.account_state = GooglePlayAccountState::OnHold;
                            info.expiration_time = Self::get_subscription_period_end(
                                &subscription,
                                &notification_type,
                                public_key,
                            )?;
                        }
                        NotificationType::SubscriptionExpired
                        | NotificationType::SubscriptionRevoked => {
                            if info.purchase_token == sub_notif.purchase_token {
                                account.billing_info.billing_platform = None
                            } else {
                                let old_purchase_token = &sub_notif.purchase_token;
                                let new_purchase_token = &info.purchase_token;
                                debug!(
                                ?old_purchase_token,
                                ?new_purchase_token,
                                "Expired or revoked subscription was tied to an old purchase_token"
                            );
                            }
                        }
                        NotificationType::SubscriptionCanceled => {
                            info.account_state = GooglePlayAccountState::Canceled;
                            let cancellation_reason = &subscription.cancel_survey_result;
                            let owner = Owner(public_key);
                            debug!(?cancellation_reason, ?owner, "Subscription cancelled");
                        }
                        NotificationType::SubscriptionPriceChangeConfirmed
                        | NotificationType::SubscriptionDeferred
                        | NotificationType::SubscriptionPaused
                        | NotificationType::SubscriptionPausedScheduleChanged
                        | NotificationType::SubscriptionPurchased => {
                            return Err(internal!(
                                "Unexpected subscription notification: {:?}, public_key: {:?}",
                                notification_type,
                                public_key
                            ))
                        }
                        NotificationType::Unknown => {
                            return Err(internal!(
                                "Unknown subscription change. public_key: {:?}",
                                public_key
                            ))
                        }
                    }

                    Ok(())
                } else {
                    Err(internal!(
                        "Cannot get any billing info for user. public_key: {:?}",
                        public_key
                    ))
                }
            })
            .await?;
        }

        if let Some(test_notif) = notification.test_notification {
            let version = &test_notif.version;
            debug!(?version, "Test notification");
        }

        if let Some(otp_notif) = notification.one_time_product_notification {
            return Err(internal!(
                "Received a one time product notification although there are no registered one time products. one_time_product_notification: {:?}",
                otp_notif
            ));
        }

        Ok(())
    }

    pub async fn app_store_notification_webhook(
        &self, body: Bytes,
    ) -> Result<(), ServerError<AppStoreNotificationError>> {
        let resp = Self::decode_verify_notification(&self.config.billing.apple, &body)?;

        if let NotificationChange::Subscribed = resp.notification_type {
            return Ok(());
        } else if let NotificationChange::Test = resp.notification_type {
            debug!(?resp, "This is a test notification.");
            return Ok(());
        }

        let trans = Self::decode_verify_transaction(
            &self.config.billing.apple,
            &resp
                .clone()
                .data
                .encoded_transaction_info
                .ok_or(ClientError(AppStoreNotificationError::InvalidJWS))?,
        )?;
        let public_key = self.get_public_key_from_tx(&trans).await?;

        let owner = Owner(public_key);
        let maybe_username = &self
            .index_db
            .lock()
            .await
            .accounts
            .get()
            .get(&owner)
            .map(|acc| acc.username.clone());

        info!(
            ?owner,
            ?maybe_username,
            ?resp.notification_type,
            ?resp.subtype,
            "Updating app store user's subscription profile to match new subscription state"
        );

        self.save_subscription_profile(&public_key, |account| {
            if let Some(BillingPlatform::AppStore(ref mut info)) =
                account.billing_info.billing_platform
            {
                match resp.notification_type {
                    NotificationChange::DidFailToRenew => {
                        info.account_state = if let Some(Subtype::GracePeriod) = resp.subtype {
                            AppStoreAccountState::GracePeriod
                        } else {
                            AppStoreAccountState::FailedToRenew
                        };
                    }
                    NotificationChange::Expired => {
                        info.account_state = AppStoreAccountState::Expired;

                        match resp.subtype {
                            Some(Subtype::BillingRetry) => {
                                info!(
                                    ?owner,
                                    ?resp,
                                    "Subscription failed to renew due to billing issues."
                                );
                            }
                            Some(Subtype::Voluntary) => {
                                info!(?owner, ?resp, "Subscription cancelled");
                            }
                            _ => {
                                return Err(internal!(
                            "Unexpected subtype: {:?}, notification_type {:?}, public_key: {:?}",
                            resp.subtype,
                            resp.notification_type,
                            public_key
                        ))
                            }
                        }
                    }
                    NotificationChange::GracePeriodExpired => {
                        info.account_state = AppStoreAccountState::Expired
                    }
                    NotificationChange::Refund => {
                        info!(?resp, "A user has requested a refund.");
                    }
                    NotificationChange::RefundDeclined => {
                        info!(?resp, "A user's refund request has been denied.");
                    }
                    NotificationChange::DidChangeRenewalStatus => match resp.subtype {
                        Some(Subtype::AutoRenewEnabled) => {
                            info.account_state = AppStoreAccountState::Ok;
                            info.expiration_time = trans.expires_date;
                        }
                        Some(Subtype::AutoRenewDisabled) => {}
                        _ => {
                            return Err(internal!(
                            "Unexpected subtype: {:?}, notification_type {:?}, public_key: {:?}",
                            resp.subtype,
                            resp.notification_type,
                            public_key
                        ))
                        }
                    },
                    NotificationChange::RenewalExtended => {
                        info.expiration_time = trans.expires_date
                    }
                    NotificationChange::DidRenew => {
                        if let Some(Subtype::BillingRecovery) = resp.subtype {
                            info.account_state = AppStoreAccountState::Ok;
                        }
                        info.expiration_time = trans.expires_date
                    }
                    NotificationChange::ConsumptionRequest
                    | NotificationChange::Subscribed
                    | NotificationChange::DidChangeRenewalPref
                    | NotificationChange::OfferRedeemed
                    | NotificationChange::PriceIncrease
                    | NotificationChange::Revoke
                    | NotificationChange::Test => {
                        return Err(internal!(
                            "Unexpected notification change: {:?} {:?}, public_key: {:?}",
                            resp.notification_type,
                            resp.subtype,
                            public_key
                        ))
                    }
                }

                Ok(())
            } else {
                Err(internal!("Cannot get any billing info for user. public_key: {:?}", public_key))
            }
        })
        .await?;

        Ok(())
    }
}

pub fn stringify_public_key(pk: &PublicKey) -> String {
    base64::encode(pk.serialize_compressed())
}

#[derive(Debug)]
pub enum StripeWebhookError {
    VerificationError(String),
    InvalidHeader(String),
    InvalidBody(String),
    ParseError(String),
}

#[derive(Debug)]
pub enum LockBillingWorkflowError {
    UserNotFound,
    ExistingRequestPending,
}

#[derive(Debug)]
pub enum GooglePlayWebhookError {
    InvalidToken,
    CannotRetrieveData,
    CannotDecodePubSubData(DecodeError),
    CannotRetrieveUserInfo,
    CannotRetrievePublicKey,
    CannotParseTime,
}

#[derive(Debug)]
pub enum AppStoreNotificationError {
    InvalidJWS,
}
