use libsecp256k1::PublicKey;
use warp::hyper::body::Bytes;
use lockbook_shared::api::{UnixTimeMillis, UpgradeAccountAppStoreError};
use lockbook_shared::clock::get_time;
use crate::billing::app_store_model::{EncodedNotificationResponseBody, NotificationResponseBody, ReceiptInfo, TransactionInfo};
use crate::{ClientError, ServerError, ServerState};
use crate::billing::app_store_client;
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

pub fn decode_verify_notification(server_state: &ServerState, request_body: &Bytes) -> Result<NotificationResponseBody, ServerError<AppStoreNotificationError>>{
    // let key = ES256KeyPair::from_pem(&server_state.config.billing.apple.iap_key)?;

    let encoded_resp: EncodedNotificationResponseBody = serde_json::from_slice(request_body).map_err(|_| ClientError(AppStoreNotificationError::InvalidJWS))?;

    // key.public_key().verify_token::<NoCustomClaims>(&encoded_resp.signed_payload, None).map_err(|_| ClientError(AppStoreNotificationError::InvalidJWS))?;

    let resp = serde_json::from_slice::<NotificationResponseBody>(&base64::decode(encoded_resp.signed_payload).map_err(|_| ClientError(AppStoreNotificationError::InvalidJWS))?)?;

    Ok(resp)
}

pub fn decode_verify_transaction(server_state: &ServerState, encoded_transaction: &str) -> Result<TransactionInfo, ServerError<AppStoreNotificationError>>{
    // let key = ES256KeyPair::from_pem(&server_state.config.billing.apple.iap_key)?;

    // key.public_key().verify_token::<NoCustomClaims>(encoded_transaction, None).map_err(|_| ClientError(AppStoreNotificationError::InvalidJWS))?;

    let resp = serde_json::from_slice::<TransactionInfo>(&base64::decode(encoded_transaction).map_err(|_| ClientError(AppStoreNotificationError::InvalidJWS))?)?;

    Ok(resp)
}
