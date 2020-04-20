use rocket::http::Status;
use rocket::Response;
use serde::Serialize;
use std::io::Cursor;

pub fn make_response_generic<'a, T: Serialize>(http_code: u16, value: T) -> Response<'a> {
    Response::build()
        .status(
            Status::from_code(http_code).expect("Server has an invalid status code hard-coded!"),
        )
        .sized_body(Cursor::new(
            serde_json::to_string(&value).expect("Failed to json-serialize response!"),
        ))
        .finalize()
}
