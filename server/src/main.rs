#![recursion_limit = "256"]

use account_dbs::AccountDbs;
use db_rs::Db;
use lb_rs::model::file_like::FileLike;
use lb_rs::model::server_meta::ServerMeta;
use lockbook_server_lib::billing::google_play_client::get_google_play_client;
use lockbook_server_lib::config::Config;
use lockbook_server_lib::document_service::OnDiskDocuments;
use lockbook_server_lib::router_service::{
    app_store_notification_webhooks, build_info, core_routes, get_metrics,
    google_play_notification_webhooks, stripe_webhooks,
};
use lockbook_server_lib::schema::ServerV4;
use lockbook_server_lib::*;
use schema::{Account, AccountV1, ServerV5};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
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

    let db_v4 = ServerV4::init(db_rs::Config::in_folder(&cfg.index_db.db_location))
        .expect("Failed to load index_db");
    if db_v4.incomplete_write().unwrap() {
        error!("dbrs indicated that the last write to the log was unsuccessful")
    }
    let db_v4 = Arc::new(Mutex::new(db_v4));
    spawn_compacter(&cfg, &db_v4);

    let db_v5 = ServerV5::init(db_rs::Config::in_folder(&cfg.index_db.db_location))
        .expect("Failed to load index_db");
    if db_v5.incomplete_write().unwrap() {
        error!("dbrs indicated that the last write to the log was unsuccessful")
    }
    let db_v5 = Arc::new(RwLock::new(db_v5));
    // todo: compaction
    let document_service = OnDiskDocuments::from(&config);

    let account_dbs = Default::default();

    let server_state = Arc::new(ServerState {
        config,
        db_v4,
        db_v5,
        account_dbs,
        stripe_client,
        google_play_client,
        app_store_client,
        document_service,
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

async fn migrate(
    cfg: Config, v4: Arc<Mutex<ServerV4>>, v5: Arc<RwLock<ServerV5>>, a_dbs: AccountDbs,
) {
    let v4 = v4.lock().await;
    let mut v5 = v5.write().await;
    let mut adbs = a_dbs.write().await;

    for (owner, old_account) in v4.accounts.get() {
        v5.accounts
            .insert(
                *owner,
                Account {
                    username: old_account.username.clone(),
                    billing_info: old_account.billing_info.clone(),
                },
            )
            .unwrap();

        let db = AccountV1::init(db_rs::Config::in_folder(&cfg.index_db.db_location)).unwrap();
        let db = Arc::new(RwLock::new(db));
        adbs.insert(*owner, db);
    }

    for (owner, ids) in v4.owned_files.get() {
        for id in ids {
            let meta = v4.metas.get().get(id).unwrap();
            let size = *v4.sizes.get().get(id).unwrap();
            let db = adbs.get_mut(&owner).unwrap();
            let mut db = db.write().await;

            db.metas
                .insert(*meta.id(), ServerMeta::from(meta.clone()))
                .unwrap();
            db.sizes.insert(*meta.id(), size).unwrap();
        }
    }

    for (owner, ids) in v4.shared_files.get() {
        for id in ids {
            let meta = v4.metas.get().get(id).unwrap();
            let db = adbs.get_mut(&owner).unwrap();
            let mut db = db.write().await;
            db.shared_files.push((*meta.id(), meta.owner())).unwrap();
        }
    }

    for (username, owner) in v4.usernames.get() {
        v5.usernames.insert(username.clone(), *owner).unwrap();
    }

    for (play_id, owner) in v4.google_play_ids.get() {
        v5.google_play_ids.insert(play_id.clone(), *owner).unwrap();
    }

    for (stripe_id, owner) in v4.stripe_ids.get() {
        v5.stripe_ids.insert(stripe_id.clone(), *owner).unwrap();
    }

    for (appstore_id, owner) in v4.app_store_ids.get() {
        v5.app_store_ids
            .insert(appstore_id.clone(), *owner)
            .unwrap();
    }
}

fn spawn_compacter(cfg: &Config, db: &Arc<Mutex<ServerV4>>) {
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
