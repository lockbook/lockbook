use crate::endpoint::Endpoint;
use crate::server::ServerState;
use hyper::{Body, Method, Request, Response, StatusCode};
use lockbook_core::model::api::{
    ChangeFileContentError, ChangeFileContentRequest, ChangeFileContentResponse,
};
use lockbook_core::model::api::{CreateFileError, CreateFileRequest, CreateFileResponse};
use lockbook_core::model::api::{DeleteFileError, DeleteFileRequest, DeleteFileResponse};
use lockbook_core::model::api::{GetUpdatesError, GetUpdatesRequest, GetUpdatesResponse};
use lockbook_core::model::api::{MoveFileError, MoveFileRequest, MoveFileResponse};
use lockbook_core::model::api::{NewAccountError, NewAccountRequest, NewAccountResponse};
use lockbook_core::model::api::{RenameFileError, RenameFileRequest, RenameFileResponse};
use lockbook_core::service::logging_service::Logger;
use std::marker::PhantomData;
use std::sync::{Arc, Mutex};

pub trait Api {
    fn handle(server_state: Arc<Mutex<ServerState>>, req: Request<Body>) -> Response<Body>;
}

pub struct ApiImpl<
    LoggerImpl: Logger,
    ChangeFileContentEndpoint: Endpoint<ChangeFileContentRequest, ChangeFileContentResponse, ChangeFileContentError>,
    CreateFileEndpoint: Endpoint<CreateFileRequest, CreateFileResponse, CreateFileError>,
    DeleteFileEndpoint: Endpoint<DeleteFileRequest, DeleteFileResponse, DeleteFileError>,
    GetUpdatesEndpoint: Endpoint<GetUpdatesRequest, GetUpdatesResponse, GetUpdatesError>,
    MoveFileEndpoint: Endpoint<MoveFileRequest, MoveFileResponse, MoveFileError>,
    NewAccountEndpoint: Endpoint<NewAccountRequest, NewAccountResponse, NewAccountError>,
    RenameFileEndpoint: Endpoint<RenameFileRequest, RenameFileResponse, RenameFileError>,
> {
    logger: PhantomData<LoggerImpl>,
    change_file_content: PhantomData<ChangeFileContentEndpoint>,
    create_file: PhantomData<CreateFileEndpoint>,
    delete_file: PhantomData<DeleteFileEndpoint>,
    get_updates: PhantomData<GetUpdatesEndpoint>,
    move_file: PhantomData<MoveFileEndpoint>,
    new_account: PhantomData<NewAccountEndpoint>,
    rename_file: PhantomData<RenameFileEndpoint>,
}

impl<
        LoggerImpl: Logger,
        ChangeFileContentEndpoint: Endpoint<ChangeFileContentRequest, ChangeFileContentResponse, ChangeFileContentError>,
        CreateFileEndpoint: Endpoint<CreateFileRequest, CreateFileResponse, CreateFileError>,
        DeleteFileEndpoint: Endpoint<DeleteFileRequest, DeleteFileResponse, DeleteFileError>,
        GetUpdatesEndpoint: Endpoint<GetUpdatesRequest, GetUpdatesResponse, GetUpdatesError>,
        MoveFileEndpoint: Endpoint<MoveFileRequest, MoveFileResponse, MoveFileError>,
        NewAccountEndpoint: Endpoint<NewAccountRequest, NewAccountResponse, NewAccountError>,
        RenameFileEndpoint: Endpoint<RenameFileRequest, RenameFileResponse, RenameFileError>,
    > Api
    for ApiImpl<
        LoggerImpl,
        ChangeFileContentEndpoint,
        CreateFileEndpoint,
        DeleteFileEndpoint,
        GetUpdatesEndpoint,
        MoveFileEndpoint,
        NewAccountEndpoint,
        RenameFileEndpoint,
    >
{
    fn handle(server_state: Arc<Mutex<ServerState>>, req: Request<Body>) -> Response<Body> {
        // TODO: logging
        match (req.method(), req.uri().path()) {
            (&Method::PUT, "/change-file-content") => {
                ChangeFileContentEndpoint::handle(server_state, req)
            }
            (&Method::POST, "/create-file") => CreateFileEndpoint::handle(server_state, req),
            (&Method::DELETE, "/delete-file") => DeleteFileEndpoint::handle(server_state, req),
            (&Method::GET, "/get-updates") => GetUpdatesEndpoint::handle(server_state, req),
            (&Method::PUT, "/move-file") => MoveFileEndpoint::handle(server_state, req),
            (&Method::POST, "/new-account") => NewAccountEndpoint::handle(server_state, req),
            (&Method::PUT, "/rename-file") => RenameFileEndpoint::handle(server_state, req),
            _ => {
                let mut response = Response::default();
                *response.status_mut() = StatusCode::NOT_FOUND;
                response
            }
        }
    }
}
