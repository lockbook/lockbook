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
    println!("create_file: {:?}", create_file);

    let mut locked_index_db_client = server_state.index_db_client.lock().unwrap();
    let locked_files_db_client = server_state.files_db_client.lock().unwrap();

    match Ok(())
        .and_then(|_| {
            match files_db::get_file_details(&locked_files_db_client, &create_file.file_id) {
                Err(files_db::get_file_details::Error::NoSuchFile(_)) => Ok(()),
                _ => Err(Error::FileAlreadyExists(())),
            }
        })
        .and_then(|_| {
            match index_db::create_file(
                &mut locked_index_db_client,
                &create_file.file_id,
                &create_file.username,
                &create_file.file_name,
                &create_file.file_path,
            ) {
                Ok(version) => Ok(version),
                Err(err) => Err(Error::IndexDb(err)),
            }
        })
        .and_then(|version| {
            match files_db::create_file(
                &locked_files_db_client,
                &create_file.file_id,
                &create_file.file_content,
            ) {
                Ok(()) => Ok(version),
                Err(err) => Err(Error::FilesDb(err)),
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
