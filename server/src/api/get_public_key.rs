use crate::config::ServerState;
use crate::index_db;
use crate::index_db::get_public_key::Error;

use rocket::http::Status;
use rocket::{Response, State};
use std::io::Cursor;

#[get("/get-public-key/<username>")]
pub fn get_public_key(server_state: State<ServerState>, username: String) -> Response {
    let mut locked_index_db_client = server_state.index_db_client.lock().unwrap();

    let get_public_key_result = index_db::get_public_key(&mut locked_index_db_client, &username);

    match get_public_key_result {
        Ok(public_key) => Response::build()
            .status(Status::Ok)
            .sized_body(Cursor::new(
                serde_json::to_string(&public_key).expect("Failed to json-serialize response!"),
            ))
            .finalize(),
        Err(Error::Postgres(_)) => Response::build().status(Status::NotFound).finalize(),
        Err(Error::SerializationError(_)) => Response::build().status(Status::Conflict).finalize(),
    }
}
