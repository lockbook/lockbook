use crate::api::utils::make_response_generic;
use crate::config::ServerState;
use crate::files_db;
use crate::index_db;
use lockbook_core::lockbook_api::CreateFileResponse;
use rocket::request::Form;
use rocket::Response;
use rocket::State;

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

    let get_file_details_result =
        files_db::get_file_details(&locked_files_db_client, &create_file.file_id);
    match get_file_details_result {
        Err(files_db::get_file_details::Error::NoSuchFile(())) => {}
        Err(files_db::get_file_details::Error::S3ConnectionFailed(_)) => {
            println!("Internal server error! {:?}", get_file_details_result);
            return make_response(500, "internal_error", 0);
        }
        Err(files_db::get_file_details::Error::S3OperationUnsuccessful(_)) => {
            println!("Internal server error! {:?}", get_file_details_result);
            return make_response(500, "internal_error", 0);
        }
        Ok(_) => return make_response(422, "file_id_taken", 0),
    };

    let index_db_create_file_result = index_db::create_file(
        &mut locked_index_db_client,
        &create_file.file_id,
        &create_file.username,
        &create_file.file_name,
        &create_file.file_path,
    );
    let new_version = match index_db_create_file_result {
        Ok(version) => version,
        Err(index_db::create_file::Error::FileIdTaken) => {
            return make_response(422, "file_id_taken", 0);
        }
        Err(index_db::create_file::Error::FilePathTaken) => {
            return make_response(422, "file_path_taken", 0);
        }
        Err(index_db::create_file::Error::Uninterpreted(_)) => {
            println!("Internal server error! {:?}", index_db_create_file_result);
            return make_response(500, "internal_error", 0);
        }
        Err(index_db::create_file::Error::VersionGeneration(_)) => {
            println!("Internal server error! {:?}", index_db_create_file_result);
            return make_response(500, "internal_error", 0);
        }
    };

    let files_db_create_file_result = files_db::create_file(
        &locked_files_db_client,
        &create_file.file_id,
        &create_file.file_content,
    );
    match files_db_create_file_result {
        Ok(()) => make_response(201, "ok", new_version),
        Err(files_db::create_file::Error::S3(_)) => {
            println!("Internal server error! {:?}", files_db_create_file_result);
            make_response(500, "internal_error", 0)
        }
    }
}

fn make_response(http_code: u16, error_code: &str, current_version: i64) -> Response {
    make_response_generic(
        http_code,
        CreateFileResponse {
            error_code: String::from(error_code),
            current_version: current_version as u64,
        },
    )
}
