use crate::billing::app_store_client;
use crate::billing::app_store_model::{
    EncodedNotificationResponseBody, NotificationResponseBody, TransactionInfo,
};
use crate::billing::billing_service::AppStoreNotificationError;
use crate::config::AppleConfig;
use crate::ServerError::InternalError;
use crate::{ClientError, ServerError, ServerState};
use itertools::Itertools;
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, TokenData, Validation};
use libsecp256k1::PublicKey;
use lockbook_shared::api::{UnixTimeMillis, UpgradeAccountAppStoreError};
use lockbook_shared::clock::get_time;
use serde::de::DeserializeOwned;
use serde::Serialize;
use warp::hyper::body::Bytes;
use x509_parser::error::X509Error;
use x509_parser::parse_x509_certificate;
use x509_parser::prelude::X509Certificate;

pub fn get_public_key(
    state: &ServerState, trans: &TransactionInfo,
) -> Result<PublicKey, ServerError<AppStoreNotificationError>> {
    let public_key: PublicKey = state
        .index_db
        .app_store_ids
        .get(&trans.app_account_token)?
        .ok_or_else(|| {
            internal!("There is no public_key related to this app_account_token: {:?}", trans)
        })?
        .0;

    Ok(public_key)
}

pub async fn verify_receipt(
    client: &reqwest::Client, config: &AppleConfig, encoded_receipt: &str, app_account_token: &str,
    original_transaction_id: &str,
) -> Result<UnixTimeMillis, ServerError<UpgradeAccountAppStoreError>> {
    let validated_receipts = app_store_client::verify_receipt(client, config, encoded_receipt)
        .await?
        .latest_receipt_info
        .ok_or_else(|| {
            InternalError::<UpgradeAccountAppStoreError>(
                "A verify receipt status of 0 should include the latest receipt".to_string(),
            )
        })?;

    let latest_receipt = validated_receipts.get(0).unwrap();
    let expires_date_ms: i64 = latest_receipt.expires_date_ms.parse().unwrap();

    if latest_receipt.app_account_token != app_account_token
        || latest_receipt.original_transaction_id != original_transaction_id
        || expires_date_ms < get_time().0
    {
        return Err(ClientError(UpgradeAccountAppStoreError::InvalidAuthDetails));
    }

    Ok(expires_date_ms as UnixTimeMillis)
}

pub fn decode_verify_notification(
    config: &AppleConfig, request_body: &Bytes,
) -> Result<NotificationResponseBody, ServerError<AppStoreNotificationError>> {
    let encoded_resp: EncodedNotificationResponseBody = serde_json::from_slice(request_body)
        .map_err(|_| ClientError(AppStoreNotificationError::InvalidJWS))?;

    Ok(validate_jwt(&config, &encoded_resp.signed_payload)?)
}

fn validate_jwt<T: Serialize + DeserializeOwned>(
    config: &AppleConfig, token: &str,
) -> Result<T, ServerError<AppStoreNotificationError>> {
    let header = decode_header(token).unwrap();
    let cert_chain: Vec<Vec<u8>> = header
        .x5c
        .clone()
        .unwrap()
        .into_iter()
        .map(|cert| base64::decode(cert.as_bytes()))
        .collect::<Result<Vec<Vec<u8>>, base64::DecodeError>>()?;

    let certs: Vec<X509Certificate> = cert_chain
        .iter()
        .map(|cert| parse_x509_certificate(cert.as_slice()).map(|(_, cert)| cert))
        .collect::<Result<Vec<X509Certificate>, x509_parser::nom::Err<X509Error>>>()
        .map_err(|_| ClientError(AppStoreNotificationError::InvalidJWS))?;

    for i in 0..certs.len() {
        if i != certs.len() - 1 {
            certs[i]
                .verify_signature(Some(&certs[i + 1].subject_pki))
                .map_err(|_| ClientError(AppStoreNotificationError::InvalidJWS))?;
        } else {
            certs[i]
                .verify_signature(Some(
                    &parse_x509_certificate(config.apple_root_cert.as_slice())
                        .unwrap()
                        .1
                        .subject_pki,
                ))
                .map_err(|_| ClientError(AppStoreNotificationError::InvalidJWS))?;
        }
    }

    let pem = format!(
        "-----BEGIN PUBLIC KEY-----\n{}\n-----END PUBLIC KEY-----",
        base64::encode(certs[0].public_key().raw)
    );
    let key = DecodingKey::from_ec_pem(pem.as_bytes())?;

    let mut validate = Validation::new(Algorithm::ES256);
    validate.required_spec_claims.remove("exp");

    Ok(decode::<T>(&token, &key, &validate)?.claims)
}

pub fn decode_verify_transaction(
    config: &AppleConfig, encoded_transaction: &str,
) -> Result<TransactionInfo, ServerError<AppStoreNotificationError>> {
    Ok(validate_jwt(config, encoded_transaction)?)
}
