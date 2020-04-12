#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate rocket;
extern crate base64;
extern crate lockbook_core;

pub mod api;
pub mod config;
pub mod files_db;
pub mod index_db;

use crate::config::{config, ServerState};
use std::sync::Mutex;

fn main() {
    let config = config();
    let index_db_client = match index_db::connect(&config.index_db_config) {
        Ok(client) => client,
        Err(index_db::connect::Error::OpenSslFailed(err)) => {
            println!("{:?}", err);
            panic!("{}", err);
        }
        Err(index_db::connect::Error::PostgresConnectionFailed(err)) => panic!("{}", err),
        Err(index_db::connect::Error::PostgresPortNotU16(err)) => panic!("{}", err),
    };
    let files_db_client = match files_db::connect(&config.files_db_config) {
        Ok(x) => x,
        Err(files_db::connect::Error::S3ConnectionFailed(err)) => panic!("{}", err),
        Err(files_db::connect::Error::UnknownRegion(err)) => panic!("{}", err),
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
                api::get_file::get_file,
                api::change_file_content::change_file_content,
                api::get_updates::get_updates,
                api::rename_file::rename_file,
                api::move_file::move_file,
                api::delete_file::delete_file,
            ],
        )
        .launch();
}
