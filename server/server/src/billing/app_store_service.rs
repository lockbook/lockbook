use libsecp256k1::PublicKey;
use lockbook_shared::api::{UnixTimeMillis, UpgradeAccountAppStoreError};
use lockbook_shared::clock::get_time;
use crate::billing::app_store_model::{ReceiptInfo, TransactionInfo};
use crate::{ClientError, ServerError, ServerState};
use crate::billing::app_store_client;
use crate::billing::billing_service::GooglePlayWebhookError;
use crate::config::AppleConfig;

pub fn get_public_key(
    state: &ServerState, trans: &TransactionInfo
) -> Result<PublicKey, ServerError<GooglePlayWebhookError>> {
    let public_key: PublicKey = state
        .index_db
        .google_play_ids
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

    let latest_receipt: ReceiptInfo = serde_json::from_slice(base64::decode(receipt.encoded_latest_receipt)?.as_slice())?;

    if latest_receipt.app_account_token != app_account_token || latest_receipt.original_transaction_id != original_transaction_id || latest_receipt.expires_date_ms < get_time().0 {
        return Err(ClientError(UpgradeAccountAppStoreError::InvalidAuthDetails));
    }

    Ok(latest_receipt.expires_date_ms as UnixTimeMillis)
}
