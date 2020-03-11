use reqwest::Client;
use reqwest::Error as ReqwestError;
use crate::API_LOC;
use serde::Deserialize;

pub enum MoveFileError {
    SendFailed(ReqwestError),
    InvalidAuth,
    ExpiredAuth,
    FileNotFound,
    FileDeleted,
    FilePathTaken,
    Unspecified,
}

pub struct MoveFileParams {
    pub username: String,
    pub auth: String,
    pub file_id: String,
    pub new_file_path: String,
}

#[derive(Deserialize)]
struct MoveFileResponse {
    error_code: String,
}

impl From<ReqwestError> for MoveFileError {
    fn from(e: ReqwestError) -> MoveFileError {
        MoveFileError::SendFailed(e)
    }
}

pub fn move_file(params: &MoveFileParams) -> Result<(), MoveFileError> {
    let client = Client::new();
    let form_params = [
        ("username", params.username.as_str()),
        ("auth", params.auth.as_str()),
        ("file_id", params.file_id.as_str()),
        ("new_file_path", params.new_file_path.as_str()),
    ];
    let mut response = client
        .put(format!("{}/move-file", API_LOC).as_str())
        .form(&form_params)
        .send()?;

    match (response.status().as_u16(), response.json::<MoveFileResponse>()?.error_code.as_str()) {
        (200..=299, _) => Ok(()),
        (401, "invalid_auth") => Err(MoveFileError::InvalidAuth),
        (401, "expired_auth") => Err(MoveFileError::ExpiredAuth),
        (404, "file_not_found") => Err(MoveFileError::FileNotFound),
        (410, "file_deleted") => Err(MoveFileError::FileDeleted),
        (422, "file_path_taken") => Err(MoveFileError::FilePathTaken),
        _ => Err(MoveFileError::Unspecified),
    }
}