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

    let public_key = match index_db::get_public_key(&mut locked_index_db_client, &username) {
        Ok(public_key) => public_key,
        Err(_) => return Response::build().status(Status::NotFound).finalize(),
    };

    if let Err(e) = AuthServiceImpl::<ClockImpl, RsaImpl>::verify_auth(
        &auth,
        &serde_json::from_str(&public_key).unwrap(),
        &username,
        config().auth_config.max_auth_delay.parse().unwrap(), //TODO: don't unwrap
    ) {
        println!(
            "Auth failed for: {}, {}, {}, {:?}",
            username, auth, public_key, e
        );
        return Response::build().status(Status::Unauthorized).finalize();
    }

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
