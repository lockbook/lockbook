use crate::billing::billing_model::StripeUserInfo;
use crate::document_service::DocumentService;
use crate::{ClientError, ServerError, ServerState, StripeWebhookError};
use google_androidpublisher3::hyper::body::Bytes;
use google_androidpublisher3::hyper::header::HeaderValue;
use lb_rs::model::api::{
    PaymentMethod, StripeAccountState, StripeAccountTier, UpgradeAccountStripeError,
};
use lb_rs::model::file_metadata::Owner;
use libsecp256k1::PublicKey;

use stripe::{Event, Invoice};
use tracing::*;
use uuid::Uuid;

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
    pub async fn create_subscription(
        &self, public_key: &PublicKey, account_tier: &StripeAccountTier,
        maybe_user_info: Option<StripeUserInfo>,
    ) -> Result<StripeUserInfo, ServerError<UpgradeAccountStripeError>> {
        let owner = Owner(*public_key);
        let (payment_method, price_id) = match account_tier {
            StripeAccountTier::Premium(payment_method) => {
                (payment_method, &self.config.billing.stripe.premium_price_id)
            }
        };

        let (customer_id, customer_name, payment_method_id, last_4) = match payment_method {
            PaymentMethod::NewCard { number, exp_year, exp_month, cvc } => {
                info!(?owner, "Creating a new card");
                let payment_method_resp = self
                    .stripe_client
                    .create_payment_method(number, *exp_month, *exp_year, cvc)
                    .await?;

                let last_4 = payment_method_resp
                    .card
                    .as_ref()
                    .ok_or_else::<ServerError<UpgradeAccountStripeError>, _>(|| {
                        internal!(
                            "Cannot retrieve card info from payment method response: {:?}",
                            payment_method_resp
                        )
                    })?
                    .last4
                    .clone();

                info!(?owner, ?last_4, "Created a new payment method");

                let (customer_id, customer_name) = match &maybe_user_info {
                    None => {
                        info!(?owner, "User has no customer_id; creating one with stripe now");

                        let customer_name = Uuid::new_v4();
                        let customer_resp = self
                            .stripe_client
                            .create_customer(
                                customer_name.to_string(),
                                payment_method_resp.id.clone(),
                            )
                            .await?;
                        let customer_id = customer_resp.id.to_string();

                        info!(?owner, ?customer_id, "Created customer_id");

                        self.index_db
                            .lock()
                            .await
                            .stripe_ids
                            .insert(customer_id, Owner(*public_key))?;

                        (customer_resp.id, customer_name)
                    }
                    Some(user_info) => {
                        let customer_id = &user_info.customer_id;

                        info!(?owner, ?customer_id, "User already has customer_id");

                        let customer_id = customer_id.parse()?;

                        info!(?owner, "Disabling old card since a new card has just been added");

                        self.stripe_client
                            .detach_payment_method_from_customer(
                                &user_info.payment_method_id.parse()?,
                            )
                            .await?;

                        (customer_id, user_info.customer_name)
                    }
                };

                info!(
                    ?owner,
                    "Creating a setup intent to confirm a users payment method for their subscription"
                );

                let setup_intent_resp = self
                    .stripe_client
                    .create_setup_intent(customer_id.clone(), payment_method_resp.id.clone())
                    .await?;

                let setup_intent = setup_intent_resp.id.to_string();
                info!(?owner, ?setup_intent, "Created a setup intent");

                (customer_id, customer_name, payment_method_resp.id.to_string(), last_4)
            }
            PaymentMethod::OldCard => {
                info!(?owner, "Using an old card stored on redis");

                let user_info = maybe_user_info
                    .ok_or(ClientError(UpgradeAccountStripeError::OldCardDoesNotExist))?;

                (
                    user_info.customer_id.parse()?,
                    user_info.customer_name,
                    user_info.payment_method_id,
                    user_info.last_4,
                )
            }
        };

        info!(?owner, "Successfully retrieved card");

        let subscription_resp = self
            .stripe_client
            .create_subscription(customer_id.clone(), &payment_method_id, price_id)
            .await?;

        let subscription_id = subscription_resp.id;
        info!(?owner, ?subscription_id, "Successfully created subscription");

        Ok(StripeUserInfo {
            customer_id: customer_id.to_string(),
            customer_name,
            price_id: price_id.to_string(),
            payment_method_id: payment_method_id.to_string(),
            last_4,
            subscription_id: subscription_id.to_string(),
            expiration_time: subscription_resp.current_period_end as u64,
            account_state: StripeAccountState::Ok,
        })
    }

    pub async fn get_public_key_from_invoice(
        &self, invoice: &Invoice,
    ) -> Result<PublicKey, ServerError<StripeWebhookError>> {
        let customer_id = match invoice.customer.as_ref().ok_or_else(|| {
            ClientError(StripeWebhookError::InvalidBody(
                "Cannot retrieve the customer_id".to_string(),
            ))
        })? {
            stripe::Expandable::Id(id) => id.to_string(),
            stripe::Expandable::Object(customer) => customer.id.to_string(),
        };

        let public_key = self
            .index_db
            .lock()
            .await
            .stripe_ids
            .get()
            .get(&customer_id)
            .copied()
            .ok_or_else(|| {
                internal!("There is no public_key related to this customer_id: {:?}", customer_id)
            })?;

        Ok(public_key.0)
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
