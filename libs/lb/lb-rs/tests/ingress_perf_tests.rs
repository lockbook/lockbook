use std::io::{Read, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use lb_rs::Lb;
use lb_rs::model::core_config::Config;
use lb_rs::service::events::{Event, SyncIncrement};
use lb_rs::service::import_export::ImportStatus;
use rand::RngCore;
use sha2::{Digest, Sha256};
use test_utils::{generate_premium_account_tier, random_name, test_core_from, test_credit_cards, url};
use uuid::Uuid;

const ONE_MIB: usize = 1024 * 1024;
const TWO_GB: usize = 1024 * ONE_MIB * 2;

/// Fixed path so reruns reuse the same file. Delete it to force regeneration.
const FIXTURE_PATH: &str = "/tmp/lockbook-ingress-perf-1gib.bin";

fn ensure_random_file(path: &Path, bytes: usize) {
    if let Ok(meta) = std::fs::metadata(path) {
        if meta.is_file() && meta.len() == bytes as u64 {
            println!("reusing existing fixture at {} ({} bytes)", path.display(), meta.len());
            return;
        }
        // Wrong size (likely a partial write from a previous run) — replace it.
        let _ = std::fs::remove_file(path);
    }

    println!("generating {} byte random file at {}...", bytes, path.display());
    let gen_start = Instant::now();
    let mut file = std::fs::File::create(path).unwrap();
    let mut rng = rand::thread_rng();
    let mut buf = vec![0u8; 4 * ONE_MIB];
    let mut remaining = bytes;
    while remaining > 0 {
        let chunk = remaining.min(buf.len());
        rng.fill_bytes(&mut buf[..chunk]);
        file.write_all(&buf[..chunk]).unwrap();
        remaining -= chunk;
    }
    file.flush().unwrap();
    let elapsed = gen_start.elapsed();
    println!("  file generation: {:?} ({:.1} MiB/s)", elapsed, mib_per_sec(bytes, elapsed));
}

fn mib_per_sec(bytes: usize, elapsed: Duration) -> f64 {
    let secs = elapsed.as_secs_f64();
    if secs == 0.0 { 0.0 } else { (bytes as f64 / ONE_MIB as f64) / secs }
}

/// Like `test_utils::test_config` but with stdout logging on so the
/// underlying reqwest error (e.g. the body that EINVAL'd) is visible when
/// sync surfaces only `LbErrKind::ServerUnreachable`.
fn verbose_config() -> Config {
    Config {
        writeable_path: format!("/tmp/{}", Uuid::new_v4()),
        background_work: false,
        logs: true,
        stdout_logs: true,
        colored_logs: false,
    }
}

#[tokio::test]
#[ignore = "generates a 1 GiB file and contacts the server"]
// this test cannot suceed on macOS until we do streaming sends from the server like we did for
// network.rs
async fn ingress_one_gib_single_file() {
    let doc_path = Path::new(FIXTURE_PATH);
    ensure_random_file(doc_path, TWO_GB);

    let core = Lb::init(verbose_config()).await.unwrap();
    core.create_account(&random_name(), &url(), false)
        .await
        .unwrap();

    // Upgrade to premium so the upload doesn't trip the free-tier usage cap.
    // Requires the server to have Stripe test mode configured.
    core.upgrade_account_stripe(generate_premium_account_tier(
        test_credit_cards::GOOD,
        None,
        None,
        None,
    ))
    .await
    .unwrap();

    let root = core.root().await.unwrap();

    // Subscribe to sync events and print them with elapsed time so we can
    // see exactly where push/pull stalls or fails. Spawned before sync so
    // we don't miss SyncStarted.
    let mut events = core.subscribe();
    let watch_start = Instant::now();
    let watcher = tokio::spawn(async move {
        loop {
            match events.recv().await {
                Ok(evt) => {
                    if let Event::Sync(s) = evt {
                        let stamp = watch_start.elapsed();
                        match s {
                            SyncIncrement::SyncStarted => println!("  [+{stamp:?}] sync started"),
                            SyncIncrement::PullingDocument(id, started) => println!(
                                "  [+{stamp:?}] pull doc {id} {}",
                                if started { "start" } else { "done" }
                            ),
                            SyncIncrement::PushingDocument(id, started) => println!(
                                "  [+{stamp:?}] push doc {id} {}",
                                if started { "start" } else { "done" }
                            ),
                            SyncIncrement::SyncFinished(err) => {
                                println!("  [+{stamp:?}] sync finished err={err:?}")
                            }
                        }
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    println!("  watcher lagged by {n} events");
                }
            }
        }
    });

    println!("importing into lockbook...");
    let import_start = Instant::now();
    // Capture the imported doc's id from the FinishedItem callback so the
    // second client knows what to read back.
    let imported_id: Arc<Mutex<Option<Uuid>>> = Arc::new(Mutex::new(None));
    let imported_id_cb = Arc::clone(&imported_id);
    core.import_files(&[doc_path.to_path_buf()], root.id, &|status: ImportStatus| match status {
        ImportStatus::CalculatedTotal(n) => println!("  import: total items = {n}"),
        ImportStatus::StartingItem(p) => println!("  import: starting {p}"),
        ImportStatus::FinishedItem(f) => {
            println!("  import: finished id={} name={}", f.id, f.name);
            *imported_id_cb.lock().unwrap() = Some(f.id);
        }
    })
    .await
    .unwrap();
    let import_elapsed = import_start.elapsed();
    println!(
        "import_files:    {:?} ({:.1} MiB/s)",
        import_elapsed,
        mib_per_sec(TWO_GB, import_elapsed)
    );

    // Pre-sync diagnostics so we know what sync is about to attempt.
    let status = core.status().await;
    println!("pre-sync status: {status:?}");
    match core.get_usage().await {
        Ok(usage) => println!("pre-sync server usage: {usage:?}"),
        Err(e) => println!("pre-sync get_usage failed: {:?}", e.kind),
    }

    println!("syncing to server...");
    let sync_start = Instant::now();
    let result = core.sync().await;
    let sync_elapsed = sync_start.elapsed();
    println!(
        "sync:            {:?} ({:.1} MiB/s)",
        sync_elapsed,
        mib_per_sec(TWO_GB, sync_elapsed)
    );

    // Stop the watcher cleanly before asserting.
    watcher.abort();

    if let Err(e) = result {
        // The `kind` is what classifies the failure; the long `backtrace`
        // string isn't useful on its own (LbErrKind::ServerUnreachable
        // discards the underlying reqwest cause). The actual cause appears
        // in the tracing output above — search stdout for
        // "network request send failed" or "network request took".
        panic!("sync failed: kind = {:?}", e.kind);
    }

    // Round-trip check: spin up a fresh client, pull the doc, and confirm
    // the bytes it reads match the fixture we uploaded.
    let doc_id = imported_id
        .lock()
        .unwrap()
        .expect("import never reported a FinishedItem");

    println!("starting fresh client to verify round-trip...");
    let core2 = test_core_from(&core).await;

    let pull_start = Instant::now();
    core2.sync().await.unwrap();
    let pull_elapsed = pull_start.elapsed();
    println!(
        "fresh-client sync: {:?} ({:.1} MiB/s)",
        pull_elapsed,
        mib_per_sec(TWO_GB, pull_elapsed)
    );

    let read_start = Instant::now();
    let downloaded = core2.read_document(doc_id, false).await.unwrap();
    let read_elapsed = read_start.elapsed();
    println!(
        "read_document:     {:?} ({:.1} MiB/s)",
        read_elapsed,
        mib_per_sec(TWO_GB, read_elapsed)
    );

    assert_eq!(downloaded.len(), TWO_GB, "downloaded size differs from fixture");

    // Compare via SHA-256 so we don't have to hold two 2 GiB buffers in
    // memory at once. Hash the downloaded buffer, drop it, then stream the
    // fixture off disk.
    let downloaded_hash = Sha256::digest(&downloaded);
    drop(downloaded);

    let fixture_hash = {
        let mut h = Sha256::new();
        let mut f = std::fs::File::open(doc_path).unwrap();
        let mut buf = vec![0u8; 4 * ONE_MIB];
        loop {
            let n = f.read(&mut buf).unwrap();
            if n == 0 {
                break;
            }
            h.update(&buf[..n]);
        }
        h.finalize()
    };

    assert_eq!(downloaded_hash, fixture_hash, "round-tripped bytes don't match fixture");
    println!("round-trip verified: sha256 = {:x}", fixture_hash);
}
