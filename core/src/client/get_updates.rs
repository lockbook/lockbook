use crate::model::api::{GetUpdatesError, GetUpdatesRequest, GetUpdatesResponse};
use reqwest::Client;
use reqwest::Error as ReqwestError;

#[derive(Debug)]
pub enum Error {
    SendFailed(ReqwestError),
    ReceiveFailed(ReqwestError),
    API(GetUpdatesError),
}

pub fn send(api_location: String, params: &GetUpdatesRequest) -> Result<GetUpdatesResponse, Error> {
    let client = Client::new();
    let form_params = [
        ("username", params.username.as_str()),
        ("auth", params.auth.as_str()),
        ("since_version", "todo"),
    ];
    let response = client
        .delete(format!("{}/get-updates", api_location).as_str())
        .form(&form_params)
        .send()
        .map_err(|err| Error::SendFailed(err))?
        .json::<Result<GetUpdatesResponse, GetUpdatesError>>()
        .map_err(|err| Error::ReceiveFailed(err))?;

    match response {
        Ok(r) => Ok(r),
        Err(e) => Err(Error::API(e)),
    }
}
