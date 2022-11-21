use jsonwebtoken::{Algorithm, encode, EncodingKey, Header};
use reqwest::header::HeaderMap;
use reqwest::RequestBuilder;
use crate::config::AppleConfig;
use crate::billing::app_store_model::{VerifyReceiptRequest, VerifyReceiptResponse};
use serde::{Deserialize, Serialize};
use lockbook_shared::clock::get_time;

const VERIFY_PROD: &str = "https://buy.itunes.apple.com/verifyReceipt";
const VERIFY_SANDBOX: &str = "https://sandbox.itunes.apple.com/verifyReceipt";

const AUDIENCE: &str = "appstoreconnect-v2";
const BUNDLE_ID: &str = "app.lockbook";

#[derive(Debug)]
pub enum AppStoreError {
    Other(String)
}

#[derive(Serialize, Deserialize, Debug)]
struct Claims {
    iss: String,
    iat: usize,
    exp: usize,
    aud: String,
    bid: String
}

pub fn gen_auth_req(config: &AppleConfig, request: RequestBuilder) -> Result<RequestBuilder, AppStoreError> {

    // println!("DA claims: {}", serde_json::to_string(&claims).unwrap());
    // println!("THE config: {:?}", config);

    let mut headers = HeaderMap::new();
    headers.insert("alg", "ES256".parse().map_err(|e| AppStoreError::Other(format!("{:?}", e)))?);
    headers.insert("kid", config.iap_key_id.parse().map_err(|e| AppStoreError::Other(format!("{:?}", e)))?);
    headers.insert("typ", "JWT".parse().map_err(|e| AppStoreError::Other(format!("{:?}", e)))?);

    let mut header = Header::new(Algorithm::ES256);
    header.kid = Some(config.iap_key_id.clone());

    let iat = get_time().0 as usize;
    let exp = (get_time().0 + 1200000) as usize;

    let claims = Claims {
        iss: config.issuer_id.to_string(),
        iat,
        exp,
        aud: AUDIENCE.to_string(),
        bid: BUNDLE_ID.to_string()
    };

    println!("CONFUSed: {:?} {:?}", header, claims);

    let token = encode(&header, &claims, &EncodingKey::from_secret(config.iap_key.as_bytes())).unwrap();

    Ok(request
        .headers(headers)
        .bearer_auth(token))
}

pub async fn request_test_notif(
    client: &reqwest::Client, config: &AppleConfig
) -> u16 {
    let resp = gen_auth_req(config, client.post("https://api.storekit-sandbox.itunes.apple.com/inApps/v1/notifications/test"))
        .unwrap()
        .send()
        .await
        .unwrap();

    return resp.status().as_u16()
}

pub async fn verify_receipt(
    client: &reqwest::Client, config: &AppleConfig, encoded_receipt: &str
) -> Result<VerifyReceiptResponse, AppStoreError> {
    let req_body = serde_json::to_string(&VerifyReceiptRequest {
        encoded_receipt: encoded_receipt.to_string(),
        password: config.asc_shared_secret.clone(),
        exclude_old_transactions: true
    }).map_err(|e| AppStoreError::Other(format!("Cannot parse verify receipt request body: {:?}", e)))?;

    let resp = gen_auth_req(config, client.post(VERIFY_PROD))?
        .body(req_body.clone())
        .send()
        .await?;

    let resp_n = resp.status().as_u16();

    match resp_n {
        200 => {
            Ok(resp.json().await?)
        }
        21007 => {
            let resp = gen_auth_req(config, client.post(VERIFY_SANDBOX))?
                .body(req_body)
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
