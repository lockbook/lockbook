use jsonwebtoken::{Algorithm, DecodingKey, Validation};
use libsecp256k1::PublicKey;
use warp::hyper::body::Bytes;
use lockbook_shared::api::{UnixTimeMillis, UpgradeAccountAppStoreError};
use lockbook_shared::clock::get_time;
use crate::billing::app_store_model::{EncodedNotificationResponseBody, NotificationResponseBody, ReceiptInfo, TransactionInfo};
use crate::{ClientError, ServerError, ServerState};
use crate::billing::app_store_client;
use crate::billing::app_store_client::Claims;
use crate::billing::billing_service::AppStoreNotificationError;
use crate::config::AppleConfig;

pub fn get_public_key(
    state: &ServerState, trans: &TransactionInfo
) -> Result<PublicKey, ServerError<AppStoreNotificationError>> {
    let public_key: PublicKey = state
        .index_db
        .app_store_ids
        .get(&trans
            .app_account_token)?
        .ok_or_else(|| {
            internal!("There is no public_key related to this app_account_token: {:?}", trans)
        })?
        .0;

    Ok(public_key)
}

pub async fn verify_receipt(client: &reqwest::Client, config: &AppleConfig, encoded_receipt: &str, app_account_token: &str, original_transaction_id: &str) -> Result<UnixTimeMillis, ServerError<UpgradeAccountAppStoreError>> {
    let receipt = app_store_client::verify_receipt(client, config, encoded_receipt).await?;

    let latest_receipt: ReceiptInfo = serde_json::from_slice(base64::decode(receipt.encoded_latest_receipt).map_err(|_| ClientError(UpgradeAccountAppStoreError::InvalidAuthDetails))?.as_slice())?;

    if latest_receipt.app_account_token != app_account_token || latest_receipt.original_transaction_id != original_transaction_id || latest_receipt.expires_date_ms < get_time().0 {
        return Err(ClientError(UpgradeAccountAppStoreError::InvalidAuthDetails));
    }

    Ok(latest_receipt.expires_date_ms as UnixTimeMillis)
}

pub fn decode_verify_notification(config: &AppleConfig, request_body: &Bytes) -> Result<NotificationResponseBody, ServerError<AppStoreNotificationError>>{
    let encoded_resp: EncodedNotificationResponseBody = serde_json::from_slice(request_body).map_err(|_| ClientError(AppStoreNotificationError::InvalidJWS))?;
    println!("GOT HERE 1");
    let key = DecodingKey::from_ec_pem(config.asc_public_key.as_bytes())?;
    println!("GOT HERE 2: {}", encoded_resp.signed_payload);
    let payload = jsonwebtoken::decode::<NotificationResponseBody>(&encoded_resp.signed_payload, &key, &Validation::new(Algorithm::ES256))?;
    println!("GOT HERE 3");
    Ok(payload.claims)
}

pub fn decode_verify_transaction(config: &AppleConfig, encoded_transaction: &str) -> Result<TransactionInfo, ServerError<AppStoreNotificationError>>{
    let key = DecodingKey::from_ec_pem(config.asc_public_key.as_bytes())?;
    let payload = jsonwebtoken::decode::<TransactionInfo>(encoded_transaction, &key, &Validation::new(Algorithm::ES256))?;

    Ok(payload.claims)
}
