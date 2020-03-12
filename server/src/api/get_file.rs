use crate::config::ServerState;
use crate::files_db;
use rocket::http::Status;
use rocket::Response;
use rocket::State;
use std::io::Cursor;

#[get("/get-file/<file_id>")]
pub fn get_file(server_state: State<ServerState>, file_id: String) -> Response {
    println!("get_file: {:?}", file_id);

    let locked_files_db_client = server_state.files_db_client.lock().unwrap();

    match files_db::get_file(&locked_files_db_client, &file_id) {
        Ok(content) => Response::build()
            .status(Status::Ok)
            .sized_body(Cursor::new(content))
            .finalize(),
        Err(err) => {
            println!("{:?}", err);
            Response::build()
                .status(Status::InternalServerError)
                .finalize()
        }
    }
}
