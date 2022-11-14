use std::collections::BTreeMap;
use jwt_simple::algorithms::ECDSAP256KeyPairLike;
use jwt_simple::claims::{Audiences, Claims};
use jwt_simple::prelude::{Duration, ES256KeyPair, NoCustomClaims};
use reqwest::header::HeaderMap;
use reqwest::RequestBuilder;
use lockbook_shared::clock::get_time;
use crate::config::AppleConfig;
use crate::billing::app_store_model::{VerifyReceiptRequest, VerifyReceiptResponse};
use serde::{Deserialize, Serialize};

const STORE_KIT_URL: &str = "https://api.storekit.itunes.apple.com";
const VERIFY_PROD: &str = "https://buy.itunes.apple.com/verifyReceipt";
const VERIFY_SANDBOX: &str = "https://sandbox.itunes.apple.com/verifyReceipt";
const SANDBOX_STORE_KIT_URL: &str = "https://api.storekit-sandbox.itunes.apple.com";

const AUDIENCE: &str = "appstoreconnect-v1";
const BUNDLE_ID: &str = "app.lockbook";

fn subscription_status_url(original_transaction_id: &str) -> String {
    return format!("{}/inApps/v1/subscriptions/{}", STORE_KIT_URL, original_transaction_id);
}

#[derive(Debug)]
pub enum AppStoreError {
    Other(String)
}

#[derive(Serialize, Deserialize, Debug)]
struct BundleIdClaim {
    bid: String
}

pub fn gen_auth_req(config: &AppleConfig, request: RequestBuilder) -> RequestBuilder {
    let mut claims = Claims::with_custom_claims(
        BundleIdClaim { bid: BUNDLE_ID.to_string() },
        Duration::from_hours(1)
    );
    claims.audiences = Some(Audiences::AsString(AUDIENCE.to_string()));
    claims.issuer = Some(config.issuer_id.to_string());

    let token = config.iap_key.sign(claims).unwrap();

    let mut headers = HeaderMap::new();
    headers.typed_insert("kid", &config.iap_key_id);
    headers.typed_insert("typ", "JWT");
    headers.typed_insert("alg", "ES256");

    request
        .headers(headers)
        .bearer_auth(token)
}


pub async fn verify_receipt(
    client: &reqwest::Client, config: &AppleConfig, encoded_receipt: &str
) -> Result<VerifyReceiptResponse, AppStoreError> {
    let req_body = VerifyReceiptRequest {
        encoded_receipt: encoded_receipt.to_string(),
        password: config.asc_shared_secret.clone(),
        exclude_old_transactions: true
    };

    let resp = gen_auth_req(config, client.post(VERIFY_PROD))
        .body(&req_body)
        .send()
        .await?;

    let resp_n = resp.status().as_u16();

    match resp_n {
        200 => {
            Ok(resp.json().await?)
        }
        21007 => {
            let resp = gen_auth_req(config, client.post(VERIFY_SANDBOX))
                .body(&req_body)
                .send()
                .await?;

            if resp.status().as_u16() != 200 {
                Ok(resp.json().await?)
            } else {
                Err(AppStoreError::Other(format!("Unexpected response: {}", resp.status().as_str())))
            }
        }
        _ => Err(AppStoreError::Other(format!("Unexpected response: {}", resp.status().as_str())))
    }
}
