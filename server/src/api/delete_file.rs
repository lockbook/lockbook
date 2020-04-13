use crate::config::ServerState;
use crate::files_db;
use crate::index_db;
use lockbook_core::lockbook_api::DeleteFileResponse;
use rocket::http::Status;
use rocket::request::Form;
use rocket::Response;
use rocket::State;
use std::io::Cursor;

#[derive(FromForm, Debug)]
pub struct DeleteFile {
    pub username: String,
    pub auth: String,
    pub file_id: String,
}

#[delete("/delete-file", data = "<delete_file>")]
pub fn delete_file(server_state: State<ServerState>, delete_file: Form<DeleteFile>) -> Response {
    let mut locked_index_db_client = server_state.index_db_client.lock().unwrap();
    let locked_files_db_client = server_state.files_db_client.lock().unwrap();

    let index_db_delete_file_result =
        index_db::delete_file(&mut locked_index_db_client, &delete_file.file_id);
    match index_db_delete_file_result {
        Ok(_) => {}
        Err(index_db::delete_file::Error::FileDoesNotExist) => {
            return make_response(404, "file_not_found");
        }
        Err(index_db::delete_file::Error::FileDeleted) => {
            return make_response(410, "file_deleted");
        }
        Err(index_db::delete_file::Error::Uninterpreted(_)) => {
            println!("Internal server error! {:?}", index_db_delete_file_result);
            return make_response(500, "internal_error");
        }
        Err(index_db::delete_file::Error::VersionGeneration(_)) => {
            println!("Internal server error! {:?}", index_db_delete_file_result);
            return make_response(500, "internal_error");
        }
    };

    let filed_db_delete_file_result =
        files_db::delete_file(&locked_files_db_client, &delete_file.file_id);
    match filed_db_delete_file_result {
        Ok(()) => make_response(200, "ok"),
        Err(files_db::delete_file::Error::S3OperationUnsuccessful(_)) => {
            println!("Internal server error! {:?}", filed_db_delete_file_result);
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
            serde_json::to_string(&DeleteFileResponse {
                error_code: String::from(error_code),
            })
            .expect("Failed to json-serialize response!"),
        ))
        .finalize()
}
