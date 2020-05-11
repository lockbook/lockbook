use crate::services::change_file_content::change_file_content;
use crate::ServerState;
use hyper::{Body, Request, Response};
use std::sync::Arc;

pub fn handle(server_state: Arc<ServerState>, req: Request<Body>) -> Response<Body> {
    String::from_utf8(body::to_bytes(req.into_body()).await.unwrap().to_vec()).unwrap();

    Response::new("".into())
}
