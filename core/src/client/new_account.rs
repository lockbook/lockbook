use crate::model::api::{NewAccountError, NewAccountRequest, NewAccountResponse};
use reqwest::Client;
use reqwest::Error as ReqwestError;

#[derive(Debug)]
pub enum Error {
    SendFailed(ReqwestError),
    ReceiveFailed(ReqwestError),
    API(NewAccountError),
}

pub fn send(api_location: String, params: &NewAccountRequest) -> Result<NewAccountResponse, Error> {
    let client = Client::new();
    let form_params = [
        ("username", params.username.as_str()),
        ("auth", params.auth.as_str()),
        ("public_key", params.public_key.as_str()),
    ];
    let response = client
        .post(format!("{}/new-account", api_location).as_str())
        .form(&form_params)
        .send()
        .map_err(|err| Error::SendFailed(err))?
        .json::<Result<NewAccountResponse, NewAccountError>>()
        .map_err(|err| Error::ReceiveFailed(err))?;

    match response {
        Ok(r) => Ok(r),
        Err(e) => Err(Error::API(e)),
    }
}
