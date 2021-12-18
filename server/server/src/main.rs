extern crate chrono;
extern crate log;
extern crate tokio;

use lockbook_models::api::{FileMetadataUpsertsRequest, NewAccountRequest, RequestWrapper};
use lockbook_server_lib::account_service::new_account;
use lockbook_server_lib::config::Config;
use lockbook_server_lib::file_service::upsert_file_metadata;
use lockbook_server_lib::*;

use std::sync::Arc;

use warp::hyper::body::Bytes;
use warp::Filter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config = Config::from_env_vars();
    pretty_env_logger::init();

    let index_db_client = file_index_repo::connect(&config.index_db)
        .await
        .expect("Failed to connect to index_db");

    let files_db_client = file_content_client::create_client(&config.files_db)
        .expect("Failed to create files_db client");

    let server_state = Arc::new(ServerState {
        config: config.clone(),
        index_db_client,
        files_db_client,
    });

    let route = core_request!(NewAccountRequest, new_account, server_state).or(core_request!(
        FileMetadataUpsertsRequest,
        upsert_file_metadata,
        server_state
    ));

    warp::serve(route).run(([127, 0, 0, 1], 3030)).await;
    Ok(())
}
