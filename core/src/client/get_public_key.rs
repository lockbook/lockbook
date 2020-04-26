use reqwest::Client;
use reqwest::Error as ReqwestError;
use std::option::NoneError;
use std::string::FromUtf8Error;

#[derive(Debug)]
pub enum GetPublicKeyError {
    UsernameNotFound,
    SendFailed(ReqwestError),
    ParseError(FromUtf8Error),
    ReceiveFailed
}

impl From<NoneError> for GetPublicKeyError {
    fn from(_e: NoneError) -> GetPublicKeyError {
        GetPublicKeyError::ReceiveFailed
    }
}

impl From<FromUtf8Error> for GetPublicKeyError {
    fn from(e: FromUtf8Error) -> GetPublicKeyError {
        GetPublicKeyError::ParseError(e)
    }
}

pub struct GetPublicKeyRequest {
    pub username: String
}

pub fn get_public_key(
    api_location: String,
    params: &GetPublicKeyRequest,
) -> Result<String, GetPublicKeyError> {
    let client = Client::new();
    let response = client
        .get(
            format!(
                "{}/get-public-key/{}",
                api_location, params.username
            )
                .as_str(),
        )
        .send()
        .map_err(|err| GetPublicKeyError::SendFailed(err))?;

    match response.status().as_u16() {
        200..=299 => Ok(String::from_utf8(Vec::from(response
            .headers()
            .get("public_key")?
            .as_bytes()))?),
        _ => Err(GetPublicKeyError::UsernameNotFound),
    }
}
