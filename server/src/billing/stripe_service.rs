use crate::billing::billing_model::{BillingPlatform, StripeUserInfo};
use crate::billing::billing_service::LockBillingWorkflowError;
use crate::document_service::DocumentService;
use crate::guard::ServerTx;
use crate::schema::Account;
use crate::{ClientError, RequestContext, ServerError, ServerState, StripeWebhookError};
use google_androidpublisher3::hyper::body::Bytes;
use google_androidpublisher3::hyper::header::HeaderValue;
use lb_rs::model::api::{
    StripeAccountState, StripeBillingPortalError, StripeBillingPortalRequest,
    StripeBillingPortalResponse, StripeCheckoutError, StripeCheckoutRequest,
    StripeCheckoutResponse, StripePlan,
};
use lb_rs::model::file_metadata::Owner;
use libsecp256k1::PublicKey;

use stripe::Event;
use tracing::*;
use uuid::Uuid;

use super::app_store_client::AppStoreClient;
use super::google_play_client::GooglePlayClient;
use super::stripe_client::StripeClient;

const CHECKOUT_SUCCESS_URL: &str = "https://lockbook.net/premium/thanks";
const CHECKOUT_CANCEL_URL: &str = "https://lockbook.net/premium/canceled";
const BILLING_PORTAL_RETURN_URL: &str = "https://lockbook.net";

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
        let account = self.lock_subscription_profile(&context.public_key).await?;
        let result = self
            .stripe_checkout_url(&context.public_key, &account, context.request.plan)
            .await;
        self.release_subscription_profile::<StripeCheckoutError>(context.public_key, account)
            .await?;
        Ok(StripeCheckoutResponse { url: result? })
    }

    async fn stripe_checkout_url(
        &self, public_key: &PublicKey, account: &Account, plan: StripePlan,
    ) -> Result<String, ServerError<StripeCheckoutError>> {
        if account.billing_info.is_premium() {
            return Err(ClientError(StripeCheckoutError::AlreadyPremium));
        }

        let customer_id = match self.stripe_customer_of(public_key, account).await {
            Some(customer_id) => customer_id.parse()?,
            None => {
                let customer = self
                    .stripe_client
                    .create_customer(Uuid::new_v4().to_string())
                    .await?;
                let mut db = self.index_db.lock().await;
                let mut tx = ServerTx::begin(&mut db)?;
                tx.stripe_ids
                    .insert(customer.id.to_string(), Owner(*public_key))?;
                tx.end()?;
                customer.id
            }
        };

        // a paid subscription the webhook hasn't delivered yet still counts
        let live = self
            .stripe_client
            .list_subscriptions(customer_id.clone())
            .await?
            .iter()
            .any(|s| {
                use stripe::SubscriptionStatus as Status;
                matches!(s.status, Status::Active | Status::Trialing | Status::PastDue)
            });
        if live {
            return Err(ClientError(StripeCheckoutError::AlreadyPremium));
        }
        self.stripe_client
            .expire_open_checkout_sessions(customer_id.clone())
            .await?;

        let stripe = &self.config.billing.stripe;
        let price_id = match plan {
            StripePlan::Monthly => &stripe.premium_price_id,
            StripePlan::Yearly => &stripe.premium_yearly_price_id,
        };

        let session = self
            .stripe_client
            .create_checkout_session(
                customer_id,
                price_id,
                CHECKOUT_SUCCESS_URL,
                CHECKOUT_CANCEL_URL,
            )
            .await?;

        session
            .url
            .ok_or_else(|| internal!("Stripe checkout session has no url: {:?}", session.id))
    }

    pub async fn stripe_billing_portal(
        &self, context: RequestContext<StripeBillingPortalRequest>,
    ) -> Result<StripeBillingPortalResponse, ServerError<StripeBillingPortalError>> {
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

        let session = self
            .stripe_client
            .create_billing_portal_session(customer_id.parse()?, BILLING_PORTAL_RETURN_URL)
            .await?;

        Ok(StripeBillingPortalResponse { url: session.url })
    }

    /// The Stripe customer that belongs to this account, if one was ever created.
    async fn stripe_customer_of(
        &self, public_key: &PublicKey, account: &Account,
    ) -> Option<String> {
        if let Some(BillingPlatform::Stripe(info)) = &account.billing_info.billing_platform {
            return Some(info.customer_id.clone());
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

    /// Makes the billing of the account behind `customer_id` match Stripe's subscriptions now.
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

        let mut account = loop {
            match self.lock_subscription_profile(&owner.0).await {
                Ok(account) => break account,
                Err(ClientError(LockBillingWorkflowError::ExistingRequestPending)) => {
                    tokio::time::sleep(self.config.billing.time_between_lock_attempts).await;
                }
                Err(ClientError(LockBillingWorkflowError::UserNotFound)) => {
                    warn!(?owner, "Ignoring stripe event for a deleted account");
                    return Ok(());
                }
                Err(err) => return Err(internal!("Cannot lock billing for {owner:?}: {err:?}")),
            }
        };

        let result = self.current_stripe_billing(customer_id).await;
        if let Ok(Some(info)) = &result {
            let other_platform_premium = account.billing_info.is_premium()
                && !matches!(
                    account.billing_info.billing_platform,
                    Some(BillingPlatform::Stripe(_))
                );
            if other_platform_premium {
                if info.account_state == StripeAccountState::Ok {
                    error!(
                        ?owner,
                        "Stripe subscription for an account premium on another platform"
                    );
                }
            } else {
                info!(?owner, state = ?info.account_state, "Syncing stripe billing");
                account.billing_info.billing_platform = Some(BillingPlatform::Stripe(info.clone()));
            }
        }
        self.release_subscription_profile::<StripeWebhookError>(owner.0, account)
            .await?;
        result.map(|_| ())
    }

    /// The customer's live subscription, or its latest if none is live.
    async fn current_stripe_billing(
        &self, customer_id: &str,
    ) -> Result<Option<StripeUserInfo>, ServerError<StripeWebhookError>> {
        use stripe::SubscriptionStatus as Status;
        // past_due keeps Premium while Stripe retries the payment
        let live = |s: &stripe::Subscription| {
            matches!(s.status, Status::Active | Status::Trialing | Status::PastDue)
        };

        let subscriptions = self
            .stripe_client
            .list_subscriptions(customer_id.parse()?)
            .await?;
        let Some(subscription) = subscriptions
            .iter()
            .filter(|s| live(s))
            .max_by_key(|s| s.current_period_end)
            .or_else(|| subscriptions.iter().max_by_key(|s| s.created))
        else {
            return Ok(None);
        };

        let customer_name = match &subscription.customer {
            stripe::Expandable::Object(customer) => customer
                .name
                .as_deref()
                .and_then(|name| name.parse().ok())
                .unwrap_or_else(Uuid::nil),
            stripe::Expandable::Id(_) => Uuid::nil(),
        };

        let (payment_method_id, last_4) = match &subscription.default_payment_method {
            Some(stripe::Expandable::Object(method)) => (
                method.id.to_string(),
                method
                    .card
                    .as_ref()
                    .map(|card| card.last4.clone())
                    .unwrap_or_default(),
            ),
            Some(stripe::Expandable::Id(id)) => (id.to_string(), String::new()),
            None => (String::new(), String::new()),
        };

        let price_id = subscription
            .items
            .data
            .first()
            .and_then(|item| item.price.as_ref())
            .map(|price| price.id.to_string())
            .unwrap_or_default();

        Ok(Some(StripeUserInfo {
            customer_id: customer_id.to_string(),
            customer_name,
            price_id,
            payment_method_id,
            last_4,
            subscription_id: subscription.id.to_string(),
            // Stripe counts seconds; every client reads milliseconds
            expiration_time: subscription.current_period_end as u64 * 1000,
            account_state: if live(subscription) {
                StripeAccountState::Ok
            } else {
                StripeAccountState::Canceled
            },
        }))
    }

    pub fn verify_request_and_get_event(
        &self, request_body: &Bytes, stripe_sig: HeaderValue,
    ) -> Result<Event, ServerError<StripeWebhookError>> {
        let payload = std::str::from_utf8(request_body).map_err(|e| {
            ClientError(StripeWebhookError::InvalidBody(format!("Cannot get body as str: {e:?}")))
        })?;

        let sig = stripe_sig.to_str().map_err(|e| {
            ClientError(StripeWebhookError::InvalidHeader(format!(
                "Cannot get header as str: {e:?}"
            )))
        })?;

        info!("Verifying a stripe webhook request");

        Ok(stripe::Webhook::construct_event(
            payload,
            sig,
            &self.config.billing.stripe.signing_secret,
        )?)
    }
}
