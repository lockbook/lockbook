use reqwest::Client;
use reqwest::Error as ReqwestError;
use serde::{Deserialize, Serialize};

#[derive(Debug)]
pub enum ChangeFileContentError {
    SendFailed(ReqwestError),
    ReceiveFailed(ReqwestError),
    InvalidAuth,
    ExpiredAuth,
    FileNotFound,
    EditConflict(u64),
    FileDeleted,
    Unspecified,
}

pub struct ChangeFileContentRequest {
    pub username: String,
    pub auth: String,
    pub file_id: String,
    pub old_file_version: u64,
    pub new_file_content: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct ChangeFileContentResponse {
    pub error_code: String,
    pub current_version: u64,
}

pub trait FileContentClient {
    fn change_file_content(
        api_location: String,
        params: &ChangeFileContentRequest,
    ) -> Result<u64, ChangeFileContentError>;
}

pub struct FileContentClientImpl;

impl FileContentClient for FileContentClientImpl {
    fn change_file_content(
        api_location: String,
        params: &ChangeFileContentRequest,
    ) -> Result<u64, ChangeFileContentError> {
        let client = Client::new();
        let form_params = [
            ("username", params.username.as_str()),
            ("auth", params.auth.as_str()),
            ("file_id", params.file_id.as_str()),
            ("old_file_version", &params.old_file_version.to_string()),
            ("new_file_content", params.new_file_content.as_str()),
        ];
        let mut response = client
            .put(format!("{}/change-file-content", api_location).as_str())
            .form(&form_params)
            .send()
            .map_err(|err| ChangeFileContentError::SendFailed(err))?;

        let response_body = response
            .json::<ChangeFileContentResponse>()
            .map_err(|err| ChangeFileContentError::ReceiveFailed(err))?;

        match (
            response.status().as_u16(),
            response_body.error_code.as_str(),
        ) {
            (200..=299, _) => Ok(response_body.current_version),
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
}