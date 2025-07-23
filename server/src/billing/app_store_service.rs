use crate::billing::app_store_model::{
    EncodedNotificationResponseBody, NotificationResponseBody, TransactionInfo,
};
use crate::billing::billing_service::AppStoreNotificationError;
use crate::config::AppleConfig;
use crate::document_service::DocumentService;
use crate::{ClientError, ServerError, ServerState};
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};
use lb_rs::model::api::{AppStoreAccountState, UnixTimeMillis, UpgradeAccountAppStoreError};
use libsecp256k1::PublicKey;
use serde::Serialize;
use serde::de::DeserializeOwned;
use tracing::debug;
use warp::hyper::body::Bytes;
use x509_parser::error::X509Error;
use x509_parser::parse_x509_certificate;
use x509_parser::prelude::X509Certificate;

use super::app_store_client::AppStoreClient;
use super::google_play_client::GooglePlayClient;
use super::stripe_client::StripeClient;

const SUBSCRIBED: u16 = 1;
const EXPIRED: u16 = 2;
const BILLING_RETRY: u16 = 3;
const GRACE_PERIOD: u16 = 4;

impl<S, A, G, D> ServerState<S, A, G, D>
where
    S: StripeClient,
    A: AppStoreClient,
    G: GooglePlayClient,
    D: DocumentService,
{
    pub async fn get_public_key_from_tx(
        &self, trans: &TransactionInfo,
    ) -> Result<PublicKey, ServerError<AppStoreNotificationError>> {
        let public_key: PublicKey = self
            .index_db
            .lock()
            .await
            .app_store_ids
            .get()
            .get(&trans.app_account_token)
            .ok_or_else(|| {
                internal!("There is no public_key related to this app_account_token: {:?}", trans)
            })?
            .0;

        Ok(public_key)
    }

    pub async fn verify_details(
        client: &A, config: &AppleConfig, app_account_token: &str, original_transaction_id: &str,
    ) -> Result<(UnixTimeMillis, AppStoreAccountState), ServerError<UpgradeAccountAppStoreError>>
    {
        let (transaction, transaction_info) = client
            .get_sub_status(config, original_transaction_id)
            .await?;

        debug!(?transaction_info.app_account_token, ?app_account_token, "Comparing verified app account token and with unverified");
        debug!(?transaction.original_transaction_id, ?original_transaction_id, "Comparing verified original transaction id and with unverified");
        debug!(?transaction.status, "Checking the subscription status.");

        let account_state = match transaction.status {
            SUBSCRIBED => AppStoreAccountState::Ok,
            EXPIRED => AppStoreAccountState::Expired,
            BILLING_RETRY => AppStoreAccountState::FailedToRenew,
            GRACE_PERIOD => AppStoreAccountState::GracePeriod,
            _ => return Err(internal!("Unknown subscription status.")),
        };

        if transaction_info.app_account_token != app_account_token
            || transaction.original_transaction_id != original_transaction_id
        {
            return Err(ClientError(UpgradeAccountAppStoreError::InvalidAuthDetails));
        }

        Ok((transaction_info.expires_date as UnixTimeMillis, account_state))
    }

    pub fn decode_verify_notification(
        config: &AppleConfig, request_body: &Bytes,
    ) -> Result<NotificationResponseBody, ServerError<AppStoreNotificationError>> {
        let encoded_resp: EncodedNotificationResponseBody = serde_json::from_slice(request_body)
            .map_err(|_| ClientError(AppStoreNotificationError::InvalidJWS))?;

        Self::validate_jwt(config, &encoded_resp.signed_payload)
    }

    fn validate_jwt<T: Serialize + DeserializeOwned>(
        config: &AppleConfig, token: &str,
    ) -> Result<T, ServerError<AppStoreNotificationError>> {
        let header = decode_header(token)?;
        let cert_chain: Vec<Vec<u8>> = header
            .x5c
            .ok_or(ClientError(AppStoreNotificationError::InvalidJWS))?
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
                            .map_err(|err| internal!("{:?}", err))?
                            .1
                            .subject_pki,
                    ))
                    .map_err(|_| ClientError(AppStoreNotificationError::InvalidJWS))?;
            }
        }

        let pem = format!(
            "-----BEGIN PUBLIC KEY-----\n{}\n-----END PUBLIC KEY-----",
            base64::encode(
                certs
                    .first()
                    .ok_or(ClientError(AppStoreNotificationError::InvalidJWS))?
                    .public_key()
                    .raw
            )
        );
        let key = DecodingKey::from_ec_pem(pem.as_bytes())?;

        let mut validate = Validation::new(Algorithm::ES256);
        validate.required_spec_claims.remove("exp");

        Ok(decode::<T>(token, &key, &validate)?.claims)
    }

    pub fn decode_verify_transaction(
        config: &AppleConfig, encoded_transaction: &str,
    ) -> Result<TransactionInfo, ServerError<AppStoreNotificationError>> {
        Self::validate_jwt(config, encoded_transaction)
    }
}
