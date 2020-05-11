use crate::ServerState;
use hyper::{Body, Request, Response};
use std::sync::Arc;

pub fn handle(server_state: Arc<ServerState>, req: Request<Body>) -> Response<Body> {
    Response::new(req.into_body())
}
