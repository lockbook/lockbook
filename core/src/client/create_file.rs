use crate::model::api::{CreateFileError, CreateFileRequest, CreateFileResponse};
use reqwest::Client;
use reqwest::Error as ReqwestError;

#[derive(Debug)]
pub enum Error {
    SendFailed(ReqwestError),
    ReceiveFailed(ReqwestError),
    API(CreateFileError),
}

pub fn send(api_location: String, params: &CreateFileRequest) -> Result<CreateFileResponse, Error> {
    let client = Client::new();
    let form_params = [
        ("username", params.username.as_str()),
        ("auth", params.auth.as_str()),
        ("file_id", params.file_id.as_str()),
        ("file_name", params.file_name.as_str()),
        ("file_path", params.file_path.as_str()),
        ("file_content", params.file_content.as_str()),
    ];
    let response = client
        .post(format!("{}/create-file", api_location).as_str())
        .form(&form_params)
        .send()
        .map_err(|err| Error::SendFailed(err))?
        .json::<Result<CreateFileResponse, CreateFileError>>()
        .map_err(|err| Error::ReceiveFailed(err))?;

    match response {
        Ok(r) => Ok(r),
        Err(e) => Err(Error::API(e)),
    }
}
