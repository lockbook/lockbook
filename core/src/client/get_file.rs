use crate::service::file_encryption_service::EncryptedFile;
use reqwest::blocking::Client;
use reqwest::Error as ReqwestError;

#[derive(Debug)]
pub enum GetFileError {
    SendFailed(ReqwestError),
    ReceiveFailed(ReqwestError),
    SerdeError(serde_json::Error),
    Unspecified,
}

pub struct GetFileRequest {
    pub file_id: String,
}

pub fn get_file(
    bucket_location: String,
    params: &GetFileRequest,
) -> Result<EncryptedFile, GetFileError> {
    let client = Client::new();
    let resource = format!("{}/{}", bucket_location, params.file_id.as_str());
    let response = client
        .get(resource.as_str())
        .send()
        .map_err(GetFileError::SendFailed)?;

    let status = response.status();
    let response_body = response
        .text()
        .map_err(GetFileError::ReceiveFailed)?;
    let encrypted_file: EncryptedFile = serde_json::from_str(response_body.as_str())
        .map_err(GetFileError::SerdeError)?;
    match status.as_u16() {
        200..=299 => Ok(encrypted_file),
        _ => Err(GetFileError::Unspecified),
    }
}
