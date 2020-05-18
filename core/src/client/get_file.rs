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

pub fn send(bucket_location: String, file_id: String) -> Result<EncryptedFile, Error> {
    let client = Client::new();
    let resource = format!("{}/{}", bucket_location, file_id.as_str());
    let response = client
        .get(resource.as_str())
        .send()
        .map_err(|err| Error::SendFailed(err))?;

    let status_code = response.status().as_u16();
    let response_body = response.text().map_err(|err| Error::ReceiveFailed(err))?;
    let encrypted_file: EncryptedFile =
        serde_json::from_str(response_body.as_str()).map_err(|err| Error::SerdeError(err))?;
    match status_code {
        200..=299 => Ok(encrypted_file),
        _ => Err(Error::Unspecified),
    }
}
