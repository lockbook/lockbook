use reqwest::Client;
use reqwest::Error as ReqwestError;
use serde::Deserialize;

use crate::auth_service::{AuthService, AuthServiceImpl, VerificationError};
use crate::crypto::PublicKey;
use crate::lockbook_api::new_account::NewAccountError::AuthVerificationFailure;

#[derive(Debug)]
pub enum NewAccountError {
    SendFailed(ReqwestError),
    ReceiveFailed(ReqwestError),
    InvalidAuth,
    ExpiredAuth,
    UsernameTaken,
    Unspecified,
    AuthVerificationFailure(VerificationError),
}

pub struct NewAccountRequest {
    pub username: String,
    pub auth: String,
    pub pub_key_n: String,
    pub pub_key_e: String,
}

#[derive(Deserialize)]
struct NewAccountResponse {
    error_code: String,
}

impl From<VerificationError> for NewAccountError {
    fn from(e: VerificationError) -> Self { AuthVerificationFailure(e) }
}

trait NewAccountClient {
    fn new_account(
        api_location: String,
        params: &NewAccountRequest,
    ) -> Result<(), NewAccountError>;
}

pub struct NewAccountClientImpl;

impl NewAccountClient for NewAccountClientImpl {
    fn new_account(
        api_location: String,
        params: &NewAccountRequest,
    ) -> Result<(), NewAccountError> {
        let client = Client::new();

        AuthServiceImpl::verify_auth(
            &PublicKey {
                n: params.pub_key_n.clone(),
                e: params.pub_key_e.clone(),
            },
            &params.username,
            &params.auth)?;

        let form_params = [
            ("username", params.username.as_str()),
            ("auth", params.auth.as_str()),
            ("pub_key_n", params.pub_key_n.as_str()),
            ("pub_key_e", params.pub_key_e.as_str()),
        ];
        let mut response = client
            .post(format!("{}/new-account", api_location).as_str())
            .form(&form_params)
            .send()
            .map_err(|err| NewAccountError::SendFailed(err))?;

        match response.status().as_u16() {
            200..=299 => Ok(()),
            status => match (
                status,
                response
                    .json::<NewAccountResponse>()
                    .map_err(|err| NewAccountError::ReceiveFailed(err))?
                    .error_code
                    .as_str(),
            ) {
                (401, "invalid_auth") => Err(NewAccountError::InvalidAuth),
                (401, "expired_auth") => Err(NewAccountError::ExpiredAuth),
                (422, "username_taken") => Err(NewAccountError::UsernameTaken),
                _ => Err(NewAccountError::Unspecified),
            },
        }
    }
}
