use crate::billing::billing_model::{BillingPlatform, StripeUserInfo};
use crate::billing::billing_service::LockBillingWorkflowError;
use crate::document_service::DocumentService;
use crate::guard::ServerTx;
use crate::schema::Account;
use crate::{ClientError, RequestContext, ServerError, ServerState, StripeWebhookError};
use constant_time_eq::constant_time_eq;
use google_androidpublisher3::hyper::body::Bytes;
use google_androidpublisher3::hyper::header::HeaderValue;
use hmac::{Hmac, Mac};
use lb_rs::model::api::{
    AdminStripeAccountCheck, AdminStripeAccountStatus, AdminStripeSnapshot, AdminSyncStripeError,
    AdminSyncStripeRequest, AdminSyncStripeResponse, StripeAccountState, StripeBillingPortalError,
    StripeBillingPortalRequest, StripeBillingPortalResponse, StripeCheckoutError,
    StripeCheckoutRequest, StripeCheckoutResponse,
};
use lb_rs::model::file_metadata::Owner;
use libsecp256k1::PublicKey;
use sha2::Sha256;
use std::collections::HashMap;
use std::time::Duration;
use tracing::*;
use uuid::Uuid;

use super::app_store_client::AppStoreClient;
use super::google_play_client::GooglePlayClient;
use super::stripe_client::{StripeClient, StripeSubscription};

const CHECKOUT_SUCCESS_URL: &str = "https://lockbook.net/checkout/thanks/";
const CHECKOUT_CANCEL_URL: &str = "https://lockbook.net/checkout/canceled/";
const BILLING_PORTAL_RETURN_URL: &str = "https://lockbook.net";
const WEBHOOK_TOLERANCE_SECS: u64 = 300;
const MAX_LOCK_WAIT: Duration = Duration::from_secs(5);
pub(crate) const MIN_LOCK_RETRY_SLEEP: Duration = Duration::from_millis(50);

type HmacSha256 = Hmac<Sha256>;

