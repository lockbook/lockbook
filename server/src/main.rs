#![recursion_limit = "256"]

use db_rs::Db;
use lockbook_server_lib::billing::google_play_client::get_google_play_client;
use lockbook_server_lib::config::Config;
use lockbook_server_lib::document_service::OnDiskDocuments;
use lockbook_server_lib::router_service::{
    app_store_notification_webhooks, build_info, core_routes, get_metrics,
    google_play_notification_webhooks, stripe_webhooks,
};
use lockbook_server_lib::schema::{ServerDb, ServerV5};
use lockbook_server_lib::*;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::*;
use warp::Filter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let cfg = Config::from_env_vars();
    loggers::init(&cfg);

    let config = cfg.clone();

    let stripe_client = stripe::Client::new(&cfg.billing.stripe.stripe_secret);
    let google_play_client = get_google_play_client(&cfg.billing.google.service_account_key).await;
    let app_store_client = reqwest::Client::new();

    let index_db = ServerV5::init(db_rs::Config::in_folder(&cfg.index_db.db_location))
        .expect("Failed to load index_db");
    if index_db.incomplete_write().unwrap() {
        error!("dbrs indicated that the last write to the log was unsuccessful")
    }
    let index_db = Arc::new(Mutex::new(index_db));
    spawn_compacter(&cfg, &index_db);

    let document_service = OnDiskDocuments::from(&config);

    let server_state = Arc::new(ServerState {
        config,
        index_db,
        stripe_client,
        google_play_client,
        app_store_client,
        document_service,
        recent_ips: Default::default(),
    });

    let routes = core_routes(&server_state)
        .or(build_info())
        .or(stripe_webhooks(&server_state))
        .or(google_play_notification_webhooks(&server_state))
        .or(app_store_notification_webhooks(&server_state));

    let server = warp::serve(routes);

    error!("server started successfully");

    server_state.start_metrics_worker();

    // metrics endpoint to be served anauthenticated, locally, only
    tokio::spawn(warp::serve(get_metrics()).run(([127, 0, 0, 1], 8080)));

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

fn spawn_compacter(cfg: &Config, db: &Arc<Mutex<ServerDb>>) {
    let cfg = cfg.clone();
    let db = db.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(cfg.index_db.time_between_compacts).await;
            if let Err(e) = db.lock().await.compact_log() {
                error!("failed to compact log: {e:?}");
            }
        }
    });
}
