use std::net::SocketAddr;

use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto;
use hyper_util::service::TowerToHyperService;
use lockbook_server_lib::ClientAddr;
use lockbook_server_lib::http_server::{serve, tls_acceptor};
use lockbook_server_lib::router_service::build_response;
use reqwest::{Certificate, Client, Version};
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;
use warp::http::StatusCode;
use warp::{Filter, Rejection, Reply};

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");
const OVER_H2_PAYLOAD_CAP: usize = (1 << 31) + (64 << 20);

fn routes() -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    let size =
        warp::path!("size" / usize).map(|n: usize| build_response(vec![b'x'; n], StatusCode::OK));
    let addr = warp::path!("addr")
        .and(warp::ext::optional::<ClientAddr>())
        .map(|addr: Option<ClientAddr>| {
            addr.map(|addr| addr.0.ip().to_string()).unwrap_or_default()
        });
    size.or(addr)
}

async fn start(acceptor: Option<TlsAcceptor>) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(serve(listener, acceptor, warp::service(routes())));
    addr
}

async fn start_without_splitting() -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let (stream, _) = listener.accept().await.unwrap();
            tokio::spawn(async move {
                let service = TowerToHyperService::new(warp::service(routes()));
                let _ = auto::Builder::new(TokioExecutor::new())
                    .serve_connection(TokioIo::new(stream), service)
                    .await;
            });
        }
    });
    addr
}

fn tls_client(cert: &str) -> Client {
    let pem = std::fs::read(format!("{FIXTURES}/{cert}")).unwrap();
    Client::builder()
        .add_root_certificate(Certificate::from_pem(&pem).unwrap())
        .build()
        .unwrap()
}

async fn download(client: &Client, url: &str) -> Result<(Version, usize), reqwest::Error> {
    let mut response = client.get(url).send().await?.error_for_status()?;
    let version = response.version();
    let mut total = 0;
    while let Some(chunk) = response.chunk().await? {
        total += chunk.len();
    }
    Ok((version, total))
}

#[tokio::test]
async fn http1_serves_with_content_length_and_client_addr() {
    let addr = start(None).await;
    let client = Client::builder().http1_only().build().unwrap();

    let response = client
        .get(format!("http://{addr}/size/12345"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.version(), Version::HTTP_11);
    assert_eq!(response.content_length(), Some(12345));
    assert_eq!(response.bytes().await.unwrap().len(), 12345);

    let ip = client
        .get(format!("http://{addr}/addr"))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert_eq!(ip, "127.0.0.1");
}

#[tokio::test]
async fn h2c_serves_and_splits_large_bodies() {
    let addr = start(None).await;
    let client = Client::builder().http2_prior_knowledge().build().unwrap();
    let (version, len) = download(&client, &format!("http://{addr}/size/{}", 20 << 20))
        .await
        .unwrap();
    assert_eq!(version, Version::HTTP_2);
    assert_eq!(len, 20 << 20);
}

#[tokio::test]
async fn h2_accepts_large_headers_like_hyper_0_14() {
    let addr = start(None).await;
    let client = Client::builder().http2_prior_knowledge().build().unwrap();
    let response = client
        .get(format!("http://{addr}/size/10"))
        .header("x-big", "a".repeat(64 * 1024))
        .send()
        .await
        .unwrap();
    assert_eq!(response.version(), Version::HTTP_2);
    assert_eq!(response.status(), 200);
}

#[tokio::test]
async fn tls_negotiates_h2_and_http1_for_every_key_format() {
    for (cert, key) in [
        ("localhost.crt", "localhost-pkcs8.key"),
        ("localhost.crt", "localhost-sec1.key"),
        ("localhost-rsa.crt", "localhost-pkcs1.key"),
    ] {
        let acceptor =
            tls_acceptor(&format!("{FIXTURES}/{cert}"), &format!("{FIXTURES}/{key}")).unwrap();
        let addr = start(Some(acceptor)).await;
        let url = format!("https://localhost:{}/size/1000", addr.port());

        let (version, len) = download(&tls_client(cert), &url).await.unwrap();
        assert_eq!((version, len), (Version::HTTP_2, 1000), "{key}");

        let http1 = Client::builder()
            .add_root_certificate(
                Certificate::from_pem(&std::fs::read(format!("{FIXTURES}/{cert}")).unwrap())
                    .unwrap(),
            )
            .http1_only()
            .build()
            .unwrap();
        let (version, len) = download(&http1, &url).await.unwrap();
        assert_eq!((version, len), (Version::HTTP_11, 1000), "{key}");

        let ip = tls_client(cert)
            .get(format!("https://localhost:{}/addr", addr.port()))
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        assert_eq!(ip, "127.0.0.1", "{key}");
    }
}

#[tokio::test]
async fn tls_rejects_missing_files() {
    assert!(tls_acceptor("/nonexistent.crt", &format!("{FIXTURES}/localhost-pkcs8.key")).is_err());
    assert!(tls_acceptor(&format!("{FIXTURES}/localhost.crt"), "/nonexistent.key").is_err());
}

#[tokio::test]
#[ignore = "allocates and transfers over 2 GiB"]
async fn h2_response_over_payload_cap() {
    let client = Client::builder().http2_prior_knowledge().build().unwrap();

    let unsplit = start_without_splitting().await;
    let result = download(&client, &format!("http://{unsplit}/size/{OVER_H2_PAYLOAD_CAP}")).await;
    assert!(result.is_err(), "an unsplit body over 2 GiB should fail on h2: {result:?}");

    let addr = start(None).await;
    let (version, len) = download(&client, &format!("http://{addr}/size/{OVER_H2_PAYLOAD_CAP}"))
        .await
        .unwrap();
    assert_eq!((version, len), (Version::HTTP_2, OVER_H2_PAYLOAD_CAP));

    let tls = start(Some(
        tls_acceptor(
            &format!("{FIXTURES}/localhost.crt"),
            &format!("{FIXTURES}/localhost-pkcs8.key"),
        )
        .unwrap(),
    ))
    .await;
    let url = format!("https://localhost:{}/size/{OVER_H2_PAYLOAD_CAP}", tls.port());
    let (version, len) = download(&tls_client("localhost.crt"), &url).await.unwrap();
    assert_eq!((version, len), (Version::HTTP_2, OVER_H2_PAYLOAD_CAP));
}
