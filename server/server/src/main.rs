extern crate chrono;
extern crate log;
extern crate tokio;

use lockbook_server_lib::config::Config;

use lockbook_server_lib::*;

use std::sync::Arc;
use warp::Filter;

use lockbook_server_lib::router_service::{build_info, core_routes, get_metrics};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config = Config::from_env_vars();
    pretty_env_logger::init();

    // *** Things this server connects to ***
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

    let routes = core_routes(&server_state)
        .or(build_info())
        .or(get_metrics());

    let server = warp::serve(routes);

    // *** How people can connect to this server ***
    match (
        config.server.ssl_cert_location,
        config.server.ssl_private_key_location,
    ) {
        (Some(cert), Some(key)) => {
            server
                .tls()
                .cert_path(&cert)
                .key_path(&key)
                .run(([0, 0, 0, 0], config.server.port))
                .await
        }
        _ => {
            println!("binding to localhost (not 0) without tls for local development");
            server.run(([127, 0, 0, 1], config.server.port)).await
        }
    };

    Ok(())
}
