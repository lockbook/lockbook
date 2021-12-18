extern crate chrono;
extern crate log;
extern crate tokio;

use lockbook_server_lib::config::Config;

use lockbook_server_lib::*;

use std::sync::Arc;

use lockbook_server_lib::router_service::core_routes;

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

    warp::serve(core_routes(&server_state))
        .run(([127, 0, 0, 1], 8000))
        .await;
    Ok(())
}
