use crate::model::api::{
    ChangeFileContentError, ChangeFileContentRequest, ChangeFileContentResponse,
};
use reqwest::Client;
use reqwest::Error as ReqwestError;

#[derive(Debug)]
pub enum Error {
    SendFailed(ReqwestError),
    ReceiveFailed(ReqwestError),
    API(ChangeFileContentError),
}

pub fn send(
    api_location: String,
    params: &ChangeFileContentRequest,
) -> Result<ChangeFileContentResponse, Error> {
    let client = Client::new();
    let form_params = [
        ("username", params.username.as_str()),
        ("auth", params.auth.as_str()),
        ("file_id", params.file_id.as_str()),
        ("old_file_version", &params.old_file_version.to_string()),
        ("new_file_content", params.new_file_content.as_str()),
    ];
    let response = client
        .put(format!("{}/change-file-content", api_location).as_str())
        .form(&form_params)
        .send()
        .map_err(|err| Error::SendFailed(err))?
        .json::<Result<ChangeFileContentResponse, ChangeFileContentError>>()
        .map_err(|err| Error::ReceiveFailed(err))?;

    match response {
        Ok(r) => Ok(r),
        Err(e) => Err(Error::API(e)),
    }
}
