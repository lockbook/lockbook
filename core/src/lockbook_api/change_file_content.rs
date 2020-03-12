use crate::API_LOC;
use reqwest::Client;
use reqwest::Error as ReqwestError;
use serde::Deserialize;

pub enum ChangeFileContentError {
    SendFailed(ReqwestError),
    InvalidAuth,
    ExpiredAuth,
    FileNotFound,
    EditConflict(u64),
    FileDeleted,
    Unspecified,
}

pub struct ChangeFileContentParams {
    pub username: String,
    pub auth: String,
    pub file_id: String,
    pub old_file_version: u64,
    pub new_file_content: String,
}

#[derive(Deserialize)]
struct ChangeFileContentResponse {
    error_code: String,
    current_version: u64,
}

impl From<ReqwestError> for ChangeFileContentError {
    fn from(e: ReqwestError) -> ChangeFileContentError {
        ChangeFileContentError::SendFailed(e)
    }
}

pub fn change_file_content(params: &ChangeFileContentParams) -> Result<(), ChangeFileContentError> {
    let client = Client::new();
    let form_params = [
        ("username", params.username.as_str()),
        ("auth", params.auth.as_str()),
        ("file_id", params.file_id.as_str()),
        ("old_file_version", &params.old_file_version.to_string()),
        ("new_file_content", params.new_file_content.as_str()),
    ];
    let mut response = client
        .put(format!("{}/change-file-content", API_LOC).as_str())
        .form(&form_params)
        .send()?;

    let response_body = response.json::<ChangeFileContentResponse>()?;

    match (
        response.status().as_u16(),
        response_body.error_code.as_str(),
    ) {
        (200..=299, _) => Ok(()),
        (401, "invalid_auth") => Err(ChangeFileContentError::InvalidAuth),
        (401, "expired_auth") => Err(ChangeFileContentError::ExpiredAuth),
        (404, "file_not_found") => Err(ChangeFileContentError::FileNotFound),
        (409, "edit_conflict") => Err(ChangeFileContentError::EditConflict(
            response_body.current_version,
        )),
        (410, "file_deleted") => Err(ChangeFileContentError::FileDeleted),
        _ => Err(ChangeFileContentError::Unspecified),
    }
}
