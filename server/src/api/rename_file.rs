use crate::config::ServerState;
use crate::index_db;
use rocket::http::Status;
use rocket::request::Form;
use rocket::Response;
use rocket::State;
use std::io::Cursor;
use lockbook_core::client::RenameFileResponse;

#[derive(FromForm, Debug)]
pub struct RenameFile {
    pub username: String,
    pub auth: String,
    pub file_id: String,
    pub new_file_name: String,
}

#[put("/rename-file", data = "<rename_file>")]
pub fn rename_file(server_state: State<ServerState>, rename_file: Form<RenameFile>) -> Response {
    let mut locked_index_db_client = server_state.index_db_client.lock().unwrap();

    let rename_file_result = index_db::rename_file(
        &mut locked_index_db_client,
        &rename_file.file_id,
        &rename_file.new_file_name,
    );
    match rename_file_result {
        Ok(_) => make_response(200, "ok"),
        Err(index_db::rename_file::Error::FileDoesNotExist) => make_response(404, "file_not_found"),
        Err(index_db::rename_file::Error::FileDeleted) => make_response(410, "file_deleted"),
        Err(index_db::rename_file::Error::Uninterpreted(_)) => {
            println!("Internal server error! {:?}", rename_file_result);
            make_response(500, "internal_error")
        }
        Err(index_db::rename_file::Error::VersionGeneration(_)) => {
            println!("Internal server error! {:?}", rename_file_result);
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
            serde_json::to_string(&RenameFileResponse {
                error_code: String::from(error_code),
            })
            .expect("Failed to json-serialize response!"),
        ))
        .finalize()
}
