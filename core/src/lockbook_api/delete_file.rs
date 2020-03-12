use crate::API_LOC;
use reqwest::Client;
use reqwest::Error as ReqwestError;
use serde::Deserialize;

pub enum DeleteFileError {
    SendFailed(ReqwestError),
    InvalidAuth,
    ExpiredAuth,
    FileNotFound,
    FileDeleted,
    Unspecified,
}

pub struct DeleteFileParams {
    pub username: String,
    pub auth: String,
    pub file_id: String,
}

#[derive(Deserialize)]
struct DeleteFileResponse {
    error_code: String,
}

impl From<ReqwestError> for DeleteFileError {
    fn from(e: ReqwestError) -> DeleteFileError {
        DeleteFileError::SendFailed(e)
    }
}

pub fn delete_file(params: &DeleteFileParams) -> Result<(), DeleteFileError> {
    let client = Client::new();
    let form_params = [
        ("username", params.username.as_str()),
        ("auth", params.auth.as_str()),
        ("file_id", params.file_id.as_str()),
    ];
    let mut response = client
        .delete(format!("{}/delete-file", API_LOC).as_str())
        .form(&form_params)
        .send()?;

    match (
        response.status().as_u16(),
        response.json::<DeleteFileResponse>()?.error_code.as_str(),
    ) {
        (200..=299, _) => Ok(()),
        (401, "invalid_auth") => Err(DeleteFileError::InvalidAuth),
        (401, "expired_auth") => Err(DeleteFileError::ExpiredAuth),
        (404, "file_not_found") => Err(DeleteFileError::FileNotFound),
        (410, "file_deleted") => Err(DeleteFileError::FileDeleted),
        _ => Err(DeleteFileError::Unspecified),
    }
}
