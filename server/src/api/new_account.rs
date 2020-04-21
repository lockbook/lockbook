use crate::config::ServerState;
use crate::index_db;
use rocket::http::Status;
use rocket::request::Form;
use rocket::Response;
use rocket::State;
use std::io::Cursor;
use lockbook_core::client::NewAccountResponse;
use lockbook_core::auth_service::{AuthServiceImpl, AuthService};
use lockbook_core::crypto::RsaCryptoService;
use lockbook_core::clock::ClockImpl;

#[derive(FromForm, Debug)]
pub struct NewAccount {
    pub username: String,
    pub c: String,
    pub public_key: String,
}

#[post("/new-account", data = "<new_account>")]
pub fn new_account(server_state: State<ServerState>, new_account: Form<NewAccount>) -> Response {
    let mut locked_index_db_client = server_state.index_db_client.lock().unwrap();

    if let Err(e) = AuthServiceImpl::<ClockImpl, RsaCryptoService>::verify_auth(
        &new_account.auth,
        &new_account.public_key,
        &new_account.username
    ) {
        return make_response(401, "failed_authentication");
    }

    let new_account_result = index_db::new_account(
        &mut locked_index_db_client,
        &new_account.username,
        &new_account.public_key,
    );
    match new_account_result {
        Ok(()) => make_response(201, "ok"),
        Err(index_db::new_account::Error::UsernameTaken) => make_response(422, "username_taken"),
        Err(index_db::new_account::Error::Uninterpreted(_)) => {
            println!("Internal server error! {:?}", new_account_result);
            make_response(500, "internal_error")
        }
    }
}

fn make_response(http_code: u16, error_code: &str) -> Response {
    Response::build()
        .status(
            Status::from_code(http_code).expect("Server has an invalid status code hard-coded!"),
        )
        .sized_body(Cursor::new(
            serde_json::to_string(&NewAccountResponse {
                error_code: String::from(error_code),
            })
            .expect("Failed to json-serialize response!"),
        ))
        .finalize()
}
