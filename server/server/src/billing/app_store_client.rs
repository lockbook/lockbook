use itertools::Itertools;
use crate::billing::app_store_model::{LastTransactionItem, SubsStatusesResponse, TransactionInfo, VerifyReceiptRequest, VerifyReceiptResponse};
use crate::config::AppleConfig;
use crate::{ClientError, ServerError};
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use lockbook_shared::api::UpgradeAccountAppStoreError;
use lockbook_shared::clock::get_time;
use reqwest::RequestBuilder;
use serde::{Deserialize, Serialize};
use tracing::debug;

const VERIFY_PROD: &str = "https://buy.itunes.apple.com/verifyReceipt";
const VERIFY_SANDBOX: &str = "https://sandbox.itunes.apple.com/verifyReceipt";

pub const SUB_STATUS_PROD: &str = "https://api.storekit.itunes.apple.com/inApps/v1/subscriptions";
pub const SUB_STATUS_SANDBOX: &str = "https://api.storekit-sandbox.itunes.apple.com/inApps/v1/subscriptions";

const ALG: &str = "ES256";
const TYP: &str = "JWT";
const AUDIENCE: &str = "appstoreconnect-v1";
const BUNDLE_ID: &str = "app.lockbook";

const SUB_GROUP: &str = "monthly";

#[derive(Serialize, Deserialize, Debug)]
pub struct Claims {
    iss: String,
    iat: usize,
    exp: usize,
    aud: String,
    bid: String,
}

pub fn gen_auth_req(
    config: &AppleConfig, request: RequestBuilder,
) -> Result<RequestBuilder, ServerError<UpgradeAccountAppStoreError>> {
    let mut header = Header::new(Algorithm::ES256);
    header.kid = Some(config.iap_key_id.clone());

    let iat = (get_time().0 / 1000) as usize;
    let exp = (get_time().0 / 1000 + 1200) as usize;

    let claims = Claims {
        iss: config.issuer_id.to_string(),
        iat,
        exp,
        aud: AUDIENCE.to_string(),
        bid: BUNDLE_ID.to_string(),
    };

    let token = encode(&header, &claims, &EncodingKey::from_ec_pem(config.iap_key.as_bytes())?)?;
    Ok(request
        .header("alg", ALG)
        .header("kid", &config.iap_key_id)
        .header("typ", TYP)
        .bearer_auth(token))
}

pub async fn verify_receipt(
    client: &reqwest::Client, config: &AppleConfig, encoded_receipt: &str,
) -> Result<VerifyReceiptResponse, ServerError<UpgradeAccountAppStoreError>> {
    let req_body = serde_json::to_string(&VerifyReceiptRequest {
        encoded_receipt: encoded_receipt.to_string(),
        password: config.asc_shared_secret.clone(),
        exclude_old_transactions: true,
    })?;

    let resp = gen_auth_req(config, client.post(VERIFY_PROD))?
        .body(req_body.clone())
        .send()
        .await?;

    let resp_body: VerifyReceiptResponse = resp.json().await?;

    match resp_body.status {
        0 => Ok(resp_body),
        21007 => {
            let resp = gen_auth_req(config, client.post(VERIFY_SANDBOX))?
                .body(req_body)
                .send()
                .await?;

            let resp_body: VerifyReceiptResponse = resp.json().await?;

            match resp_body.status {
                0 => Ok(resp_body),
                21002 | 21003 | 21006 | 21010 => {
                    debug!(?resp_body, "Failed to verify receipt.");
                    Err(ClientError(UpgradeAccountAppStoreError::InvalidAuthDetails))
                }
                _ => Err(internal!("Unexpected response: {:?}", resp_body)),
            }
        }
        21002 | 21003 | 21006 | 21010 => Err(ClientError(UpgradeAccountAppStoreError::InvalidAuthDetails)),
        _ => Err(internal!("Unexpected response: {:?}", resp_body)),
    }
}

pub async fn get_sub_status(client: &reqwest::Client, config: &AppleConfig, original_transaction_id: &str)
    -> Result<(LastTransactionItem, TransactionInfo), ServerError<UpgradeAccountAppStoreError>> {
    let resp = gen_auth_req(config, client.get(format!("{}/{}", SUB_STATUS_PROD, original_transaction_id)))?
        .send()
        .await?;

    let resp_status = resp.status().as_u16();

    match resp_status {
        200 => {
            let sub_status: SubsStatusesResponse = resp.json().await?;

            for sub_group in &sub_status.data {
                if sub_group.sub_group == SUB_GROUP {
                    let last_trans = sub_group.last_transactions.get(0).ok_or(ClientError(UpgradeAccountAppStoreError::InvalidAuthDetails))?;

                    let trans_info = serde_json::from_str(last_trans.signed_transaction_info.split(".").collect_vec().get(1).ok_or_else(|| -> ServerError<UpgradeAccountAppStoreError> {
                        internal!("There should be a payload in apple jwt: {:?}", sub_status) })?)?;

                    return Ok((last_trans.clone(), trans_info));
                }
            }

            Err(ClientError(UpgradeAccountAppStoreError::InvalidAuthDetails))
        },
        400 | 404 => Err(ClientError(UpgradeAccountAppStoreError::InvalidAuthDetails)),
        _ => Err(internal!("Unexpected response: {:?}", resp_status)), // 401
    }
}
