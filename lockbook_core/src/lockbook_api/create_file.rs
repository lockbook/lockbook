use reqwest::Client;
use reqwest::Error as ReqwestError;
use crate::API_LOC;
use serde::Deserialize;

pub enum CreateFileError {
    SendFailed(ReqwestError),
    InvalidAuth,
    ExpiredAuth,
    FileIdTaken,
    FilePathTaken,
    Unspecified,
}

pub struct CreateFileParams {
    pub username: String,
    pub auth: String,
    pub file_id: String,
    pub file_name: String,
    pub file_path: String,
    pub file_content: String,
}

#[derive(Deserialize)]
struct CreateFileResponse {
    error_code: String
}

impl From<ReqwestError> for CreateFileError {
    fn from(e: ReqwestError) -> CreateFileError {
        CreateFileError::SendFailed(e)
    }
}

pub fn create_file(params: &CreateFileParams) -> Result<(), CreateFileError> {
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
        .post(format!("{}/create-file", API_LOC).as_str())
        .form(&form_params)
        .send()?;

    match (response.status().as_u16(), response.json::<CreateFileResponse>()?.error_code.as_str()) {
        (200..=299, _) => Ok(()),
        (401, "invalid_auth") => Err(CreateFileError::InvalidAuth),
        (401, "expired_auth") => Err(CreateFileError::ExpiredAuth),
        (422, "file_id_taken") => Err(CreateFileError::InvalidAuth),
        (422, "file_path_taken") => Err(CreateFileError::ExpiredAuth),
        _ => Err(CreateFileError::Unspecified),
    }
}