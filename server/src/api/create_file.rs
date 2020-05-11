use crate::ServerState;
use hyper::{body, Body, Request, Response};
use std::sync::{Arc, Mutex};

#[derive(Debug)]
pub enum Error<'a> {
    HyperBodyToBytes(hyper::Error),
    HyperBodyBytesToString(std::string::FromUtf8Error),
    MutexLock(std::sync::PoisonError<std::sync::MutexGuard<'a, ServerState>>),
    JsonDeserialize(serde_json::error::Error),
    CreateFile(lockbook_core::model::api::CreateFileError),
    JsonSerialize(serde_json::error::Error),
}

impl<'a> From<hyper::Error> for Error<'a> {
    fn from(error: hyper::Error) -> Error<'a> {
        Error::HyperBodyToBytes(error)
    }
}

impl<'a> From<std::string::FromUtf8Error> for Error<'a> {
    fn from(error: std::string::FromUtf8Error) -> Error<'a> {
        Error::HyperBodyBytesToString(error)
    }
}

impl<'a> From<std::sync::PoisonError<std::sync::MutexGuard<'a, ServerState>>> for Error<'a> {
    fn from(error: std::sync::PoisonError<std::sync::MutexGuard<'a, ServerState>>) -> Error<'a> {
        Error::MutexLock(error)
    }
}

impl<'a> From<lockbook_core::model::api::CreateFileError> for Error<'a> {
    fn from(error: lockbook_core::model::api::CreateFileError) -> Error<'a> {
        Error::CreateFile(error)
    }
}

pub async fn handle<'a>(
    server_state: Arc<Mutex<ServerState>>,
    req: Request<Body>,
) -> Result<Response<Body>, Error<'a>> {
    let locked_server_state = server_state.lock()?;
    let body_bytes = body::to_bytes(req.into_body()).await?;
    let body_string = String::from_utf8(body_bytes.to_vec())?;
    let request = serde_json::from_str(&body_string).map_err(|e| Error::JsonDeserialize(e))?;
    let response = crate::services::create_file::create_file(&mut locked_server_state, request)?;
    let response_body = serde_json::to_string(&response).map_err(|e| Error::JsonSerialize(e))?;

    Ok(Response::new(response_body.into()))
}
