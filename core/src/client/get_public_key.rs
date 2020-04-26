use reqwest::Client;
use reqwest::Error as ReqwestError;

#[derive(Debug)]
pub enum GetPublicKeyError {
    UsernameNotFound,
    SendFailed(ReqwestError),
    ReceiveFailed(ReqwestError),
}

pub fn get_public_key(
    api_location: String,
    username: &String,
) -> Result<String, GetPublicKeyError> {
    let client = Client::new();
    let mut response = client
        .get(
            format!(
                "{}/get-public-key/{}",
                api_location, username
            )
                .as_str(),
        )
        .send()
        .map_err(|err| GetPublicKeyError::SendFailed(err))?;

    match response.status().as_u16() {
        200..=299 => Ok(response
            .json::<String>()
            .map_err(|err| GetPublicKeyError::ReceiveFailed(err))?),
        _ => Err(GetPublicKeyError::UsernameNotFound),
    }
}
