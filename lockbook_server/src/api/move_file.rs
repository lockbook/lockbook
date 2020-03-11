use crate::config::ServerState;
use crate::index_db;
use rocket::http::Status;
use rocket::request::Form;
use rocket::Response;
use rocket::State;
use std::io::Cursor;

#[derive(FromForm, Debug)]
pub struct MoveFile {
    pub username: String,
    pub auth: String,
    pub file_id: String,
    pub new_file_path: String,
}

#[put("/move-file", data = "<move_file>")]
pub fn move_file(server_state: State<ServerState>, move_file: Form<MoveFile>) -> Response {
    println!("move_file: {:?}", move_file);

    let mut locked_index_db_client = server_state.index_db_client.lock().unwrap();

    match index_db::move_file(
        &mut locked_index_db_client,
        &move_file.file_id,
        &move_file.new_file_path,
    ) {
        Ok(new_version) => Response::build()
            .status(Status::Ok)
            .sized_body(Cursor::new(new_version.to_string()))
            .finalize(),
        Err(err) => {
            println!("{:?}", err);
            Response::build()
                .status(Status::InternalServerError)
                .finalize()
        }
    }
}
