use crate::model::api::{DeleteFileError, DeleteFileRequest, DeleteFileResponse};
use reqwest::Client;
use reqwest::Error as ReqwestError;

#[derive(Debug)]
pub enum Error {
    Serialize(serde_json::error::Error),
    SendFailed(ReqwestError),
    ReceiveFailed(ReqwestError),
    Deserialize(serde_json::error::Error),
    API(DeleteFileError),
}

pub fn send(
    api_location: String,
    request: &DeleteFileRequest,
) -> Result<DeleteFileResponse, Error> {
    let client = Client::new();
    let serialized_request = serde_json::to_string(&request).map_err(|e| Error::Serialize(e))?;
    let serialized_response = client
        .delete(format!("{}/delete-file", api_location).as_str())
        .body(serialized_request)
        .send()
        .map_err(|e| Error::SendFailed(e))?
        .text()
        .map_err(|e| Error::ReceiveFailed(e))?;
    let response = serde_json::from_str(&serialized_response).map_err(|e| Error::Deserialize(e))?;

    match response {
        Ok(r) => Ok(r),
        Err(e) => Err(Error::API(e)),
    }
}
