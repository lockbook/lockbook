//! Verifies `read_timeout` against a real `Lb` talking to a real server
//! through a tiny TCP proxy. The proxy is the only extra piece: lb-rs and
//! the server are unmodified.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use lb_rs::model::errors::LbErrKind;
use test_utils::*;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const LARGE_DOC_BYTES: usize = 2 * 1024 * 1024;
const TRICKLE_DOC_BYTES: usize = 512 * 1024;
/// ~43s to push 512 KiB — longer than `READ_TIMEOUT` (30s) if the timer
/// were a total deadline, short enough that a stall-resetting timeout
/// still succeeds.
const TRICKLE_BYTES_PER_SEC: usize = 12 * 1024;
const READ_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone, Copy)]
enum Mode {
    Forward,
    Blackhole,
    Trickle,
}

struct Proxy {
    url: String,
    mode: Arc<Mutex<Mode>>,
    conns: Arc<Mutex<Vec<tokio::task::JoinHandle<()>>>>,
}

impl Proxy {
    async fn bind(backend: String) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let mode = Arc::new(Mutex::new(Mode::Forward));
        let conns: Arc<Mutex<Vec<tokio::task::JoinHandle<()>>>> = Arc::new(Mutex::new(Vec::new()));
        let mode_clone = mode.clone();
        let conns_clone = conns.clone();
        tokio::spawn(async move {
            loop {
                let Ok((inbound, _)) = listener.accept().await else { break };
                let backend = backend.clone();
                let mode = *mode_clone.lock().unwrap();
                let handle = tokio::spawn(async move {
                    match mode {
                        Mode::Blackhole => {
                            tokio::time::sleep(Duration::from_secs(120)).await;
                            drop(inbound);
                        }
                        Mode::Forward => {
                            if let Ok(mut outbound) = TcpStream::connect(&backend).await {
                                let mut inbound = inbound;
                                let _ = tokio::io::copy_bidirectional(&mut inbound, &mut outbound)
                                    .await;
                            }
                        }
                        Mode::Trickle => {
                            if let Ok(outbound) = TcpStream::connect(&backend).await {
                                let (mut ri, mut wi) = inbound.into_split();
                                let (mut ro, mut wo) = outbound.into_split();
                                let _ = tokio::try_join!(
                                    copy_rate_limited(&mut ri, &mut wo, TRICKLE_BYTES_PER_SEC),
                                    copy_rate_limited(&mut ro, &mut wi, TRICKLE_BYTES_PER_SEC),
                                );
                            }
                        }
                    }
                });
                conns_clone.lock().unwrap().push(handle);
            }
        });
        Self { url: format!("http://{addr}"), mode, conns }
    }

    /// Drop pooled TCP connections so the next request observes the new mode.
    fn set(&self, mode: Mode) {
        *self.mode.lock().unwrap() = mode;
        for handle in self.conns.lock().unwrap().drain(..) {
            handle.abort();
        }
    }
}

async fn copy_rate_limited<R, W>(
    src: &mut R, dst: &mut W, bytes_per_sec: usize,
) -> std::io::Result<()>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut buf = vec![0u8; 1024];
    loop {
        let n = src.read(&mut buf).await?;
        if n == 0 {
            break;
        }
        dst.write_all(&buf[..n]).await?;
        dst.flush().await?;
        let sleep = Duration::from_secs_f64(n as f64 / bytes_per_sec as f64);
        tokio::time::sleep(sleep).await;
    }
    Ok(())
}

fn backend_addr() -> String {
    url()
        .trim_start_matches("http://")
        .trim_start_matches("https://")
        .replacen("localhost", "127.0.0.1", 1)
}

async fn point_at(proxy: &Proxy) -> lb_rs::Lb {
    let core = test_core().await;
    core.create_account(&random_name(), &proxy.url, false)
        .await
        .unwrap();
    core
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sync_read_timeout_via_proxy() {
    let proxy = Proxy::bind(backend_addr()).await;
    let core = point_at(&proxy).await;

    // Real stack, unthrottled: a 2 MiB document still syncs.
    let doc = core.create_at_path("/big.bin").await.unwrap();
    core.write_document(doc.id, &vec![0u8; LARGE_DOC_BYTES])
        .await
        .unwrap();
    core.sync().await.unwrap();
    assert!(local(&core).syncer.try_lock().is_ok(), "mutex held after successful sync");

    // Blackhole: first API call of sync (get-file-ids) never gets a
    // response. `read_timeout` must fire and drop the sync mutex.
    proxy.set(Mode::Blackhole);
    let start = Instant::now();
    let timed = tokio::time::timeout(Duration::from_secs(45), core.sync()).await;
    let elapsed = start.elapsed();
    let err = timed
        .unwrap_or_else(|_| panic!("sync hung for {elapsed:?}, mutex likely still held"))
        .unwrap_err();
    assert_eq!(err.kind, LbErrKind::ServerUnreachable, "{err:?}");
    assert!(
        elapsed >= READ_TIMEOUT - Duration::from_secs(2),
        "timed out too fast ({elapsed:?}), not the 30s read_timeout"
    );
    assert!(elapsed < Duration::from_secs(45), "timed out too slow ({elapsed:?})");
    assert!(local(&core).syncer.try_lock().is_ok(), "sync mutex still held after read_timeout");

    // Mutex is actually usable: a second sync through a live proxy works.
    proxy.set(Mode::Forward);
    core.sync().await.unwrap();

    // Trickle: data keeps moving, but the transfer lasts longer than
    // 30s. A total `timeout()` would kill it; `read_timeout` must not.
    proxy.set(Mode::Trickle);
    let trickle = core.create_at_path("/trickle.bin").await.unwrap();
    core.write_document(trickle.id, &vec![0u8; TRICKLE_DOC_BYTES])
        .await
        .unwrap();
    let start = Instant::now();
    core.sync().await.unwrap();
    let elapsed = start.elapsed();
    assert!(
        elapsed > READ_TIMEOUT,
        "trickle sync finished in {elapsed:?}, need >30s to prove this is not a total timeout"
    );
}
