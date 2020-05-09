use crate::model::api::{MoveFileError, MoveFileRequest, MoveFileResponse};
use reqwest::Client;
use reqwest::Error as ReqwestError;

#[derive(Debug)]
pub enum Error {
    SendFailed(ReqwestError),
    ReceiveFailed(ReqwestError),
    API(MoveFileError),
}

pub fn send(api_location: String, params: &MoveFileRequest) -> Result<MoveFileResponse, Error> {
    let client = Client::new();
    let form_params = [
        ("username", params.username.as_str()),
        ("auth", params.auth.as_str()),
        ("file_id", params.file_id.as_str()),
        ("new_file_path", params.new_file_path.as_str()),
    ];
    let response = client
        .put(format!("{}/move-file", api_location).as_str())
        .form(&form_params)
        .send()
        .map_err(|err| Error::SendFailed(err))?
        .json::<Result<MoveFileResponse, MoveFileError>>()
        .map_err(|err| Error::ReceiveFailed(err))?;

    match response {
        Ok(r) => Ok(r),
        Err(e) => Err(Error::API(e)),
    }
}