impl<S, A, G, D> ServerState<S, A, G, D>
where
    S: StripeClient,
    A: AppStoreClient,
    G: GooglePlayClient,
    D: DocumentService,
{
    pub async fn stripe_checkout(
        &self, context: RequestContext<StripeCheckoutRequest>,
    ) -> Result<StripeCheckoutResponse, ServerError<StripeCheckoutError>> {
        if self.is_sub_account(&context.public_key).await {
            return Err(ClientError(StripeCheckoutError::NotPermissioned));
        }

        let mut account = self.lock_subscription_profile(&context.public_key).await?;
        let result = self
            .stripe_checkout_url(&context.public_key, &mut account)
            .await;
        self.release_subscription_profile::<StripeCheckoutError>(context.public_key, account)
            .await?;
        Ok(StripeCheckoutResponse { url: result? })
    }

    async fn stripe_checkout_url(
        &self, public_key: &PublicKey, account: &mut Account,
    ) -> Result<String, ServerError<StripeCheckoutError>> {
        if account.billing_info.is_premium() {
            return Err(ClientError(StripeCheckoutError::AlreadyPremium));
        }

        let owner = Owner(*public_key);
        let (customer_id, is_new) = match self.stripe_customer_of(public_key, account).await {
            Some(customer_id) => {
                self.record_stripe_customer::<StripeCheckoutError>(&customer_id, owner)
                    .await?;
                (customer_id, false)
            }
            None => {
                let customer_id = self.stripe_client.create_customer().await?;
                self.record_stripe_customer::<StripeCheckoutError>(&customer_id, owner)
                    .await?;
                (customer_id, true)
            }
        };

        if !is_new {
            // A paid subscription whose webhook hasn't arrived yet still counts — heal local DB
            // now so clients polling after AlreadyPremium immediately see Premium.
            let subscriptions = self.stripe_client.list_subscriptions(&customer_id).await?;
            if let Some(info) = billing_from_subscriptions(&customer_id, &subscriptions) {
                apply_stripe_sync(owner, &customer_id, account, Some(&info));
                return Err(ClientError(StripeCheckoutError::AlreadyPremium));
            }

            self.stripe_client
                .expire_open_checkout_sessions(&customer_id)
                .await?;
        }

        let price_id = &self.config.billing.stripe.premium_price_id;
        Ok(self
            .stripe_client
            .create_checkout_session(
                &customer_id,
                price_id,
                CHECKOUT_SUCCESS_URL,
                CHECKOUT_CANCEL_URL,
            )
            .await?)
    }

    pub async fn stripe_billing_portal(
        &self, context: RequestContext<StripeBillingPortalRequest>,
    ) -> Result<StripeBillingPortalResponse, ServerError<StripeBillingPortalError>> {
        if self.is_sub_account(&context.public_key).await {
            return Err(ClientError(StripeBillingPortalError::NotPermissioned));
        }

        let account = self
            .index_db
            .lock()
            .await
            .schema
            .accounts
            .get(&Owner(context.public_key))
            .cloned()
            .ok_or(ClientError(StripeBillingPortalError::UserNotFound))?;

        let customer_id = self
            .stripe_customer_of(&context.public_key, &account)
            .await
            .ok_or(ClientError(StripeBillingPortalError::NotStripeCustomer))?;

        let url = self
            .stripe_client
            .create_billing_portal_session(&customer_id, BILLING_PORTAL_RETURN_URL)
            .await?;

        Ok(StripeBillingPortalResponse { url })
    }

    /// Best-effort immediate cancellation of any live Stripe subscriptions when a Lockbook account
    /// is deleted. Checks Stripe live by `customer_id` first (catching pre-webhook checkouts and
    /// multiple live subs), falling back to any cached `subscription_id`.
    pub async fn cancel_stripe_on_account_delete(
        &self, customer_id: Option<&str>, fallback_subscription_id: Option<&str>,
    ) {
        if let Some(customer_id) = customer_id {
            match self.stripe_client.list_subscriptions(customer_id).await {
                Ok(subs) => {
                    for sub in subs.into_iter().filter(StripeSubscription::is_live) {
                        if let Err(err) = self
                            .stripe_client
                            .cancel_subscription_immediately(&sub.id)
                            .await
                        {
                            warn!(
                                subscription_id = sub.id.as_str(),
                                ?err,
                                "Failed to cancel Stripe subscription on account delete"
                            );
                        }
                    }
                    return;
                }
                Err(err) => {
                    warn!(
                        customer_id,
                        ?err,
                        "Failed to list Stripe subscriptions on account delete"
                    );
                }
            }
        }
        if let Some(subscription_id) = fallback_subscription_id {
            if let Err(err) = self
                .stripe_client
                .cancel_subscription_immediately(subscription_id)
                .await
            {
                warn!(
                    subscription_id,
                    ?err,
                    "Failed to cancel cached Stripe subscription on account delete"
                );
            }
        }
    }

    async fn record_stripe_customer<T: std::fmt::Debug>(
        &self, customer_id: &str, owner: Owner,
    ) -> Result<(), ServerError<T>> {
        let mut db = self.index_db.lock().await;
        if db.schema.stripe_ids.get(customer_id) == Some(&owner) {
            return Ok(());
        }
        let mut tx = ServerTx::begin(&mut db)?;
        tx.stripe_ids.insert(customer_id.to_string(), owner)?;
        tx.end()?;
        Ok(())
    }

    /// The Stripe customer that belongs to this account, if one was ever created.
    async fn stripe_customer_of(
        &self, public_key: &PublicKey, account: &Account,
    ) -> Option<String> {
        if let Some(BillingPlatform::Stripe(info)) = &account.billing_info.billing_platform {
            if !info.customer_id.is_empty() {
                return Some(info.customer_id.clone());
            }
        }
        let owner = Owner(*public_key);
        self.index_db
            .lock()
            .await
            .schema
            .stripe_ids
            .iter()
            .find(|(_, o)| **o == owner)
            .map(|(customer_id, _)| customer_id.clone())
    }

    /// Makes the billing of the account behind `customer_id` match Stripe's live subscriptions now.
    /// Holding the account's billing lock across the fetch applies concurrent events in order.
    pub async fn sync_stripe_customer(
        &self, customer_id: &str,
    ) -> Result<(), ServerError<StripeWebhookError>> {
        let owner = self
            .index_db
            .lock()
            .await
            .schema
            .stripe_ids
            .get(customer_id)
            .copied();
        let Some(owner) = owner else {
            warn!(?customer_id, "Ignoring stripe event for a customer with no account");
            return Ok(());
        };

        let Some(mut account) = self
            .lock_for_stripe_sync(&owner)
            .await
            .map_err(|msg| internal!("{msg}"))?
        else {
            warn!(?owner, "Ignoring stripe event for a deleted account");
            return Ok(());
        };

        let result = self.current_stripe_billing(customer_id).await;
        if let Ok(live_info) = &result {
            apply_stripe_sync(owner, customer_id, &mut account, live_info.as_ref());
        }
        self.release_subscription_profile::<StripeWebhookError>(owner.0, account)
            .await?;
        result.map(|_| ())
    }

    async fn lock_for_stripe_sync(&self, owner: &Owner) -> Result<Option<Account>, String> {
        let deadline = tokio::time::Instant::now() + MAX_LOCK_WAIT;
        let sleep = self
            .config
            .billing
            .time_between_lock_attempts
            .max(MIN_LOCK_RETRY_SLEEP);
        loop {
            match self.lock_subscription_profile(&owner.0).await {
                Ok(account) => return Ok(Some(account)),
                Err(ClientError(LockBillingWorkflowError::ExistingRequestPending)) => {
                    if tokio::time::Instant::now() >= deadline {
                        return Err(format!(
                            "Timed out waiting for billing lock for {owner:?}; letting Stripe retry"
                        ));
                    }
                    tokio::time::sleep(sleep).await;
                }
                Err(ClientError(LockBillingWorkflowError::UserNotFound)) => return Ok(None),
                Err(err) => return Err(format!("Cannot lock billing for {owner:?}: {err:?}")),
            }
        }
    }

    /// Admin check/reconcile of cached Stripe billing against live Stripe.
    pub async fn admin_sync_stripe(
        &self, context: RequestContext<AdminSyncStripeRequest>,
    ) -> Result<AdminSyncStripeResponse, ServerError<AdminSyncStripeError>> {
        {
            let db = self.index_db.lock().await;
            if !Self::is_admin::<AdminSyncStripeError>(
                &db.schema,
                &context.public_key,
                &self.config.admin.admins,
            )? {
                return Err(ClientError(AdminSyncStripeError::NotPermissioned));
            }
        }

        let targets: Vec<(String, Owner, String)> = match &context.request.username {
            Some(username) => {
                let (owner, account) = {
                    let db = self.index_db.lock().await;
                    let owner = *db
                        .schema
                        .usernames
                        .get(username)
                        .ok_or(ClientError(AdminSyncStripeError::UserNotFound))?;
                    let account = db
                        .schema
                        .accounts
                        .get(&owner)
                        .cloned()
                        .ok_or(ClientError(AdminSyncStripeError::UserNotFound))?;
                    (owner, account)
                };
                let customer_id = self
                    .stripe_customer_of(&owner.0, &account)
                    .await
                    .ok_or(ClientError(AdminSyncStripeError::NotStripeCustomer))?;
                vec![(account.username, owner, customer_id)]
            }
            None => {
                let db = self.index_db.lock().await;
                let is_live_account = |owner: &Owner| {
                    db.schema
                        .owned_files
                        .get(owner)
                        .is_some_and(|ids| !ids.is_empty())
                };
                let mut by_owner: HashMap<Owner, (String, String)> = HashMap::new();
                for (owner, account) in db.schema.accounts.iter() {
                    if !is_live_account(owner) {
                        continue;
                    }
                    if let Some(BillingPlatform::Stripe(info)) =
                        &account.billing_info.billing_platform
                    {
                        if !info.customer_id.is_empty() {
                            by_owner.insert(
                                *owner,
                                (account.username.clone(), info.customer_id.clone()),
                            );
                        }
                    }
                }
                for (customer_id, owner) in db.schema.stripe_ids.iter() {
                    if !is_live_account(owner) {
                        continue;
                    }
                    if let Some(account) = db.schema.accounts.get(owner) {
                        by_owner
                            .entry(*owner)
                            .or_insert_with(|| (account.username.clone(), customer_id.clone()));
                    }
                }
                let mut list: Vec<(String, Owner, String)> = by_owner
                    .into_iter()
                    .map(|(owner, (username, cus_id))| (username, owner, cus_id))
                    .collect();
                list.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.2.cmp(&b.2)));
                list
            }
        };

        let mut results = Vec::with_capacity(targets.len());
        for (username, owner, customer_id) in targets {
            let status = self
                .check_or_sync_stripe_account(owner, &customer_id, context.request.apply)
                .await;
            results.push(AdminStripeAccountCheck { username, customer_id, status });
        }

        Ok(AdminSyncStripeResponse { results })
    }

    async fn check_or_sync_stripe_account(
        &self, owner: Owner, customer_id: &str, apply: bool,
    ) -> AdminStripeAccountStatus {
        if !apply {
            let account = self
                .index_db
                .lock()
                .await
                .schema
                .accounts
                .get(&owner)
                .cloned();
            let Some(account) = account else {
                return AdminStripeAccountStatus::Error("Account no longer exists".into());
            };
            let live_info = match self.current_stripe_billing(customer_id).await {
                Ok(info) => info,
                Err(err) => return AdminStripeAccountStatus::Error(format!("{err:?}")),
            };
            return compare_stripe_state(&account, customer_id, live_info.as_ref());
        }

        let mut account = match self.lock_for_stripe_sync(&owner).await {
            Ok(Some(account)) => account,
            Ok(None) => return AdminStripeAccountStatus::Error("Account no longer exists".into()),
            Err(msg) => return AdminStripeAccountStatus::Error(msg),
        };

        let live_result = self.current_stripe_billing(customer_id).await;
        let status = match live_result {
            Err(err) => AdminStripeAccountStatus::Error(format!("{err:?}")),
            Ok(live_info) => {
                let status = compare_stripe_state(&account, customer_id, live_info.as_ref());
                if matches!(status, AdminStripeAccountStatus::Drifted { .. }) {
                    apply_stripe_sync(owner, customer_id, &mut account, live_info.as_ref());
                }
                status
            }
        };

        if let Err(err) = self
            .release_subscription_profile::<AdminSyncStripeError>(owner.0, account)
            .await
        {
            return AdminStripeAccountStatus::Error(format!(
                "Failed to save account after Stripe sync: {err:?}"
            ));
        }

        // Backfill stripe_ids even when InSync so future webhooks always find this customer.
        if !matches!(status, AdminStripeAccountStatus::Error(_)) {
            if let Err(err) = self
                .record_stripe_customer::<AdminSyncStripeError>(customer_id, owner)
                .await
            {
                return AdminStripeAccountStatus::Error(format!(
                    "Failed to record stripe_ids mapping: {err:?}"
                ));
            }
        }

        status
    }

    /// The customer's live Stripe subscription, if any.
    async fn current_stripe_billing(
        &self, customer_id: &str,
    ) -> Result<Option<StripeUserInfo>, ServerError<StripeWebhookError>> {
        let subscriptions = self.stripe_client.list_subscriptions(customer_id).await?;
        Ok(billing_from_subscriptions(customer_id, &subscriptions))
    }

    /// Verifies `Stripe-Signature` (HMAC-SHA256 + timestamp tolerance) and returns the customer ID
    /// for events that can change subscription state. Parses raw JSON so Stripe webhook API
    /// version changes cannot break deserialization.
    pub fn verify_webhook_and_get_customer(
        &self, request_body: &Bytes, stripe_sig: HeaderValue,
    ) -> Result<Option<String>, ServerError<StripeWebhookError>> {
        let payload = std::str::from_utf8(request_body).map_err(|e| {
            ClientError(StripeWebhookError::InvalidBody(format!("Cannot get body as str: {e:?}")))
        })?;
        let sig = stripe_sig.to_str().map_err(|e| {
            ClientError(StripeWebhookError::InvalidHeader(format!(
                "Cannot get header as str: {e:?}"
            )))
        })?;

        info!("Verifying a stripe webhook request");
        let now_secs = self.clock.now().0 / 1000;
        verify_stripe_signature(
            payload,
            sig,
            &self.config.billing.stripe.signing_secret,
            now_secs,
        )?;

        let value: serde_json::Value = serde_json::from_str(payload).map_err(|e| {
            ClientError(StripeWebhookError::ParseError(format!("Invalid webhook JSON: {e:?}")))
        })?;

        let event_type = value.get("type").and_then(|v| v.as_str()).ok_or_else(|| {
            ClientError(StripeWebhookError::InvalidBody("Missing webhook event type".into()))
        })?;

        let relevant = matches!(
            event_type,
            "checkout.session.completed"
                | "customer.subscription.created"
                | "customer.subscription.updated"
                | "customer.subscription.deleted"
                | "invoice.paid"
                | "invoice.payment_failed"
        );
        if !relevant {
            debug!(?event_type, "Ignoring stripe event");
            return Ok(None);
        }

        let customer = value
            .pointer("/data/object/customer")
            .and_then(|v| {
                v.as_str()
                    .or_else(|| v.get("id").and_then(|id| id.as_str()))
            })
            .map(ToOwned::to_owned);
        if customer.is_none() {
            warn!(?event_type, "Relevant stripe webhook event has no customer id");
        }
        Ok(customer)
    }
}

