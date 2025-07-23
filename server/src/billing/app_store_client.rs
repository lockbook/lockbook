use crate::billing::app_store_model::{
    ErrorBody, LastTransactionItem, SubGroupIdentifierItem, SubsStatusesResponse, TransactionInfo,
};
use crate::config::AppleConfig;
use crate::{ClientError, ServerError};
use async_trait::async_trait;
use itertools::Itertools;
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use lb_rs::model::api::UpgradeAccountAppStoreError;
use lb_rs::model::clock::get_time;
use reqwest::{Client, RequestBuilder};
use serde::{Deserialize, Serialize};
use tracing::{debug, error};

pub const SUB_STATUS_PROD: &str = "https://api.storekit.itunes.apple.com/inApps/v1/subscriptions";
pub const SUB_STATUS_SANDBOX: &str =
    "https://api.storekit-sandbox.itunes.apple.com/inApps/v1/subscriptions";

const ALG: &str = "ES256";
const TYP: &str = "JWT";
const AUDIENCE: &str = "appstoreconnect-v1";
const BUNDLE_ID: &str = "app.lockbook";

pub const ORIGINAL_TRANS_ID_NOT_FOUND_ERR_CODE: u64 = 4040005;
pub const TRANS_ID_NOT_FOUND_ERR_CODE: u64 = 4040010;

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

#[async_trait]
pub trait AppStoreClient: Sync + Send + Clone + 'static {
    async fn get_sub_status(
        &self, config: &AppleConfig, original_transaction_id: &str,
    ) -> Result<(LastTransactionItem, TransactionInfo), ServerError<UpgradeAccountAppStoreError>>;
}

#[async_trait]
impl AppStoreClient for Client {
    async fn get_sub_status(
        &self, config: &AppleConfig, original_transaction_id: &str,
    ) -> Result<(LastTransactionItem, TransactionInfo), ServerError<UpgradeAccountAppStoreError>>
    {
        let resp =
            gen_auth_req(config, self.get(format!("{SUB_STATUS_PROD}/{original_transaction_id}")))?
                .send()
                .await?;

        let resp_status = resp.status().as_u16();
        match resp_status {
            200 => {
                debug!("Successfully retrieved subscription status from production apple url");

                let sub_status: SubsStatusesResponse = resp.json().await?;

                for sub_group in &sub_status.data {
                    if sub_group.sub_group == config.monthly_sub_group_id {
                        return get_trans(&sub_status, sub_group);
                    }
                }

                Err(internal!(
                    "No usable data returned from apple's production subscriptions statuses endpoint despite assumed match. resp_body: {:?}, monthly_sub_group: {}",
                    sub_status,
                    config.monthly_sub_group_id
                ))
            }
            400 | 404 => {
                let error: ErrorBody = resp.json().await?;

                if error.error_code == ORIGINAL_TRANS_ID_NOT_FOUND_ERR_CODE
                    || error.error_code == TRANS_ID_NOT_FOUND_ERR_CODE
                {
                    debug!(
                        "Could not verify subscription from apple's production servers, trying sandbox"
                    );

                    let resp = gen_auth_req(
                        config,
                        self.get(format!("{SUB_STATUS_SANDBOX}/{original_transaction_id}")),
                    )?
                    .send()
                    .await?;

                    let resp_status = resp.status().as_u16();
                    match resp_status {
                        200 => {
                            debug!(
                                "Successfully retrieved subscription status from sandbox apple url"
                            );

                            let sub_status: SubsStatusesResponse = resp.json().await?;

                            for sub_group in &sub_status.data {
                                if sub_group.sub_group == config.monthly_sub_group_id {
                                    return get_trans(&sub_status, sub_group);
                                }
                            }

                            return Err(internal!(
                                "No usable data returned from apple's sandbox subscriptions statuses endpoint despite assumed match. resp_body: {:?}, monthly_sub_group: {}",
                                sub_status,
                                config.monthly_sub_group_id
                            ));
                        }
                        400 | 404 => {
                            error!(
                                ?resp_status,
                                ?error,
                                ?original_transaction_id,
                                "Failed to verify possible sandbox subscription"
                            );

                            return Err(ClientError(
                                UpgradeAccountAppStoreError::InvalidAuthDetails,
                            ));
                        }
                        _ => return Err(internal!("Unexpected response: {:?}", resp_status)),
                    }
                }

                error!(
                    ?resp_status,
                    ?error,
                    ?original_transaction_id,
                    "Failed to verify possible production subscription"
                );

                Err(ClientError(UpgradeAccountAppStoreError::InvalidAuthDetails))
            }
            _ => Err(internal!("Unexpected response: {:?}", resp_status)),
        }
    }
}

fn get_trans(
    sub_status: &SubsStatusesResponse, sub_group: &SubGroupIdentifierItem,
) -> Result<(LastTransactionItem, TransactionInfo), ServerError<UpgradeAccountAppStoreError>> {
    let last_trans = sub_group
        .last_transactions
        .first()
        .ok_or(ClientError(UpgradeAccountAppStoreError::InvalidAuthDetails))?;

    let part = <&str>::clone(
        last_trans
            .signed_transaction_info
            .split('.')
            .collect_vec()
            .get(1)
            .ok_or_else::<ServerError<UpgradeAccountAppStoreError>, _>(|| {
                internal!("There should be a payload in apple jwt: {:?}", sub_status)
            })?,
    );

    let trans_info = serde_json::from_slice(&base64::decode(part).map_err::<ServerError<
        UpgradeAccountAppStoreError,
    >, _>(|err| {
        internal!("Cannot decode apple jwt payload: {:?}, err: {:?}", part, err)
    })?)?;

    Ok((last_trans.clone(), trans_info))
}
