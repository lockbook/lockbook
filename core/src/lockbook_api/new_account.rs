use reqwest::Client;
use reqwest::Error as ReqwestError;
use serde::Deserialize;

#[derive(Debug)]
pub enum NewAccountError {
    SendFailed(ReqwestError),
    ReceiveFailed(ReqwestError),
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
    error_code: String,
}

pub fn new_account(api_location: String, params: &NewAccountParams) -> Result<(), NewAccountError> {
    let client = Client::new();
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
