use crate::api::utils::make_response_generic;
use crate::config::{config, ServerState};
use crate::index_db;
use lockbook_core::client::RenameFileResponse;
use lockbook_core::service::auth_service::{AuthService, AuthServiceImpl};
use lockbook_core::service::clock_service::ClockImpl;
use lockbook_core::service::crypto_service::RsaImpl;
use rocket::http::Status;
use rocket::request::Form;
use rocket::Response;
use rocket::State;

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

    let public_key =
        match index_db::get_public_key(&mut locked_index_db_client, &rename_file.username) {
            Ok(public_key) => public_key,
            Err(_) => return Response::build().status(Status::NotFound).finalize(),
        };

    if let Err(e) = AuthServiceImpl::<ClockImpl, RsaImpl>::verify_auth(
        &rename_file.auth,
        &public_key,
        &rename_file.username,
        config().auth_config.max_auth_delay,
    ) {
        println!(
            "Auth failed for: {}, {}, {}, {:?}",
            rename_file.username,
            rename_file.auth,
            &serde_json::to_string(&public_key).unwrap(),
            e
        );
        return Response::build().status(Status::Unauthorized).finalize();
    }

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
    make_response_generic(
        http_code,
        RenameFileResponse {
            error_code: String::from(error_code),
        },
    )
}
