use crate::model::file::File;
use reqwest::Client;
use reqwest::Error as ReqwestError;

#[derive(Debug)]
pub enum GetFileError {
    SendFailed(ReqwestError),
    ReceiveFailed(ReqwestError),
    Unspecified,
}

pub struct GetFileRequest {
    pub file_id: String,
}

pub fn get_file(bucket_location: String, params: &GetFileRequest) -> Result<File, GetFileError> {
    let client = Client::new();
    let resource = format!("{}/{}", bucket_location, params.file_id.as_str());
    let mut response = client
        .get(resource.as_str())
        .send()
        .map_err(|err| GetFileError::SendFailed(err))?;

    let response_body = response
        .text()
        .map_err(|err| GetFileError::ReceiveFailed(err))?;
    match response.status().as_u16() {
        200..=299 => Ok(File {
            id: format!("{}", &params.file_id),
            content: response_body,
        }),
        _ => Err(GetFileError::Unspecified),
    }
}
