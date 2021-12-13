use crate::config::{Config};
use crate::{load_certs, load_private_key, error, ServerState, router_service};
use std::sync;
use async_stream::stream;
use std::sync::Arc;
use tokio_rustls::rustls::ServerConfig;
use tokio_rustls::TlsAcceptor;
use tokio::net::TcpListener;
use hyper::server::accept;
use hyper::{Body, Server};
use hyper::service::{make_service_fn, service_fn};
use std::convert::Infallible;
use futures::TryFutureExt;

pub async fn https(config: &Config, server_state: Arc<ServerState>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let addr = format!("0.0.0.0:{}", &config.server.port);
    let tcp = TcpListener::bind(&addr).await?;

    let tls_acceptor = tls_acceptor(&config);
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
    let server = Server::builder(acceptor).serve(make_service_fn(move |_| {
        let server_state = Arc::clone(&server_state);
        async {
            Ok::<_, Infallible>(service_fn(move |req: hyper::Request<Body>| {
                let server_state = Arc::clone(&server_state);
                async move { router_service::route(&server_state, req).await }
            }))
        }
    }));
    server.await?;
    Ok(())
}

fn tls_acceptor(config: &Config) -> TlsAcceptor {
    // Load public certificate.
    let certs = load_certs(config.server.ssl_cert_location.as_ref().unwrap()).expect("failed to load cert");
    // Load private key.
    let key = load_private_key(config.server.ssl_private_key_location.as_ref().unwrap()).expect("failed to load private_key");
    // Do not use client certificate authentication.
    let mut cfg = rustls::ServerConfig::builder()
        .with_safe_defaults()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .expect("failed to create tls config");
    // Configure ALPN to accept HTTP/2, HTTP/1.1 in that order.
    cfg.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
    let tls_cfg = sync::Arc::new(cfg);

    // Create a TCP listener via tokio.
    TlsAcceptor::from(tls_cfg)
}