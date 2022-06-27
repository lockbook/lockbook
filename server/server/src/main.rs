extern crate chrono;
extern crate log;
extern crate tokio;

use deadpool_redis::Runtime;
use hmdb::log::Reader;
use lockbook_server_lib::billing::google_play_client::get_google_play_client;
use lockbook_server_lib::config::Config;
use lockbook_server_lib::router_service::{
    build_info, core_routes, get_metrics, google_play_notification_webhooks, stripe_webhooks,
};
use lockbook_server_lib::schema::ServerV1;
use lockbook_server_lib::*;
use log::info;
use std::sync::Arc;
use warp::Filter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let cfg = Config::from_env_vars();
    loggers::init(&cfg);

    let config = cfg.clone();
    let files_db_client = file_content_client::create_client(&cfg.files_location)
        .expect("Failed to create files_db client");
    let index_db_pool = deadpool_redis::Config::from_url(&cfg.index_db.redis_url)
        .create_pool(Some(Runtime::Tokio1))
        .unwrap();
    let stripe_client = stripe::Client::new(&cfg.billing.stripe.stripe_secret);
    let google_play_client = get_google_play_client(&cfg.billing.google.service_account_key).await;
    let index_db = ServerV1::init(&cfg.index_db.db_location).expect("Failed to load index_db");

    let server_state =
        Arc::new(ServerState { config, index_db, stripe_client, google_play_client });

    let routes = core_routes(&server_state)
        .or(build_info())
        .or(get_metrics())
        .or(stripe_webhooks(&server_state))
        .or(google_play_notification_webhooks(&server_state));

    let server = warp::serve(routes);

    metrics::start_metrics_worker(&server_state);

    // *** How people can connect to this server ***
    match (cfg.server.ssl_cert_location, cfg.server.ssl_private_key_location) {
        (Some(cert), Some(key)) => {
            info!("binding to https://0.0.0.0:{}", cfg.server.port);
            server
                .tls()
                .cert_path(&cert)
                .key_path(&key)
                .run(([0, 0, 0, 0], cfg.server.port))
                .await
        }
        _ => {
            info!(
                "binding to http://0.0.0.0:{} without tls for local development",
                cfg.server.port
            );
            server.run(([0, 0, 0, 0], cfg.server.port)).await
        }
    };

    Ok(())
}
