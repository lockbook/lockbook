use std::io::Write;
use std::path::Path;
use std::time::{Duration, Instant};

use lb_rs::service::events::{Event, SyncIncrement};
use lb_rs::service::import_export::ImportStatus;
use rand::RngCore;
use test_utils::{generate_premium_account_tier, test_core_with_account, test_credit_cards};

const ONE_MIB: usize = 1024 * 1024;
const ONE_GIB: usize = 1024 * ONE_MIB;

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

#[tokio::test]
#[ignore = "generates a 1 GiB file and contacts the server"]
async fn ingress_one_gib_single_file() {
    let doc_path = Path::new(FIXTURE_PATH);
    ensure_random_file(doc_path, ONE_GIB);

    let core = test_core_with_account().await;

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
    core.import_files(&[doc_path.to_path_buf()], root.id, &|status: ImportStatus| match status {
        ImportStatus::CalculatedTotal(n) => println!("  import: total items = {n}"),
        ImportStatus::StartingItem(p) => println!("  import: starting {p}"),
        ImportStatus::FinishedItem(f) => {
            println!("  import: finished id={} name={}", f.id, f.name)
        }
    })
    .await
    .unwrap();
    let import_elapsed = import_start.elapsed();
    println!(
        "import_files:    {:?} ({:.1} MiB/s)",
        import_elapsed,
        mib_per_sec(ONE_GIB, import_elapsed)
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
        mib_per_sec(ONE_GIB, sync_elapsed)
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
}
