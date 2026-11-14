use std::convert::Infallible;
use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use hyper::body::{Body, Bytes, Frame, Incoming, SizeHint};
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

pub const MAX_DATA_FRAME_BYTES: usize = 4 * 1024 * 1024;

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
    B: Body<Data = Bytes> + Send + Unpin + 'static,
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
    B: Body<Data = Bytes> + Send + Unpin + 'static,
    B::Error: Into<BoxError>,
{
    let service =
        TowerToHyperService::new(tower::service_fn(move |mut request: Request<Incoming>| {
            request.extensions_mut().insert(ClientAddr(peer));
            let service = service.clone();
            async move {
                let response = service.oneshot(request).await?;
                Ok::<_, Infallible>(response.map(|body| SplitBody::new(body, MAX_DATA_FRAME_BYTES)))
            }
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

pub struct SplitBody<B> {
    inner: B,
    pending: Bytes,
    max_frame: usize,
}

impl<B> SplitBody<B> {
    pub fn new(inner: B, max_frame: usize) -> Self {
        Self { inner, pending: Bytes::new(), max_frame }
    }

    fn take_pending(&mut self) -> Frame<Bytes> {
        let n = self.pending.len().min(self.max_frame);
        Frame::data(self.pending.split_to(n))
    }
}

impl<B> Body for SplitBody<B>
where
    B: Body<Data = Bytes> + Unpin,
{
    type Data = Bytes;
    type Error = B::Error;

    fn poll_frame(
        self: Pin<&mut Self>, cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, B::Error>>> {
        let this = self.get_mut();
        if !this.pending.is_empty() {
            return Poll::Ready(Some(Ok(this.take_pending())));
        }
        match Pin::new(&mut this.inner).poll_frame(cx) {
            Poll::Ready(Some(Ok(frame))) => match frame.into_data() {
                Ok(data) if data.len() > this.max_frame => {
                    this.pending = data;
                    Poll::Ready(Some(Ok(this.take_pending())))
                }
                Ok(data) => Poll::Ready(Some(Ok(Frame::data(data)))),
                Err(frame) => Poll::Ready(Some(Ok(frame))),
            },
            other => other,
        }
    }

    fn is_end_stream(&self) -> bool {
        self.pending.is_empty() && self.inner.is_end_stream()
    }

    fn size_hint(&self) -> SizeHint {
        let inner = self.inner.size_hint();
        let pending = self.pending.len() as u64;
        let mut hint = SizeHint::new();
        hint.set_lower(inner.lower() + pending);
        if let Some(upper) = inner.upper() {
            hint.set_upper(upper + pending);
        }
        hint
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http_body_util::{BodyExt, Full};

    #[tokio::test]
    async fn splits_large_frames_and_keeps_exact_size() {
        let data: Vec<u8> = (0..10_000u32).map(|i| i as u8).collect();
        let mut body = SplitBody::new(Full::new(Bytes::from(data.clone())), 4096);
        assert_eq!(body.size_hint().exact(), Some(10_000));

        let mut sizes = Vec::new();
        let mut collected = Vec::new();
        while let Some(frame) = body.frame().await {
            let chunk = frame.unwrap().into_data().unwrap();
            sizes.push(chunk.len());
            collected.extend_from_slice(&chunk);
            assert_eq!(body.size_hint().exact(), Some((10_000 - collected.len()) as u64));
        }
        assert_eq!(sizes, vec![4096, 4096, 1808]);
        assert_eq!(collected, data);
        assert!(body.is_end_stream());
    }

    #[tokio::test]
    async fn leaves_small_frames_alone() {
        let mut body = SplitBody::new(Full::new(Bytes::from_static(b"hello")), 4096);
        let chunk = body.frame().await.unwrap().unwrap().into_data().unwrap();
        assert_eq!(&chunk[..], b"hello");
        assert!(body.frame().await.is_none());
    }
}
