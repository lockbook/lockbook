use reqwest::blocking::Client;
use reqwest::Error as ReqwestError;
use serde::{Deserialize, Serialize};

#[derive(Debug)]
pub enum MoveFileError {
    SendFailed(ReqwestError),
    ReceiveFailed(ReqwestError),
    InvalidAuth,
    ExpiredAuth,
    FileNotFound,
    FileDeleted,
    FilePathTaken,
    Unspecified,
}

pub struct MoveFileRequest {
    pub username: String,
    pub auth: String,
    pub file_id: String,
    pub new_file_path: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct MoveFileResponse {
    pub error_code: String,
}

pub fn move_file(api_location: String, params: &MoveFileRequest) -> Result<(), MoveFileError> {
    let client = Client::new();
    let form_params = [
        ("username", params.username.as_str()),
        ("auth", params.auth.as_str()),
        ("file_id", params.file_id.as_str()),
        ("new_file_path", params.new_file_path.as_str()),
    ];
    let response = client
        .put(format!("{}/move-file", api_location).as_str())
        .form(&form_params)
        .send()
        .map_err(|err| MoveFileError::SendFailed(err))?;

    let status = response.status().clone();
    let response_body = response
        .json::<MoveFileResponse>()
        .map_err(|err| MoveFileError::ReceiveFailed(err))?;

    match (status.as_u16(), response_body.error_code.as_str()) {
        (200..=299, _) => Ok(()),
        (401, "invalid_auth") => Err(MoveFileError::InvalidAuth),
        (401, "expired_auth") => Err(MoveFileError::ExpiredAuth),
        (404, "file_not_found") => Err(MoveFileError::FileNotFound),
        (410, "file_deleted") => Err(MoveFileError::FileDeleted),
        (422, "file_path_taken") => Err(MoveFileError::FilePathTaken),
        _ => Err(MoveFileError::Unspecified),
    }
}
