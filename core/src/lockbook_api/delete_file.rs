use reqwest::Client;
use reqwest::Error as ReqwestError;
use serde::Deserialize;

#[derive(Debug)]
pub enum DeleteFileError {
    SendFailed(ReqwestError),
    ReceiveFailed(ReqwestError),
    InvalidAuth,
    ExpiredAuth,
    FileNotFound,
    FileDeleted,
    Unspecified,
}

pub struct DeleteFileRequest {
    pub username: String,
    pub auth: String,
    pub file_id: String,
}

#[derive(Deserialize)]
pub struct DeleteFileResponse {
    error_code: String,
}

pub fn delete_file(
    api_location: String,
    params: &DeleteFileRequest,
) -> Result<(), DeleteFileError> {
    let client = Client::new();
    let form_params = [
        ("username", params.username.as_str()),
        ("auth", params.auth.as_str()),
        ("file_id", params.file_id.as_str()),
    ];
    let mut response = client
        .delete(format!("{}/delete-file", api_location).as_str())
        .form(&form_params)
        .send()
        .map_err(|err| DeleteFileError::SendFailed(err))?;

    match (
        response.status().as_u16(),
        response
            .json::<DeleteFileResponse>()
            .map_err(|err| DeleteFileError::ReceiveFailed(err))?
            .error_code
            .as_str(),
    ) {
        (200..=299, _) => Ok(()),
        (401, "invalid_auth") => Err(DeleteFileError::InvalidAuth),
        (401, "expired_auth") => Err(DeleteFileError::ExpiredAuth),
        (404, "file_not_found") => Err(DeleteFileError::FileNotFound),
        (410, "file_deleted") => Err(DeleteFileError::FileDeleted),
        _ => Err(DeleteFileError::Unspecified),
    }
}
