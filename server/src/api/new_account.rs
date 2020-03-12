use crate::config::ServerState;
use crate::index_db;
use rocket::http::Status;
use rocket::request::Form;
use rocket::Response;
use rocket::State;

#[derive(FromForm, Debug)]
pub struct NewAccount {
    pub username: String,
    pub auth: String,
    pub pub_key_n: String,
    pub pub_key_e: String,
}

#[post("/new-account", data = "<new_account>")]
pub fn new_account(server_state: State<ServerState>, new_account: Form<NewAccount>) -> Response {
    println!("new_account: {:?}", new_account);

    let mut locked_index_db_client = server_state.index_db_client.lock().unwrap();

    match index_db::create_user(
        &mut locked_index_db_client,
        &new_account.username,
        &new_account.pub_key_n,
        &new_account.pub_key_e,
    ) {
        Ok(_) => Response::build().status(Status::Ok).finalize(),
        Err(err) => {
            println!("{:?}", err);
            Response::build()
                .status(Status::InternalServerError)
                .finalize()
        }
    }
}
