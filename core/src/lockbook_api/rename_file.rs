use reqwest::Client;
use reqwest::Error as ReqwestError;
use serde::Deserialize;

#[derive(Debug)]
pub enum RenameFileError {
    SendFailed(ReqwestError),
    ReceiveFailed(ReqwestError),
    InvalidAuth,
    ExpiredAuth,
    FileNotFound,
    FileDeleted,
    Unspecified,
}

pub struct RenameFileParams {
    pub username: String,
    pub auth: String,
    pub file_id: String,
    pub file_name: String,
    pub file_path: String,
    pub file_content: String,
}

#[derive(Deserialize)]
struct RenameFileResponse {
    error_code: String,
}

pub fn rename_file(api_location: &str, params: &RenameFileParams) -> Result<(), RenameFileError> {
    let client = Client::new();
    let form_params = [
        ("username", params.username.as_str()),
        ("auth", params.auth.as_str()),
        ("file_id", params.file_id.as_str()),
        ("file_name", params.file_name.as_str()),
        ("file_path", params.file_path.as_str()),
        ("file_content", params.file_content.as_str()),
    ];
    let mut response = client
        .put(format!("{}/rename-file", api_location).as_str())
        .form(&form_params)
        .send()
        .map_err(|err| RenameFileError::SendFailed(err))?;

    match (
        response.status().as_u16(),
        response
            .json::<RenameFileResponse>()
            .map_err(|err| RenameFileError::ReceiveFailed(err))?
            .error_code
            .as_str(),
    ) {
        (200..=299, _) => Ok(()),
        (401, "invalid_auth") => Err(RenameFileError::InvalidAuth),
        (401, "expired_auth") => Err(RenameFileError::ExpiredAuth),
        (404, "file_not_found") => Err(RenameFileError::FileNotFound),
        (410, "file_deleted") => Err(RenameFileError::FileDeleted),
        _ => Err(RenameFileError::Unspecified),
    }
}
