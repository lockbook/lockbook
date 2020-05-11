use reqwest::blocking::Client;
use reqwest::Error as ReqwestError;
use rsa::RSAPublicKey;

#[derive(Debug)]
pub enum GetPublicKeyError {
    UsernameNotFound,
    SendFailed(ReqwestError),
    ReceiveFailed(ReqwestError),
    InvalidPublicKey,
    Unspecified,
}

pub struct GetPublicKeyRequest {
    pub username: String,
}

pub fn get_public_key(
    api_location: String,
    params: &GetPublicKeyRequest,
) -> Result<RSAPublicKey, GetPublicKeyError> {
    let client = Client::new();
    let response = client
        .get(format!("{}/get-public-key/{}", api_location, params.username).as_str())
        .send()
        .map_err(|err| GetPublicKeyError::SendFailed(err))?;

    match response.status().as_u16() {
        200..=299 => Ok(response
            .json::<RSAPublicKey>()
            .map_err(|err| GetPublicKeyError::ReceiveFailed(err))?),
        404 => Err(GetPublicKeyError::UsernameNotFound),
        409 => Err(GetPublicKeyError::InvalidPublicKey),
        _ => Err(GetPublicKeyError::Unspecified),
    }
}
