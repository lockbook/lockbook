use crate::service::file_encryption_service::EncryptedFile;
use reqwest::blocking::Client;
use reqwest::Error as ReqwestError;

#[derive(Debug)]
pub enum Error {
    SendFailed(ReqwestError),
    ReceiveFailed(ReqwestError),
    SerdeError(serde_json::Error),
    Unspecified,
}

pub fn send(
    bucket_location: String,
    file_id: String,
    file_content_version: u64,
) -> Result<EncryptedFile, Error> {
    let client = Client::new();
    let resource = format!("{}/{}:{}", bucket_location, &file_id, file_content_version);
    let response = client
        .get(resource.as_str())
        .send()
        .map_err(Error::SendFailed)?;

    let status_code = response.status().as_u16();
    let response_body = response.text().map_err(Error::ReceiveFailed)?;
    let encrypted_file: EncryptedFile =
        serde_json::from_str(response_body.as_str()).map_err(Error::SerdeError)?;
    match status_code {
        200..=299 => Ok(encrypted_file),
        _ => Err(Error::Unspecified),
    }
}
