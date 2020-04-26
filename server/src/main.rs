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
    let config = config(); // write some code to try to use lockbook core
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
                api::get_public_key::get_public_key,
                api::rename_file::rename_file,
                api::move_file::move_file,
                api::delete_file::delete_file,
            ],
        )
        .launch();
}
