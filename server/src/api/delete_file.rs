use crate::config::ServerState;
use crate::files_db;
use crate::index_db;
use rocket::http::Status;
use rocket::request::Form;
use rocket::Response;
use rocket::State;

#[derive(Debug)]
pub enum Error {
    IndexDb(index_db::delete_file::Error),
    FilesDb(files_db::delete_file::Error),
}

#[derive(FromForm, Debug)]
pub struct DeleteFile {
    pub username: String,
    pub auth: String,
    pub file_id: String,
}

#[delete("/delete-file", data = "<delete_file>")]
pub fn delete_file(server_state: State<ServerState>, delete_file: Form<DeleteFile>) -> Response {
    println!("delete_file: {:?}", delete_file);

    let mut locked_index_db_client = server_state.index_db_client.lock().unwrap();
    let locked_files_db_client = server_state.files_db_client.lock().unwrap();

    match Ok(())
        .and_then(|_| {
            match index_db::delete_file(&mut locked_index_db_client, &delete_file.file_id) {
                Ok(_) => Ok(()),
                Err(err) => Err(Error::IndexDb(err)),
            }
        })
        .and_then(
            |_| match files_db::delete_file(&locked_files_db_client, &delete_file.file_id) {
                Ok(()) => Ok(()),
                Err(err) => Err(Error::FilesDb(err)),
            },
        ) {
        Ok(()) => Response::build().status(Status::Ok).finalize(),
        Err(err) => {
            println!("{:?}", err);
            Response::build()
                .status(Status::InternalServerError)
                .finalize()
        }
    }
}
