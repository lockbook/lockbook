use crate::model::api::{RenameFileError, RenameFileRequest, RenameFileResponse};
use reqwest::Client;
use reqwest::Error as ReqwestError;

#[derive(Debug)]
pub enum Error {
    SendFailed(ReqwestError),
    ReceiveFailed(ReqwestError),
    API(RenameFileError),
}

pub fn send(api_location: String, params: &RenameFileRequest) -> Result<RenameFileResponse, Error> {
    let client = Client::new();
    let form_params = [
        ("username", params.username.as_str()),
        ("auth", params.auth.as_str()),
        ("file_id", params.file_id.as_str()),
        ("new_file_name", params.new_file_name.as_str()),
    ];
    let response = client
        .put(format!("{}/rename-file", api_location).as_str())
        .form(&form_params)
        .send()
        .map_err(|err| Error::SendFailed(err))?
        .json::<Result<RenameFileResponse, RenameFileError>>()
        .map_err(|err| Error::ReceiveFailed(err))?;

    match response {
        Ok(r) => Ok(r),
        Err(e) => Err(Error::API(e)),
    }
}
