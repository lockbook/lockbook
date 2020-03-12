use crate::API_LOC;
use reqwest::Client;
use reqwest::Error as ReqwestError;
use serde::Deserialize;

pub enum GetUpdatesError {
    SendFailed(ReqwestError),
    InvalidAuth,
    ExpiredAuth,
    Unspecified,
}

pub struct GetUpdatesParams {
    pub username: String,
    pub auth: String,
    pub since_version: u64,
}

#[derive(Deserialize)]
pub struct FileMetadata {
    pub file_id: String,
    pub file_name: String,
    pub file_path: String,
    pub file_content_version: u64,
    pub file_metadata_version: u64,
    pub deleted: bool,
}

impl From<ReqwestError> for GetUpdatesError {
    fn from(e: ReqwestError) -> GetUpdatesError {
        GetUpdatesError::SendFailed(e)
    }
}

pub fn get_updates(params: &GetUpdatesParams) -> Result<Vec<FileMetadata>, GetUpdatesError> {
    let client = Client::new();
    let mut response = client
        .get(
            format!(
                "{}/get-updates/{}/{}/{}",
                API_LOC, params.username, params.auth, params.since_version
            )
            .as_str(),
        )
        .send()?;

    match response.status().as_u16() {
        200..=299 => Ok(response.json::<Vec<FileMetadata>>()?),
        _ => Err(GetUpdatesError::Unspecified),
    }
}
