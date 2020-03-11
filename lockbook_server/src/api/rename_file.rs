use crate::config::ServerState;
use crate::index_db;
use rocket::http::Status;
use rocket::request::Form;
use rocket::Response;
use rocket::State;
use std::io::Cursor;

#[derive(FromForm, Debug)]
pub struct RenameFile {
    pub username: String,
    pub auth: String,
    pub file_id: String,
    pub new_file_name: String,
}

#[put("/rename-file", data = "<rename_file>")]
pub fn rename_file(server_state: State<ServerState>, rename_file: Form<RenameFile>) -> Response {
    println!("rename_file: {:?}", rename_file);

    let mut locked_index_db_client = server_state.index_db_client.lock().unwrap();

    match index_db::rename_file(
        &mut locked_index_db_client,
        &rename_file.file_id,
        &rename_file.new_file_name,
    ) {
        Ok(version) => Response::build()
            .status(Status::Ok)
            .sized_body(Cursor::new(version.to_string()))
            .finalize(),
        Err(err) => {
            println!("{:?}", err);
            Response::build()
                .status(Status::InternalServerError)
                .finalize()
        }
    }
}
