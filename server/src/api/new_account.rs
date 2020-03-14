use crate::config::ServerState;
use crate::index_db;
use rocket::http::Status;
use rocket::request::Form;
use rocket::Response;
use rocket::State;
use serde::Serialize;
use std::io::Cursor;

#[derive(FromForm, Debug)]
pub struct NewAccount {
    pub username: String,
    pub auth: String,
    pub pub_key_n: String,
    pub pub_key_e: String,
}

#[derive(Serialize)]
struct NewAccountResponse {
    error_code: String,
}

#[post("/new-account", data = "<new_account>")]
pub fn new_account(server_state: State<ServerState>, new_account: Form<NewAccount>) -> Response {
    println!("new_account: {:?}", new_account);

    let mut locked_index_db_client = server_state.index_db_client.lock().unwrap();

    let result = index_db::new_account(
        &mut locked_index_db_client,
        &new_account.username,
        &new_account.pub_key_n,
        &new_account.pub_key_e,
    );

    let (status, error_code) = match result {
        Ok(()) => (Status::Ok, String::from("ok")),
        Err(index_db::new_account::Error::UsernameTaken) => {
            (Status::UnprocessableEntity, String::from("username_taken"))
        }
        Err(index_db::new_account::Error::Uninterpreted(e)) => {
            (Status::InternalServerError, format!("{:?}", e))
        }
    };

    println!("status: {:?}, error_code: {:?}", status, error_code);

    Response::build()
        .status(status)
        .sized_body(Cursor::new(
            serde_json::to_string(&NewAccountResponse {
                error_code: error_code,
            })
            .expect("Failed to json-serialize response!"),
        ))
        .finalize()
}
