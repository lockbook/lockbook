use crate::model::api::{DeleteFileError, DeleteFileRequest, DeleteFileResponse};
use reqwest::Client;
use reqwest::Error as ReqwestError;

#[derive(Debug)]
pub enum Error {
    SendFailed(ReqwestError),
    ReceiveFailed(ReqwestError),
    API(DeleteFileError),
}

pub fn send(api_location: String, params: &DeleteFileRequest) -> Result<DeleteFileResponse, Error> {
    let client = Client::new();
    let form_params = [
        ("username", params.username.as_str()),
        ("auth", params.auth.as_str()),
        ("file_id", params.file_id.as_str()),
    ];
    let response = client
        .delete(format!("{}/delete-file", api_location).as_str())
        .form(&form_params)
        .send()
        .map_err(|err| Error::SendFailed(err))?
        .json::<Result<DeleteFileResponse, DeleteFileError>>()
        .map_err(|err| Error::ReceiveFailed(err))?;

    match response {
        Ok(r) => Ok(r),
        Err(e) => Err(Error::API(e)),
    }
}
