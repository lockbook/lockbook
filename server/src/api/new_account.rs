use std::io::Cursor;

use lockbook_core::crypto::*;
use rocket::http::Status;
use rocket::request::Form;
use rocket::Response;
use rocket::State;
use serde::Serialize;

use crate::config::ServerState;
use crate::index_db;
use std::time::{SystemTime, UNIX_EPOCH};

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




    let pub_key = PublicKey {
        n: new_account.pub_key_n.clone(),
        e: new_account.pub_key_e.clone(),
    };

    let true_val = format!("{}{}{}",
                           &new_account.username,
                           ",",
                           SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis().to_string());

    // let decrypt_val = CryptoService::decrypt_public(&pub_key, new_account.auth).unwrap().secret;

    let decrypt_comp: Vec<str> = decrypt_val.split(",").collect(); // only leaving type to remember since it doesnt wanna work
    let true_comp: Vec<str> = true_val.split(",").collect(); // ^

    let decrypt_time = decrypt_comp.get(2).unwrap().parse::<u128>();
    let true_time = true_comp.get(2).unwrap().parse::<u128>();


    if decrypt_comp.get(0).unwrap() != true_comp.get(0).unwrap() ||
       // decrypt_comp.get(1).unwrap() != true_comp.get(1).unwrap() ||
        (decrypt_time >= true_time - 50 && decrypt_time <= true_time + 50)  {
            return Response::build()
                .status(Status::UnprocessableEntity)
                .sized_body(Cursor::new(
                    serde_json::to_string(&NewAccountResponse {
                        error_code: String::from("Failed Verification"),
                    })
                        .expect("Failed to json-serialize response!"),
                ))
                .finalize()
        }

    // in core, I will take a username, timestamp, and the keys
    // fix string TODOz with actual auth string

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