fn verify_stripe_signature(
    payload: &str, header: &str, secret: &str, now_secs: i64,
) -> Result<(), ServerError<StripeWebhookError>> {
    if secret.is_empty() {
        return Err(internal!("Stripe webhook signing secret is empty"));
    }

    let mut timestamp: Option<i64> = None;
    let mut signatures: Vec<&str> = Vec::new();
    for part in header.split(',') {
        let Some((key, value)) = part.trim().split_once('=') else {
            continue;
        };
        match key.trim() {
            "t" => {
                timestamp = value.trim().parse::<i64>().ok();
            }
            "v1" => signatures.push(value.trim()),
            _ => {}
        }
    }

    let t = timestamp.ok_or_else(|| {
        ClientError(StripeWebhookError::InvalidHeader("Missing or invalid timestamp".into()))
    })?;
    if signatures.is_empty() {
        return Err(ClientError(StripeWebhookError::InvalidHeader("Missing v1 signature".into())));
    }
    if now_secs.abs_diff(t) > WEBHOOK_TOLERANCE_SECS {
        return Err(ClientError(StripeWebhookError::InvalidHeader(format!(
            "Timestamp outside tolerance: {t}"
        ))));
    }

    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .map_err(|e| internal!("Invalid Stripe webhook signing secret: {e:?}"))?;
    mac.update(format!("{t}.{payload}").as_bytes());
    let expected = mac.finalize().into_bytes();

    let matched = signatures.into_iter().any(|sig_hex| {
        hex::decode(sig_hex).is_ok_and(|decoded| constant_time_eq(&expected, &decoded))
    });
    if !matched {
        return Err(ClientError(StripeWebhookError::InvalidHeader("Bad signature".into())));
    }
    Ok(())
}

