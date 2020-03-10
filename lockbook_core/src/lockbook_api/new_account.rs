use reqwest::Client;
use reqwest::Error as ReqwestError;
use crate::API_LOC;
use serde::Deserialize;

pub enum NewAccountError {
    SendFailed(ReqwestError),
    InvalidAuth,
    ExpiredAuth,
    UsernameTaken,
    Unspecified,
}

pub struct NewAccountParams {
    pub username: String,
    pub auth: String,
    pub pub_key_n: String,
    pub pub_key_e: String,
}

#[derive(Deserialize)]
struct NewAccountResponse {
    error_code: String
}

impl From<ReqwestError> for NewAccountError {
    fn from(e: ReqwestError) -> NewAccountError {
        NewAccountError::SendFailed(e)
    }
}

fn new_account(account: &NewAccountParams) -> Result<(), NewAccountError> {
    let client = Client::new();
    let params = [
        ("username", account.username.as_str()),
        ("auth", account.auth.as_str()),
        ("pub_key_n", account.pub_key_n.as_str()),
        ("pub_key_e", account.pub_key_e.as_str()),
    ];
    let mut response = client
        .post(format!("{}/new-account", API_LOC).as_str())
        .form(&params)
        .send()?;

    match (response.status().as_u16(), response.json::<NewAccountResponse>()?.error_code.as_str()) {
        (200..=299, _) => Ok(()),
        (401, "invalid_auth") => Err(NewAccountError::InvalidAuth),
        (401, "expired_auth") => Err(NewAccountError::ExpiredAuth),
        (409, "username_taken") => Err(NewAccountError::UsernameTaken),
        _ => Err(NewAccountError::Unspecified),
    }
}