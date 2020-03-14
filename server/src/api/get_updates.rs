use crate::config::ServerState;
use crate::index_db;
use rocket::http::Status;
use rocket::Response;
use rocket::State;
use std::io::Cursor;

#[get("/get-updates/<username>/<auth>/<version>")]
pub fn get_updates(
    server_state: State<ServerState>,
    username: String,
    auth: String,
    version: i64,
) -> Response {
    println!("get_updates: {:?}, {:?}, {:?}, ", username, auth, version);

    let mut locked_index_db_client = server_state.index_db_client.lock().unwrap();

    match index_db::get_updates(&mut locked_index_db_client, &username, &version) {
        Ok(updates) => Response::build()
            .status(Status::Ok)
            .sized_body(Cursor::new(
                serde_json::to_string(&updates).expect("Failed to json-serialize response!"),
            ))
            .finalize(),
        Err(_) => Response::build()
            .status(Status::InternalServerError)
            .finalize(),
    }
}
