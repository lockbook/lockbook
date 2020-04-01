use reqwest::Client;
use reqwest::Error as ReqwestError;
use serde::Deserialize;

#[derive(Debug)]
pub enum CreateFileError {
    SendFailed(ReqwestError),
    ReceiveFailed(ReqwestError),
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
    error_code: String,
    current_version: u64,
}

pub fn create_file(api_location: String, params: &CreateFileParams) -> Result<u64, CreateFileError> {
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
        .post(format!("{}/create-file", api_location).as_str())
        .form(&form_params)
        .send()
        .map_err(|err| CreateFileError::SendFailed(err))?;

    let response_body = response
        .json::<CreateFileResponse>()
        .map_err(|err| CreateFileError::ReceiveFailed(err))?;

    match (
        response.status().as_u16(),
        response_body.error_code.as_str(),
    ) {
        (200..=299, _) => Ok(response_body.current_version),
        (401, "invalid_auth") => Err(CreateFileError::InvalidAuth),
        (401, "expired_auth") => Err(CreateFileError::ExpiredAuth),
        (422, "file_id_taken") => Err(CreateFileError::InvalidAuth),
        (422, "file_path_taken") => Err(CreateFileError::ExpiredAuth),
        _ => Err(CreateFileError::Unspecified),
    }
}
