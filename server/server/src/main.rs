extern crate chrono;
extern crate log;
extern crate tokio;

use lockbook_server_lib::config::Config;

use lockbook_server_lib::*;

use deadpool_redis::Runtime;
use lockbook_server_lib::content::file_content_client;
use log::info;

use reqwest::Client;
use std::sync::Arc;
use warp::Filter;

use lockbook_server_lib::router_service::{build_info, core_routes, get_metrics, stripe_webhooks};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config = Config::from_env_vars();
    loggers::init(&config);

    // *** Things this server connects to ***
    let files_db_client = file_content_client::create_client(&config.files_db)
        .expect("Failed to create files_db client");

    let index_db_pool = deadpool_redis::Config::from_url(&config.index_db.redis_url)
        .create_pool(Some(Runtime::Tokio1))
        .unwrap();

    let stripe_client = stripe::Client::new(&config.stripe.stripe_secret);

    let server_state = Arc::new(ServerState {
        config: config.clone(),
        index_db_pool,
        stripe_client,
        files_db_client,
    });

    let routes = core_routes(&server_state)
        .or(build_info())
        .or(get_metrics())
        .or(stripe_webhooks(&server_state));

    let server = warp::serve(routes);

    // *** How people can connect to this server ***
    match (
        config.server.ssl_cert_location,
        config.server.ssl_private_key_location,
    ) {
        (Some(cert), Some(key)) => {
            info!("binding to https://0.0.0.0:{}", config.server.port);
            server
                .tls()
                .cert_path(&cert)
                .key_path(&key)
                .run(([0, 0, 0, 0], config.server.port))
                .await
        }
        _ => {
            info!(
                "binding to http://0.0.0.0:{} without tls for local development",
                config.server.port
            );
            server.run(([0, 0, 0, 0], config.server.port)).await
        }
    };

    Ok(())
}
