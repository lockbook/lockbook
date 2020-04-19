use crate::service::file_encryption_service::EncryptedFile;
use reqwest::Client;
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
    let mut response = client
        .get(resource.as_str())
        .send()
        .map_err(|err| GetFileError::SendFailed(err))?;

    let response_body = response
        .text()
        .map_err(|err| GetFileError::ReceiveFailed(err))?;
    let encrypted_file: EncryptedFile = serde_json::from_str(response_body.as_str())
        .map_err(|err| GetFileError::SerdeError(err))?;
    match response.status().as_u16() {
        200..=299 => Ok(encrypted_file),
        _ => Err(GetFileError::Unspecified),
    }
}
