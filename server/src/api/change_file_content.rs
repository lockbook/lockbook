use crate::config::ServerState;
use crate::files_db;
use crate::index_db;
use rocket::http::Status;
use rocket::request::Form;
use rocket::Response;
use rocket::State;
use std::io::Cursor;

#[derive(Debug)]
pub enum Error {
    IndexDbUpdateFileVersion(index_db::update_file_version::Error),
    FilesDbUpdateCreateFile(files_db::create_file::Error),
}

#[derive(FromForm, Debug)]
pub struct ChangeFileContent {
    pub username: String,
    pub auth: String,
    pub file_id: String,
    pub old_file_version: i64,
    pub new_file_content: String,
}

#[post("/change-file-content", data = "<change_file>")]
pub fn change_file_content(
    server_state: State<ServerState>,
    change_file: Form<ChangeFileContent>,
) -> Response {
    println!("change_file: {:?}", change_file);

    let mut locked_index_db_client = server_state.index_db_client.lock().unwrap();
    let locked_files_db_client = server_state.files_db_client.lock().unwrap();

    match Ok(())
        .and_then(|_| {
            match index_db::update_file_version(
                &mut locked_index_db_client,
                &change_file.file_id,
                &change_file.old_file_version,
            ) {
                Ok(new_version) => Ok(new_version),
                Err(err) => Err(Error::IndexDbUpdateFileVersion(err)),
            }
        })
        .and_then(|new_version| {
            match files_db::create_file(
                &locked_files_db_client,
                &change_file.file_id,
                &change_file.new_file_content,
            ) {
                Ok(()) => Ok(new_version),
                Err(err) => Err(Error::FilesDbUpdateCreateFile(err)),
            }
        }) {
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
