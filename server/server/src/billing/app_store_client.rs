use crate::billing::app_store_model::{VerifyReceiptRequest, VerifyReceiptResponse};
use crate::config::AppleConfig;
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use lockbook_shared::clock::get_time;
use reqwest::RequestBuilder;
use serde::{Deserialize, Serialize};

const VERIFY_PROD: &str = "https://buy.itunes.apple.com/verifyReceipt";
const VERIFY_SANDBOX: &str = "https://sandbox.itunes.apple.com/verifyReceipt";

const ALG: &str = "ES256";
const TYP: &str = "JWT";
const AUDIENCE: &str = "appstoreconnect-v1";
const BUNDLE_ID: &str = "app.lockbook";

#[derive(Debug)]
pub enum AppStoreError {
    Other(String),
}

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
) -> Result<RequestBuilder, AppStoreError> {
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
) -> Result<VerifyReceiptResponse, AppStoreError> {
    let req_body = serde_json::to_string(&VerifyReceiptRequest {
        encoded_receipt: encoded_receipt.to_string(),
        password: config.asc_shared_secret.clone(),
        exclude_old_transactions: true,
    })
    .map_err(|e| {
        AppStoreError::Other(format!("Cannot parse verify receipt request body: {:?}", e))
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

            if resp_body.status == 0 {
                Ok(resp_body)
            } else {
                Err(AppStoreError::Other(format!("Unexpected response: {:?}", resp_body)))
            }
        }
        _ => Err(AppStoreError::Other(format!("Unexpected response: {:?}", resp_body))),
    }
}
