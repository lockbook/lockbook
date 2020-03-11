use rocket::http::Status;
use rocket::Response;
use std::io::Cursor;

#[get("/")]
pub fn index() -> Response<'static> {
    Response::build()
        .status(Status::Ok)
        .sized_body(Cursor::new("Lockbook Server"))
        .finalize()
}
