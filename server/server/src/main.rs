extern crate chrono;
extern crate log;
extern crate tokio;

use lockbook_server_lib::config::Config;

use lockbook_server_lib::*;

use deadpool_redis::Runtime;
use lockbook_server_lib::content::file_content_client;
use log::info;

use google_androidpublisher3::{hyper, hyper_rustls};
use std::sync::Arc;
use warp::Filter;

use lockbook_server_lib::router_service::{
    android_notification_webhooks, build_info, core_routes, get_metrics, stripe_webhooks,
};

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

    let (android_publisher, gcp_pubsub) = get_android_and_gcp_client(&config).await;


    let server_state = Arc::new(ServerState {
        config: config.clone(),
        index_db_pool,
        stripe_client,
        files_db_client,
        android_publisher,
        gcp_pubsub,
    });

    feature_flags::initialize_flags(&server_state).await;

    let routes = core_routes(&server_state)
        .or(build_info())
        .or(get_metrics())
        .or(stripe_webhooks(&server_state))
        .or(android_notification_webhooks(&server_state));

    let server = warp::serve(routes);

    metrics::start_metrics_worker(&server_state);

    // *** How people can connect to this server ***
    match (config.server.ssl_cert_location, config.server.ssl_private_key_location) {
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

async fn get_android_and_gcp_client(config: &Config) -> (Option<google_androidpublisher3::AndroidPublisher>, Option<google_pubsub1::Pubsub>) {
    match &config.google.service_account_cred_path {
        None => (None, None),
        Some(cred_path) => {
            let service_account_key: google_androidpublisher3::oauth2::ServiceAccountKey =
                google_androidpublisher3::oauth2::read_service_account_key(
                    cred_path,
                )
                    .await
                    .unwrap();

            let auth =
                google_androidpublisher3::oauth2::ServiceAccountAuthenticator::builder(service_account_key)
                    .build()
                    .await
                    .unwrap();

            let client = hyper::Client::builder().build(
                hyper_rustls::HttpsConnectorBuilder::with_native_roots(Default::default())
                    .https_or_http()
                    .enable_http1()
                    .enable_http2()
                    .build(),
            );


            (Some(google_androidpublisher3::AndroidPublisher::new(client.clone(), auth.clone())), Some(google_pubsub1::Pubsub::new(client, auth)))
        }
    }
}
