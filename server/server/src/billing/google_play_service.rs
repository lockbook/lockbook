use crate::billing::billing_service::GooglePlayWebhookError;
use crate::billing::google_play_model::{
    DeveloperNotification, NotificationType, PubSubNotification, SubscriptionNotification,
};
use crate::{ClientError, ServerError, ServerState};
use google_androidpublisher3::api::SubscriptionPurchase;
use google_androidpublisher3::hyper::body::Bytes;
use libsecp256k1::PublicKey;
use lockbook_shared::api::UnixTimeMillis;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::*;

pub fn get_public_key(
    state: &ServerState, sub_notif: &SubscriptionNotification, subscription: &SubscriptionPurchase,
    notification_type: &NotificationType,
) -> Result<PublicKey, ServerError<GooglePlayWebhookError>> {
    let account_id = &subscription
        .obfuscated_external_account_id
        .clone()
        .ok_or_else(|| {
            internal!("There should be an account id attached to a purchase: {:?}", sub_notif)
        })?;

    info!(
        ?notification_type,
        ?account_id,
        "Retrieved full subscription info for notification event",
    );

    let public_key: PublicKey = state
        .index_db
        .lock()?
        .google_play_ids
        .data()
        .get(account_id)
        .ok_or_else(|| {
            internal!("There is no public_key related to this account_id: {:?}", account_id)
        })?
        .0;

    Ok(public_key)
}

pub fn get_subscription_period_end(
    subscription: &SubscriptionPurchase, notification_type: &NotificationType,
    public_key: PublicKey,
) -> Result<UnixTimeMillis, ServerError<GooglePlayWebhookError>> {
    subscription
        .expiry_time_millis
        .as_ref()
        .ok_or_else(|| internal!("Cannot get expiration time of a recovered subscription. public_key {:?}, subscription notification type: {:?}", public_key, notification_type))?.parse().map_err(|e| internal!("Cannot parse millis into int: {:?}", e))
}

pub async fn verify_request_and_get_notification(
    server_state: &Arc<ServerState>, request_body: Bytes, query_parameters: HashMap<String, String>,
) -> Result<DeveloperNotification, ServerError<GooglePlayWebhookError>> {
    if !constant_time_eq::constant_time_eq(
        query_parameters
            .get("token")
            .ok_or(ClientError(GooglePlayWebhookError::InvalidToken))?
            .as_bytes(),
        server_state.config.billing.google.pubsub_token.as_bytes(),
    ) {
        return Err(ClientError(GooglePlayWebhookError::InvalidToken));
    }

    info!("Parsing pubsub notification and extracting the developer notification");

    let pubsub_notif = serde_json::from_slice::<PubSubNotification>(&request_body)?;
    let data = base64::decode(pubsub_notif.message.data)
        .map_err(|e| ClientError(GooglePlayWebhookError::CannotDecodePubSubData(e)))?;

    Ok(serde_json::from_slice::<DeveloperNotification>(&data)?)
}
