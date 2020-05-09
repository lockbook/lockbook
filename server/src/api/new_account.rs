use crate::api::utils::make_response_generic;
use crate::config::{config, ServerState};
use crate::index_db;
use crate::index_db::get_public_key::Error;
use lockbook_core::client::NewAccountResponse;
use lockbook_core::service::auth_service::{AuthService, AuthServiceImpl};
use lockbook_core::service::clock_service::ClockImpl;
use lockbook_core::service::crypto_service::RsaImpl;
use rocket::http::Status;
use rocket::request::Form;
use rocket::Response;
use rocket::State;

#[derive(FromForm, Debug)]
pub struct NewAccount {
    pub username: String,
    pub auth: String,
    pub public_key: String,
}

#[post("/new-account", data = "<new_account>")]
pub fn new_account(server_state: State<ServerState>, new_account: Form<NewAccount>) -> Response {
    let mut locked_index_db_client = server_state.index_db_client.lock().unwrap();

    if let Err(e) = AuthServiceImpl::<ClockImpl, RsaImpl>::verify_auth(
        &new_account.auth,
        &serde_json::from_str(&new_account.public_key).unwrap(),
        &new_account.username,
        config().auth_config.max_auth_delay,
    ) {
        println!(
            "Auth failed for: {}, {}, {}, {:?}",
            new_account.username, new_account.auth, new_account.public_key, e
        );
        return Response::build().status(Status::Unauthorized).finalize();
    }

    let new_account_result = index_db::new_account(
        &mut locked_index_db_client,
        &new_account.username,
        &new_account.public_key,
    );

    let public_key =
        match index_db::get_public_key(&mut locked_index_db_client, &new_account.username) {
            Ok(public_key) => public_key,
            Err(Error::SerializationError(_)) => {
                return Response::build()
                    .status(Status::InternalServerError)
                    .finalize()
            }
            Err(Error::Postgres(_)) => {
                return Response::build().status(Status::NotFound).finalize()
            }
        };

    if serde_json::to_string(&public_key).expect("Failed to json-serialize response!")
        != new_account.public_key
    {
        return Response::build().status(Status::Unauthorized).finalize();
    }

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
    make_response_generic(
        http_code,
        NewAccountResponse {
            error_code: String::from(error_code),
        },
    )
}
