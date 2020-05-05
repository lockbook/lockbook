use reqwest::blocking::Client;
use reqwest::Error as ReqwestError;
use serde::{Deserialize, Serialize};

#[derive(Debug)]
pub enum NewAccountError {
    SendFailed(ReqwestError),
    ReceiveFailed(ReqwestError),
    InvalidAuth,
    ExpiredAuth,
    UsernameTaken,
    Unspecified,
}

pub struct NewAccountRequest {
    pub username: String,
    pub auth: String,
    pub public_key: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct NewAccountResponse {
    pub error_code: String,
}

pub fn new_account(
    api_location: String,
    params: &NewAccountRequest,
) -> Result<(), NewAccountError> {
    let client = Client::new();
    let form_params = [
        ("username", params.username.as_str()),
        ("auth", params.auth.as_str()),
        ("public_key", params.public_key.as_str()),
    ];
    let mut response = client
        .post(format!("{}/new-account", api_location).as_str())
        .form(&form_params)
        .send()
        .map_err(|err| NewAccountError::SendFailed(err))?;

    let response_body = response
        .json::<NewAccountResponse>()
        .map_err(|err| NewAccountError::ReceiveFailed(err))?;

    match (
        response.status().as_u16(),
        response_body.error_code.as_str(),
    ) {
        (200..=299, _) => Ok(()),
        (401, "invalid_auth") => Err(NewAccountError::InvalidAuth),
        (401, "expired_auth") => Err(NewAccountError::ExpiredAuth),
        (422, "username_taken") => Err(NewAccountError::UsernameTaken),
        _ => Err(NewAccountError::Unspecified),
    }
}
