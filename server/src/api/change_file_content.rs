use crate::api::utils::make_response_generic;
use crate::config::ServerState;
use crate::files_db;
use crate::index_db;
use lockbook_core::lockbook_api::ChangeFileContentResponse;
use rocket::request::Form;
use rocket::Response;
use rocket::State;

#[derive(FromForm, Debug)]
pub struct ChangeFileContent {
    pub username: String,
    pub auth: String,
    pub file_id: String,
    pub old_file_version: i64,
    pub new_file_content: String,
}

#[put("/change-file-content", data = "<change_file>")]
pub fn change_file_content(
    server_state: State<ServerState>,
    change_file: Form<ChangeFileContent>,
) -> Response {
    let mut locked_index_db_client = server_state.index_db_client.lock().unwrap();
    let locked_files_db_client = server_state.files_db_client.lock().unwrap();

    let update_file_version_result = index_db::update_file_version(
        &mut locked_index_db_client,
        &change_file.file_id,
        &change_file.old_file_version,
    );
    let new_version = match update_file_version_result {
        Ok(new_version) => new_version,
        Err(index_db::update_file_version::Error::Uninterpreted(_)) => {
            println!("Internal server error! {:?}", update_file_version_result);
            return make_response(500, "internal_error", 0);
        }
        Err(index_db::update_file_version::Error::VersionGeneration(_)) => {
            println!("Internal server error! {:?}", update_file_version_result);
            return make_response(500, "internal_error", 0);
        }
        Err(index_db::update_file_version::Error::FileDoesNotExist) => {
            return make_response(404, "file_not_found", 0);
        }
        Err(index_db::update_file_version::Error::IncorrectOldVersion(_)) => {
            return make_response(409, "edit_conflict", 0);
        }
        Err(index_db::update_file_version::Error::FileDeleted) => {
            return make_response(410, "file_deleted", 0);
        }
    };

    let create_file_result = files_db::create_file(
        &locked_files_db_client,
        &change_file.file_id,
        &change_file.new_file_content,
    );
    match create_file_result {
        Ok(()) => make_response(200, "ok", new_version),
        Err(_) => {
            println!("Internal server error! {:?}", create_file_result);
            make_response(500, "internal_error", 0)
        }
    }
}

fn make_response(http_code: u16, error_code: &str, current_version: i64) -> Response {
    make_response_generic(
        http_code,
        ChangeFileContentResponse {
            error_code: String::from(error_code),
            current_version: current_version as u64,
        },
    )
}
