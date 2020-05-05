use crate::config::{config, ServerState};
use crate::index_db;
use lockbook_core::service::auth_service::{AuthService, AuthServiceImpl};
use lockbook_core::service::clock_service::ClockImpl;
use lockbook_core::service::crypto_service::RsaImpl;
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
    let mut locked_index_db_client = server_state.index_db_client.lock().unwrap();

    let get_updates_result =
        index_db::get_updates(&mut locked_index_db_client, &username, &version);
    match get_updates_result {
        Ok(updates) => Response::build()
            .status(Status::Ok)
            .sized_body(Cursor::new(
                serde_json::to_string(&updates).expect("Failed to json-serialize response!"),
            ))
            .finalize(),
        Err(_) => {
            println!("Internal server error! {:?}", get_updates_result);
            Response::build()
                .status(Status::InternalServerError)
                .finalize()
        }
    }
}