fn is_other_platform_premium(account: &Account) -> bool {
    account.billing_info.is_premium()
        && !matches!(account.billing_info.billing_platform, Some(BillingPlatform::Stripe(_)))
}

fn stripe_snapshot(info: &StripeUserInfo) -> AdminStripeSnapshot {
    AdminStripeSnapshot {
        subscription_id: info.subscription_id.clone(),
        last_4: info.last_4.clone(),
        expiration_time: info.expiration_time,
        account_state: info.account_state.clone(),
    }
}

fn compare_stripe_state(
    account: &Account, customer_id: &str, live_info: Option<&StripeUserInfo>,
) -> AdminStripeAccountStatus {
    let stripe = live_info.map(stripe_snapshot);
    if is_other_platform_premium(account) {
        return AdminStripeAccountStatus::OtherPlatformPremium { stripe };
    }
    // A lapsed Stripe customer must never touch a recoverable mobile record (e.g. OnHold,
    // FailedToRenew) or a newer Stripe customer on the same Lockbook account.
    if live_info.is_none() {
        match &account.billing_info.billing_platform {
            Some(BillingPlatform::AppStore(_) | BillingPlatform::GooglePlay(_)) => {
                return AdminStripeAccountStatus::InSync;
            }
            Some(BillingPlatform::Stripe(cur)) if cur.customer_id != customer_id => {
                return AdminStripeAccountStatus::InSync;
            }
            _ => {}
        }
    }
    let cached = match &account.billing_info.billing_platform {
        Some(BillingPlatform::Stripe(info)) if info.customer_id == customer_id => {
            Some(stripe_snapshot(info))
        }
        _ => None,
    };
    if cached == stripe {
        AdminStripeAccountStatus::InSync
    } else {
        AdminStripeAccountStatus::Drifted { cached, stripe }
    }
}

