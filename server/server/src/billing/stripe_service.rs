use crate::billing::billing_model::StripeUserInfo;
use crate::billing::stripe_client;
use crate::{keys, StripeWebhookError};
use crate::{ClientError, ServerError, ServerState};
use google_androidpublisher3::hyper::body::Bytes;
use google_androidpublisher3::hyper::header::HeaderValue;
use libsecp256k1::PublicKey;
use lockbook_models::api::{PaymentMethod, StripeAccountTier, UpgradeAccountStripeError};
use lockbook_models::file_metadata::Owner;
use log::info;
use std::ops::Deref;
use std::sync::Arc;
use stripe::{Invoice, WebhookEvent};
use uuid::Uuid;

pub async fn create_subscription(
    server_state: &ServerState, public_key: &PublicKey, account_tier: &StripeAccountTier,
    maybe_user_info: Option<StripeUserInfo>,
) -> Result<StripeUserInfo, ServerError<UpgradeAccountStripeError>> {
    let (payment_method, price_id) = match account_tier {
        StripeAccountTier::Premium(payment_method) => {
            (payment_method, &server_state.config.billing.stripe.premium_price_id)
        }
    };

    let (customer_id, customer_name, payment_method_id, last_4) = match payment_method {
        PaymentMethod::NewCard { number, exp_year, exp_month, cvc } => {
            info!("Creating a new card for public_key: {:?}", public_key);
            let payment_method_resp = stripe_client::create_payment_method(
                &server_state.stripe_client,
                number,
                *exp_month,
                *exp_year,
                cvc,
            )
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

            info!("Created a new payment method. last_4: {}, public_key: {:?}", last_4, public_key);

            let (customer_id, customer_name) = match &maybe_user_info {
                None => {
                    info!(
                        "User has no customer_id. Creating one with stripe now. public_key: {}",
                        keys::stringify_public_key(public_key)
                    );

                    let customer_name = Uuid::new_v4();
                    let customer_resp = stripe_client::create_customer(
                        &server_state.stripe_client,
                        &customer_name.to_string(),
                        payment_method_resp.id.clone(),
                    )
                    .await?;
                    let customer_id = customer_resp.id.to_string();

                    info!("Created customer_id: {}. public_key: {:?}", customer_id, public_key);

                    server_state
                        .index_db
                        .stripe_ids
                        .insert(customer_id, Owner(*public_key))?;

                    (customer_resp.id, customer_name)
                }
                Some(user_info) => {
                    info!(
                        "User already has customer_id: {} public_key: {:?}",
                        user_info.customer_id, public_key
                    );

                    let customer_id = user_info.customer_id.parse()?;

                    info!(
                        "Disabling old card since a new card has just been added. public_key: {:?}",
                        public_key
                    );

                    stripe_client::detach_payment_method_from_customer(
                        &server_state.stripe_client,
                        &user_info.payment_method_id.parse()?,
                    )
                    .await?;

                    (customer_id, user_info.customer_name)
                }
            };

            info!(
                "Creating a setup intent to confirm a users payment method for their subscription. public_key: {:?}",
                public_key
            );

            let setup_intent_resp = stripe_client::create_setup_intent(
                &server_state.stripe_client,
                customer_id.clone(),
                payment_method_resp.id.clone(),
            )
            .await?;

            info!(
                "Created a setup intent: {}, public_key: {:?}",
                setup_intent_resp.id.to_string(),
                public_key
            );

            (customer_id, customer_name, payment_method_resp.id.to_string(), last_4)
        }
        PaymentMethod::OldCard => {
            info!("Using an old card stored on redis for public_key: {:?}", public_key);

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

    info!("Successfully retrieved card for public_key: {:?}", public_key);

    let subscription_resp = stripe_client::create_subscription(
        &server_state.stripe_client,
        customer_id.clone(),
        &payment_method_id,
        price_id,
    )
    .await?;

    info!(
        "Successfully create subscription: {}, public_key: {:?}",
        subscription_resp.id, public_key
    );

    Ok(StripeUserInfo {
        customer_id: customer_id.to_string(),
        customer_name,
        price_id: price_id.to_string(),
        payment_method_id: payment_method_id.to_string(),
        last_4,
        subscription_id: subscription_resp.id.to_string(),
        expiration_time: subscription_resp.current_period_end as u64,
    })
}

pub fn get_public_key(
    server: &ServerState, invoice: &Invoice,
) -> Result<PublicKey, ServerError<StripeWebhookError>> {
    let customer_id = match invoice
        .customer
        .as_ref()
        .ok_or_else(|| {
            ClientError(StripeWebhookError::InvalidBody(
                "Cannot retrieve the customer_id.".to_string(),
            ))
        })?
        .deref()
    {
        stripe::Expandable::Id(id) => id.to_string(),
        stripe::Expandable::Object(customer) => customer.id.to_string(),
    };

    let public_key = server
        .index_db
        .stripe_ids
        .get(&customer_id)?
        .ok_or_else(|| {
            internal!("There is no public_key related to this customer_id: {:?}", customer_id)
        })?;

    Ok(public_key.0)
}

pub fn verify_request_and_get_event(
    server_state: &Arc<ServerState>, request_body: &Bytes, stripe_sig: HeaderValue,
) -> Result<WebhookEvent, ServerError<StripeWebhookError>> {
    let payload = std::str::from_utf8(request_body).map_err(|e| {
        ClientError(StripeWebhookError::InvalidBody(format!("Cannot get body as str: {:?}", e)))
    })?;

    let sig = stripe_sig.to_str().map_err(|e| {
        ClientError(StripeWebhookError::InvalidHeader(format!("Cannot get header as str: {:?}", e)))
    })?;

    info!("Verifying a stripe webhook request.");

    Ok(stripe::Webhook::construct_event(
        payload,
        sig,
        &server_state.config.billing.stripe.signing_secret,
    )?)
}
