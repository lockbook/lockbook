extern crate chrono;
extern crate log;
extern crate tokio;

use async_stream::stream;
use futures_util::future::TryFutureExt;
use hyper::server::accept;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Server};
use lockbook_server_lib::config::Config;
use lockbook_server_lib::*;
use std::convert::Infallible;
use std::sync::Arc;
use std::{io, sync};
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;

// TODO this must go
fn error(err: String) -> io::Error {
    io::Error::new(io::ErrorKind::Other, err)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config = Config::from_env_vars();

    loggers::init(&config);

    let index_db_client = file_index_repo::connect(&config.index_db)
        .await
        .expect("Failed to connect to index_db");

    let files_db_client = file_content_client::create_client(&config.files_db)
        .expect("Failed to create files_db client");

    // Build TLS configuration.
    let tls_cfg = {
        // Load public certificate.
        let certs = load_certs(config.server.ssl_cert_location.as_ref().unwrap())?;
        // Load private key.
        let key = load_private_key(config.server.ssl_private_key_location.as_ref().unwrap())?;
        // Do not use client certificate authentication.
        let mut cfg = rustls::ServerConfig::builder()
            .with_safe_defaults()
            .with_no_client_auth()
            .with_single_cert(certs, key)
            .map_err(|e| error(format!("{}", e)))?;
        // Configure ALPN to accept HTTP/2, HTTP/1.1 in that order.
        cfg.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
        sync::Arc::new(cfg)
    };

    let server_state = Arc::new(ServerState {
        config: config.clone(),
        index_db_client,
        files_db_client,
    });
    let addr = format!("0.0.0.0:{}", &config.server.port);

    // Create a TCP listener via tokio.
    let tcp = TcpListener::bind(&addr).await?;
    let tls_acceptor = TlsAcceptor::from(tls_cfg);
    // Prepare a long-running future stream to accept and serve clients.
    let incoming_tls_stream = stream! {
        loop {
            let (socket, _) = tcp.accept().await?;
            let stream = tls_acceptor.accept(socket).map_err(|e| {
                println!("[!] Voluntary server halt due to client-connection error...");
                // Errors could be handled here, instead of server aborting.
                // Ok(None)
                error(format!("TLS Error: {:?}", e))
            });
            yield stream.await;
        }
    };
    let acceptor = accept::from_stream(incoming_tls_stream);
    let make_service = make_service_fn(move |_| {
        let server_state = Arc::clone(&server_state);
        async {
            Ok::<_, Infallible>(service_fn(move |req: hyper::Request<Body>| {
                let server_state = Arc::clone(&server_state);
                async move { router_service::route(&server_state, req).await }
            }))
        }
    });
    let server = Server::builder(acceptor).serve(make_service);
    println!("Starting to serve on https://{:?}.", addr);
    server.await?;
    Ok(())
}