/// Applies Stripe's live state for `customer_id` to `account`.
///
/// - A live subscription writes `Some(BillingPlatform::Stripe(Ok))` unless another platform is
///   already Premium.
/// - No live subscription clears `billing_platform` to `None`, **only** when `billing_platform`
///   currently points to `Stripe(customer_id)` — never clobbering recoverable mobile records
///   (`GooglePlay::OnHold`, `AppStore::FailedToRenew`) or a newer Stripe customer on this owner.
fn apply_stripe_sync(
    owner: Owner, customer_id: &str, account: &mut Account, live_info: Option<&StripeUserInfo>,
) {
    match live_info {
        Some(info) => {
            if is_other_platform_premium(account) {
                error!(?owner, "Stripe subscription for an account premium on another platform");
            } else {
                info!(?owner, state = ?info.account_state, "Syncing live stripe billing");
                account.billing_info.billing_platform = Some(BillingPlatform::Stripe(info.clone()));
            }
        }
        None => {
            if matches!(
                &account.billing_info.billing_platform,
                Some(BillingPlatform::Stripe(cur)) if cur.customer_id == customer_id
            ) {
                info!(?owner, customer_id, "Clearing ended stripe billing");
                account.billing_info.billing_platform = None;
            }
        }
    }
}

fn billing_from_subscriptions(
    customer_id: &str, subscriptions: &[StripeSubscription],
) -> Option<StripeUserInfo> {
    let subscription = subscriptions
        .iter()
        .filter(|s| s.is_live())
        .max_by_key(|s| s.current_period_end)?;

    Some(StripeUserInfo {
        customer_id: customer_id.to_string(),
        customer_name: Uuid::nil(),
        price_id: subscription.price_id.clone(),
        payment_method_id: subscription.payment_method_id.clone(),
        last_4: subscription.card_last4.clone(),
        subscription_id: subscription.id.clone(),
        // Stripe counts seconds; every client reads milliseconds.
        expiration_time: subscription.current_period_end.saturating_mul(1000),
        account_state: StripeAccountState::Ok,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::billing::billing_model::{AppStoreUserInfo, GooglePlayUserInfo};
    use crate::router_service::retired_card_upgrade;
    use lb_rs::model::api::{
        AppStoreAccountState, ErrorWrapper, FREE_TIER_USAGE_SIZE, GooglePlayAccountState,
        PREMIUM_TIER_USAGE_SIZE,
    };
    use lb_rs::model::wire::{WIRE_FORMAT_HEADER, WireFormat};

    fn make_sub(
        id: &str, status: &str, current_period_end: u64, card_last4: &str,
    ) -> StripeSubscription {
        StripeSubscription {
            id: id.into(),
            status: status.into(),
            current_period_end,
            created: 1_700_000_000,
            cancel_at_period_end: false,
            cancel_at: None,
            price_id: "price_monthly".into(),
            payment_method_id: "pm_1".into(),
            card_last4: card_last4.into(),
        }
    }

    fn sign(secret: &str, t: i64, payload: &str) -> String {
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(format!("{t}.{payload}").as_bytes());
        hex::encode(mac.finalize().into_bytes())
    }

    #[test]
    fn verify_stripe_signature_checks_hmac_rotation_and_timestamps() {
        let secret = "whsec_test_secret";
        let now = 1_700_000_000;
        let payload =
            r#"{"type":"customer.subscription.updated","data":{"object":{"customer":"cus_1"}}}"#;
        let sig = sign(secret, now, payload);

        // Valid single signature and dual-v1 secret rotation header.
        verify_stripe_signature(payload, &format!("t={now},v1={sig}"), secret, now).unwrap();
        verify_stripe_signature(payload, &format!("t={now},v1=deadbeef,v1={sig}"), secret, now)
            .unwrap();

        // Bad sig, stale/future timestamps, i64::MIN overflow, missing fields rejected as 400.
        for bad in [
            format!("t={now},v1=deadbeef"),
            format!("t={},v1={}", now - 600, sign(secret, now - 600, payload)),
            format!("t={},v1={}", now + 600, sign(secret, now + 600, payload)),
            "t=-9223372036854775808,v1=00".into(),
            format!("v1={sig}"),
            format!("t={now}"),
        ] {
            assert!(matches!(
                verify_stripe_signature(payload, &bad, secret, now),
                Err(ClientError(StripeWebhookError::InvalidHeader(_)))
            ));
        }

        // Empty signing secret fails closed as 500.
        assert!(matches!(
            verify_stripe_signature(payload, &format!("t={now},v1={sig}"), "", now),
            Err(ServerError::InternalError(_))
        ));
    }

    #[test]
    fn billing_from_subscriptions_and_apply_sync_invariants() {
        let owner = Owner(PublicKey::from_secret_key(&lb_rs::model::pubkey::generate_key()));
        let mut account = Account { username: "alice".into(), billing_info: Default::default() };

        // Live active (empty Link last_4) -> Ok with milliseconds expiration.
        let live =
            billing_from_subscriptions("cus_1", &[make_sub("sub_1", "active", 1_800_000_000, "")])
                .unwrap();
        assert_eq!(live.expiration_time, 1_800_000_000_000);
        assert_eq!(live.last_4, "");
        assert_eq!(live.account_state, StripeAccountState::Ok);
        apply_stripe_sync(owner, "cus_1", &mut account, Some(&live));
        assert_eq!(account.billing_info.data_cap(), PREMIUM_TIER_USAGE_SIZE);

        // past_due stays Premium; re-subscribing picks live sub_2 over old canceled sub_1.
        let past_due = billing_from_subscriptions(
            "cus_1",
            &[make_sub("sub_1", "past_due", 1_800_000_000, "4242")],
        )
        .unwrap();
        assert_eq!(past_due.account_state, StripeAccountState::Ok);
        let resub = billing_from_subscriptions(
            "cus_1",
            &[
                make_sub("sub_1", "canceled", 1_800_000_000, "4242"),
                make_sub("sub_2", "active", 1_900_000_000, "1111"),
            ],
        )
        .unwrap();
        assert_eq!(resub.subscription_id, "sub_2");
        assert_eq!(resub.last_4, "1111");

        // Ended sub -> None, clears matching Stripe customer to Free (None).
        assert!(
            billing_from_subscriptions(
                "cus_1",
                &[make_sub("sub_1", "canceled", 1_800_000_000, "4242")]
            )
            .is_none()
        );
        apply_stripe_sync(owner, "cus_1", &mut account, None);
        assert!(account.billing_info.billing_platform.is_none());
        assert_eq!(account.billing_info.data_cap(), FREE_TIER_USAGE_SIZE);

        // Lapsed old customer never wipes a newer active Stripe customer on the same owner.
        let newer = billing_from_subscriptions(
            "cus_new",
            &[make_sub("sub_new", "active", 1_900_000_000, "9999")],
        )
        .unwrap();
        apply_stripe_sync(owner, "cus_new", &mut account, Some(&newer));
        assert_eq!(
            compare_stripe_state(&account, "cus_old", None),
            AdminStripeAccountStatus::InSync
        );
        apply_stripe_sync(owner, "cus_old", &mut account, None);
        assert!(matches!(
            &account.billing_info.billing_platform,
            Some(BillingPlatform::Stripe(s)) if s.customer_id == "cus_new"
        ));

        // Lapsed Stripe never wipes recoverable GooglePlay(OnHold); live Stripe never wipes active AppStore(Ok).
        account.billing_info.billing_platform =
            Some(BillingPlatform::GooglePlay(GooglePlayUserInfo {
                purchase_token: "tok".into(),
                subscription_product_id: "prod".into(),
                subscription_offer_id: "offer".into(),
                expiration_time: 1_900_000_000_000,
                account_state: GooglePlayAccountState::OnHold,
            }));
        assert_eq!(
            compare_stripe_state(&account, "cus_old", None),
            AdminStripeAccountStatus::InSync
        );
        apply_stripe_sync(owner, "cus_old", &mut account, None);
        assert!(matches!(
            &account.billing_info.billing_platform,
            Some(BillingPlatform::GooglePlay(g)) if g.account_state == GooglePlayAccountState::OnHold
        ));

        account.billing_info.billing_platform = Some(BillingPlatform::AppStore(AppStoreUserInfo {
            account_token: "tok".into(),
            original_transaction_id: "tx1".into(),
            subscription_product_id: "prod".into(),
            expiration_time: 1_900_000_000_000,
            account_state: AppStoreAccountState::Ok,
        }));
        assert!(matches!(
            compare_stripe_state(&account, "cus_old", Some(&newer)),
            AdminStripeAccountStatus::OtherPlatformPremium { .. }
        ));
        apply_stripe_sync(owner, "cus_old", &mut account, Some(&newer));
        assert!(matches!(
            &account.billing_info.billing_platform,
            Some(BillingPlatform::AppStore(_))
        ));
    }

    #[tokio::test]
    async fn retired_card_upgrade_returns_client_update_required_in_both_wire_formats() {
        let filter = retired_card_upgrade();

        for (header, fmt) in [(None, WireFormat::Json), (Some("bincode"), WireFormat::Bincode)] {
            let mut req = warp::test::request()
                .method("POST")
                .path("/upgrade-account-stripe");
            if let Some(h) = header {
                req = req.header(WIRE_FORMAT_HEADER, h);
            }
            let resp = req.reply(&filter).await;
            assert_eq!(resp.status(), warp::http::StatusCode::BAD_REQUEST);
            let parsed: Result<(), ErrorWrapper<()>> = fmt.deserialize(resp.body()).unwrap();
            assert_eq!(parsed, Err(ErrorWrapper::ClientUpdateRequired));
        }
    }
}
