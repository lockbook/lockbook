use crate::api::utils::make_response_generic;
use crate::config::{config, ServerState};
use crate::files_db;
use crate::index_db;
use lockbook_core::client::DeleteFileResponse;
use lockbook_core::service::auth_service::{AuthService, AuthServiceImpl};
use lockbook_core::service::clock_service::ClockImpl;
use lockbook_core::service::crypto_service::RsaImpl;
use rocket::http::Status;
use rocket::request::Form;
use rocket::Response;
use rocket::State;

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

    let public_key =
        match index_db::get_public_key(&mut locked_index_db_client, &delete_file.username) {
            Ok(public_key) => public_key,
            Err(_) => return Response::build().status(Status::NotFound).finalize(),
        };

    if let Err(e) = AuthServiceImpl::<ClockImpl, RsaImpl>::verify_auth(
        &delete_file.auth,
        &public_key,
        &delete_file.username,
        config().auth_config.max_auth_delay.parse().unwrap(), //TODO: don't unwrap
    ) {
        println!(
            "Auth failed for: {}, {}, {}, {:?}",
            delete_file.username, delete_file.auth, &serde_json::to_string(&public_key).unwrap(), e
        );
        return Response::build().status(Status::Unauthorized).finalize();
    }

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
        Err(_) => {
            println!("Internal server error! {:?}", filed_db_delete_file_result);
            make_response(500, "internal_error")
        }
    }
}

fn make_response(http_code: u16, error_code: &str) -> Response {
    make_response_generic(
        http_code,
        DeleteFileResponse {
            error_code: String::from(error_code),
        },
    )
}
