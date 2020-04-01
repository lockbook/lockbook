use reqwest::Client;
use reqwest::Error as ReqwestError;
use serde::Deserialize;

#[derive(Debug)]
pub enum GetUpdatesError {
    SendFailed(ReqwestError),
    ReceiveFailed(ReqwestError),
    InvalidAuth,
    ExpiredAuth,
    Unspecified,
}

pub struct GetUpdatesParams {
    pub username: String,
    pub auth: String,
    pub since_version: u64,
}

#[derive(Deserialize, Debug, PartialEq)]
pub struct FileMetadata {
    pub file_id: String,
    pub file_name: String,
    pub file_path: String,
    pub file_content_version: u64,
    pub file_metadata_version: u64,
    pub deleted: bool,
}

pub fn get_updates(
    api_location: String,
    params: &GetUpdatesParams,
) -> Result<Vec<FileMetadata>, GetUpdatesError> {
    let client = Client::new();
    let mut response = client
        .get(
            format!(
                "{}/get-updates/{}/{}/{}",
                api_location, params.username, params.auth, params.since_version
            )
            .as_str(),
        )
        .send()
        .map_err(|err| GetUpdatesError::SendFailed(err))?;

    match response.status().as_u16() {
        200..=299 => Ok(response
            .json::<Vec<FileMetadata>>()
            .map_err(|err| GetUpdatesError::ReceiveFailed(err))?),
        _ => Err(GetUpdatesError::Unspecified),
    }
}
