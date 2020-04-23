use crate::api::utils::make_response_generic;
use crate::config::ServerState;
use crate::index_db;
use lockbook_core::client::MoveFileResponse;
use rocket::request::Form;
use rocket::Response;
use rocket::State;

#[derive(FromForm, Debug)]
pub struct MoveFile {
    pub username: String,
    pub auth: String,
    pub file_id: String,
    pub new_file_path: String,
}

#[put("/move-file", data = "<move_file>")]
pub fn move_file(server_state: State<ServerState>, move_file: Form<MoveFile>) -> Response {
    let mut locked_index_db_client = server_state.index_db_client.lock().unwrap();

    let move_file_result = index_db::move_file(
        &mut locked_index_db_client,
        &move_file.file_id,
        &move_file.new_file_path,
    );
    match move_file_result {
        Ok(_) => make_response(200, "ok"),
        Err(index_db::move_file::Error::FileDoesNotExist) => make_response(404, "file_not_found"),
        Err(index_db::move_file::Error::FileDeleted) => make_response(410, "file_deleted"),
        Err(index_db::move_file::Error::FilePathTaken) => make_response(422, "file_path_taken"),
        Err(index_db::move_file::Error::Uninterpreted(_)) => {
            println!("Internal server error! {:?}", move_file_result);
            make_response(500, "internal_error")
        }
        Err(index_db::move_file::Error::VersionGeneration(_)) => {
            println!("Internal server error! {:?}", move_file_result);
            make_response(500, "internal_error")
        }
    }
}

fn make_response(http_code: u16, error_code: &str) -> Response {
    make_response_generic(
        http_code,
        MoveFileResponse {
            error_code: String::from(error_code),
        },
    )
}
