#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate rocket;
extern crate base64;
extern crate lockbook_core;
extern crate hyper;
extern crate tokio;

pub mod api;
pub mod config;
pub mod files_db;
pub mod index_db;

use crate::config::{config, ServerState};
use std::sync::Mutex;

fn main_rocket() {
    let config = config();
    let index_db_client = match index_db::connect(&config.index_db_config) {
        Ok(client) => client,
        Err(err) => panic!("{:?}", err),
    };
    let files_db_client = match files_db::connect(&config.files_db_config) {
        Ok(x) => x,
        Err(err) => panic!("{:?}", err),
    };
    let server_state = ServerState {
        index_db_client: Mutex::new(index_db_client),
        files_db_client: Mutex::new(files_db_client),
    };

    rocket::ignite()
        .manage(server_state)
        .mount(
            "/",
            routes![
                api::index::index,
                api::new_account::new_account,
                api::create_file::create_file,
                api::change_file_content::change_file_content,
                api::get_updates::get_updates,
                api::rename_file::rename_file,
                api::move_file::move_file,
                api::delete_file::delete_file,
            ],
        )
        .launch();
}

use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Method, Request, Response, Server, StatusCode};

async fn echo(req: Request<Body>) -> Result<Response<Body>, hyper::Error> {
    match (req.method(), req.uri().path()) {
        (&Method::POST, "/echo") => Ok(Response::new(req.into_body())),
        _ => {
            let mut not_found = Response::default();
            *not_found.status_mut() = StatusCode::NOT_FOUND;
            Ok(not_found)
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // let config = config();
    // let index_db_client = match index_db::connect(&config.index_db_config) {
    //     Ok(client) => client,
    //     Err(err) => panic!("{:?}", err),
    // };
    // let files_db_client = match files_db::connect(&config.files_db_config) {
    //     Ok(x) => x,
    //     Err(err) => panic!("{:?}", err),
    // };
    // let server_state = ServerState {
    //     index_db_client: Mutex::new(index_db_client),
    //     files_db_client: Mutex::new(files_db_client),
    // };
    let addr = ([127, 0, 0, 1], 3000).into();
    let service = make_service_fn(|_| async { Ok::<_, hyper::Error>(service_fn(echo)) });
    let server = Server::bind(&addr).serve(service);
    println!("Listening on http://{}", addr);
    server.await?;
    Ok(())
}
