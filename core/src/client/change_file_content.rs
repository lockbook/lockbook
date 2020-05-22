use crate::model::api::{
    ChangeFileContentError, ChangeFileContentRequest, ChangeFileContentResponse,
};
use reqwest::blocking::Client;
use reqwest::Error as ReqwestError;

#[derive(Debug)]
pub enum Error {
    Serialize(serde_json::error::Error),
    SendFailed(ReqwestError),
    ReceiveFailed(ReqwestError),
    Deserialize(serde_json::error::Error),
    API(ChangeFileContentError),
}

pub fn send(
    api_location: String,
    request: &ChangeFileContentRequest,
) -> Result<ChangeFileContentResponse, Error> {
    let client = Client::new();
    let serialized_request = serde_json::to_string(&request).map_err(Error::Serialize)?;
    let serialized_response = client
        .put(format!("{}/change-file-content", api_location).as_str())
        .body(serialized_request)
        .send()
        .map_err(Error::SendFailed)?
        .text()
        .map_err(Error::ReceiveFailed)?;
    let response = serde_json::from_str(&serialized_response).map_err(Error::Deserialize)?;

    match response {
        Ok(r) => Ok(r),
        Err(e) => Err(Error::API(e)),
    }
}
