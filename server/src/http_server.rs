use std::convert::Infallible;
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use hyper::body::{Body, Bytes, Incoming};
use hyper::{Request, Response};
use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto;
use hyper_util::service::TowerToHyperService;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;
use tokio_rustls::rustls::ServerConfig;
use tokio_rustls::rustls::crypto::ring;
use tokio_rustls::rustls::pki_types::pem::PemObject;
use tokio_rustls::rustls::pki_types::{CertificateDer, PrivateKeyDer};
use tower::{Service, ServiceExt};
use tracing::*;

use crate::ClientAddr;

pub type BoxError = Box<dyn std::error::Error + Send + Sync>;

pub fn tls_acceptor(cert: &str, key: &str) -> Result<TlsAcceptor, BoxError> {
    let certs = CertificateDer::pem_file_iter(cert)?.collect::<Result<Vec<_>, _>>()?;
    let key = PrivateKeyDer::from_pem_file(key)?;
    let mut config = ServerConfig::builder_with_provider(Arc::new(ring::default_provider()))
        .with_safe_default_protocol_versions()?
        .with_no_client_auth()
        .with_single_cert(certs, key)?;
    config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
    Ok(TlsAcceptor::from(Arc::new(config)))
}

pub async fn serve<S, B>(listener: TcpListener, acceptor: Option<TlsAcceptor>, service: S)
where
    S: Service<Request<Incoming>, Response = Response<B>, Error = Infallible>
        + Clone
        + Send
        + 'static,
    S::Future: Send + 'static,
    B: Body<Data = Bytes> + Send + 'static,
    B::Error: Into<BoxError>,
{
    loop {
        let (stream, peer) = match listener.accept().await {
            Ok(accepted) => accepted,
            Err(e) if is_connection_error(&e) => continue,
            Err(e) => {
                debug!("accept error: {e:?}");
                tokio::time::sleep(Duration::from_secs(1)).await;
                continue;
            }
        };
        let _ = stream.set_nodelay(true);
        let service = service.clone();
        let acceptor = acceptor.clone();
        tokio::spawn(async move {
            match acceptor {
                Some(acceptor) => {
                    if let Ok(stream) = acceptor.accept(stream).await {
                        serve_connection(stream, peer, service).await
                    }
                }
                None => serve_connection(stream, peer, service).await,
            }
        });
    }
}

async fn serve_connection<I, S, B>(io: I, peer: SocketAddr, service: S)
where
    I: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    S: Service<Request<Incoming>, Response = Response<B>, Error = Infallible>
        + Clone
        + Send
        + 'static,
    S::Future: Send + 'static,
    B: Body<Data = Bytes> + Send + 'static,
    B::Error: Into<BoxError>,
{
    let service =
        TowerToHyperService::new(tower::service_fn(move |mut request: Request<Incoming>| {
            request.extensions_mut().insert(ClientAddr(peer));
            service.clone().oneshot(request)
        }));

    let mut builder = auto::Builder::new(TokioExecutor::new());
    builder.http1().header_read_timeout(None);
    builder
        .http2()
        .max_concurrent_streams(None)
        .max_header_list_size(16 << 20);

    let _ = builder
        .serve_connection_with_upgrades(TokioIo::new(io), service)
        .await;
}

fn is_connection_error(e: &io::Error) -> bool {
    matches!(
        e.kind(),
        io::ErrorKind::ConnectionRefused
            | io::ErrorKind::ConnectionAborted
            | io::ErrorKind::ConnectionReset
    )
}
