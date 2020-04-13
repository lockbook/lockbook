use crate::config::ServerState;
use crate::index_db;
use lockbook_core::lockbook_api::MoveFileResponse;
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
        Ok(_) => make_response(200, "ok"),
        Err(index_db::move_file::Error::FileDoesNotExist) => make_response(404, "file_not_found"),
        Err(index_db::move_file::Error::FileDeleted) => make_response(410, "file_deleted"),
        Err(index_db::move_file::Error::FilePathTaken) => make_response(422, "file_path_taken"),
        Err(index_db::move_file::Error::Uninterpreted(postgres_error)) => {
            println!("Internal server error! {:?}", postgres_error);
            make_response(500, "internal_error")
        },
        Err(index_db::move_file::Error::VersionGeneration(version_generation_error)) => {
            println!("Internal server error! {:?}", version_generation_error);
            make_response(500, "internal_error")
        }
    }
}

fn make_response(http_code: u16, error_code: &str) -> Response {
    Response::build()
        .status(Status::from_code(http_code)
            .expect("Server has an invalid status code hard-coded!"))
        .sized_body(Cursor::new(
            serde_json::to_string(&MoveFileResponse {
                error_code: String::from(error_code),
            })
            .expect("Failed to json-serialize response!"),
        ))
        .finalize()
}
