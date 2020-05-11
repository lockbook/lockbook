use crate::server::ServerState;
use hyper::{body, Body, StatusCode};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::sync::{Arc, Mutex};

pub trait EndpointService<Request, Response, Error> {
    fn handle(server_state: &mut ServerState, request: Request) -> Result<Response, Error>;
}

pub trait Endpoint<Request: DeserializeOwned, Response: Serialize, Error: Serialize> {
    fn handle(
        server_state: Arc<Mutex<ServerState>>,
        req: hyper::Request<Body>,
    ) -> hyper::Response<Body>;
}

impl<
        Request: DeserializeOwned,
        Response: Serialize,
        Error: Serialize,
        Service: EndpointService<Request, Response, Error>,
    > Endpoint<Request, Response, Error> for Service
{
    fn handle(
        server_state: Arc<Mutex<ServerState>>,
        req: hyper::Request<Body>,
    ) -> hyper::Response<Body> {
        // TODO: log successes and errors
        match server_state
            .lock()
            .map_err(|e| HandleError::MutexLock(e))
            .and_then(|mut locked_server_state| {
                handle_helper::<Request, Response, Error, Service>(&mut locked_server_state, req)
            }) {
            Ok(response) => response,
            Err(err) => {
                let mut response = hyper::Response::default();
                *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
                response
            }
        }
    }
}

#[derive(Debug)]
enum HandleError<'a> {
    HyperBodyToBytes(hyper::Error),
    HyperBodyBytesToString(std::string::FromUtf8Error),
    MutexLock(std::sync::PoisonError<std::sync::MutexGuard<'a, ServerState>>),
    JsonDeserialize(serde_json::error::Error),
    JsonSerialize(serde_json::error::Error),
}

impl<'a> From<hyper::Error> for HandleError<'a> {
    fn from(error: hyper::Error) -> HandleError<'a> {
        HandleError::HyperBodyToBytes(error)
    }
}

impl<'a> From<std::string::FromUtf8Error> for HandleError<'a> {
    fn from(error: std::string::FromUtf8Error) -> HandleError<'a> {
        HandleError::HyperBodyBytesToString(error)
    }
}

impl<'a> From<std::sync::PoisonError<std::sync::MutexGuard<'a, ServerState>>> for HandleError<'a> {
    fn from(
        error: std::sync::PoisonError<std::sync::MutexGuard<'a, ServerState>>,
    ) -> HandleError<'a> {
        HandleError::MutexLock(error)
    }
}

fn handle_helper<
    'a,
    Request: DeserializeOwned,
    Response: Serialize,
    Error: Serialize,
    Service: EndpointService<Request, Response, Error>,
>(
    server_state: &mut ServerState,
    req: hyper::Request<Body>,
) -> Result<hyper::Response<Body>, HandleError<'a>> {
    let body_bytes = body_to_bytes(req)?;
    let body_string = String::from_utf8(body_bytes.to_vec())?;
    let request =
        serde_json::from_str(&body_string).map_err(|e| HandleError::JsonDeserialize(e))?;
    let response = Service::handle(server_state, request);
    let response_body =
        serde_json::to_string(&response).map_err(|e| HandleError::JsonSerialize(e))?;

    Ok(hyper::Response::new(response_body.into()))
}

#[tokio::main]
async fn body_to_bytes(req: hyper::Request<Body>) -> Result<hyper::body::Bytes, hyper::Error> {
    body::to_bytes(req.into_body()).await
}
