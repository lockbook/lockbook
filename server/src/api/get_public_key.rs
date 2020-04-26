use crate::config::ServerState;
use crate::index_db;

use rocket::http::Status;
use rocket::{Response, State};
use std::io::Cursor;

#[put("/get-public-key/<username>")] // TODO: should I create a wrapper for data?
pub fn get_public_key(server_state: State<ServerState>, username: String) -> Response {
    let mut locked_index_db_client = server_state.index_db_client.lock().unwrap();

    let get_public_key_result = index_db::get_public_key(&mut locked_index_db_client, &username);

    match get_public_key_result {
        Ok(public_key) => Response::build()
            .status(Status::Ok)
            .sized_body(Cursor::new(public_key))
            .finalize(),
        Err(e) => Response::build().status(Status::NotFound).finalize(),
    }
}
