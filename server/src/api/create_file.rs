use crate::config::ServerState;
use crate::files_db;
use crate::index_db;
use lockbook_core::lockbook_api::CreateFileResponse;
use rocket::http::Status;
use rocket::request::Form;
use rocket::Response;
use rocket::State;
use std::io::Cursor;

#[derive(Debug)]
pub enum Error {
    IndexDb(index_db::create_file::Error),
    FilesDb(files_db::create_file::Error),
    FileAlreadyExists(()),
}

#[derive(FromForm, Debug)]
pub struct CreateFile {
    pub username: String,
    pub auth: String,
    pub file_id: String,
    pub file_name: String,
    pub file_path: String,
    pub file_content: String,
}

#[post("/create-file", data = "<create_file>")]
pub fn create_file(server_state: State<ServerState>, create_file: Form<CreateFile>) -> Response {
    let mut locked_index_db_client = server_state.index_db_client.lock().unwrap();
    let locked_files_db_client = server_state.files_db_client.lock().unwrap();

    match files_db::get_file_details(&locked_files_db_client, &create_file.file_id) {
        Err(files_db::get_file_details::Error::NoSuchFile(())) => {}
        Err(files_db::get_file_details::Error::S3ConnectionFailed(s3_error)) => {
            println!("Internal server error! {:?}", s3_error);
            return make_response(500, "internal_error", 0);
        }
        Err(files_db::get_file_details::Error::S3OperationUnsuccessful(s3_err_code)) => {
            println!("Internal server error! {:?}", s3_err_code);
            return make_response(500, "internal_error", 0);
        }
        Ok(_) => return make_response(422, "file_id_taken", 0),
    };

    let new_version = match index_db::create_file(
        &mut locked_index_db_client,
        &create_file.file_id,
        &create_file.username,
        &create_file.file_name,
        &create_file.file_path,
    ) {
        Ok(version) => version,
        Err(index_db::create_file::Error::FileIdTaken) => {
            return make_response(422, "file_id_taken", 0);
        }
        Err(index_db::create_file::Error::FilePathTaken) => {
            return make_response(422, "file_path_taken", 0);
        }
        Err(index_db::create_file::Error::Uninterpreted(postgres_error)) => {
            println!("Internal server error! {:?}", postgres_error);
            return make_response(500, "internal_error", 0);
        }
        Err(index_db::create_file::Error::VersionGeneration(version_generation_error)) => {
            println!("Internal server error! {:?}", version_generation_error);
            return make_response(500, "internal_error", 0);
        }
    };

    match files_db::create_file(
        &locked_files_db_client,
        &create_file.file_id,
        &create_file.file_content,
    ) {
        Ok(()) => make_response(201, "ok", new_version),
        Err(files_db::create_file::Error::S3(s3_error)) => {
            println!("Internal server error! {:?}", s3_error);
            make_response(500, "internal_error", 0)
        }
    }
}

fn make_response(http_code: u16, error_code: &str, current_version: i64) -> Response {
    Response::build()
        .status(
            Status::from_code(http_code).expect("Server has an invalid status code hard-coded!"),
        )
        .sized_body(Cursor::new(
            serde_json::to_string(&CreateFileResponse {
                error_code: String::from(error_code),
                current_version: current_version as u64,
            })
            .expect("Failed to json-serialize response!"),
        ))
        .finalize()
}
