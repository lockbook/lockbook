use crate::config::ServerState;
use crate::index_db;

use rocket::http::Status;
use rocket::http::Header;
use rocket::{Response, State};

#[put("/get-public-key/<username>")] // TODO: should I create a wrapper for data?
pub fn get_public_key(server_state: State<ServerState>, username: String) -> Response {
    let mut locked_index_db_client = server_state.index_db_client.lock().unwrap();

    let get_public_key_result = index_db::get_public_key(&mut locked_index_db_client, &username);

    match get_public_key_result {
        Ok(public_key) => Response::build()
            .status(Status::Ok)
            .header(Header::new("public_key", public_key))
            .finalize(),
        Err(_) => Response::build().status(Status::NotFound).finalize(),
    }
}
