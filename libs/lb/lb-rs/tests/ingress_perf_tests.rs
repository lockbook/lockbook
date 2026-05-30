use std::io::{Read, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use lb_rs::Lb;
use lb_rs::model::core_config::Config;
use lb_rs::service::events::{Event, SyncIncrement};
use lb_rs::service::import_export::{ExportFileInfo, ImportStatus};
use rand::RngCore;
use sha2::{Digest, Sha256};
use test_utils::{
    generate_premium_account_tier, random_name, test_core_from, test_credit_cards, url,
};
use uuid::Uuid;

const ONE_MIB: usize = 1024 * 1024;

const FIXTURE_PATH: &str = "/Users/parth/Downloads/egui.mov";

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

/// Peak resident-set size the kernel has observed for this process,
/// in bytes. `getrusage`'s `ru_maxrss` is monotonically non-decreasing
/// across calls, so polling it at phase boundaries gives a clean
/// high-water mark without a separate sampling thread.
///
/// macOS reports `ru_maxrss` in bytes; Linux reports kilobytes.
fn max_rss_bytes() -> u64 {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::uninit();
    let ret = unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) };
    if ret != 0 {
        return 0;
    }
    let usage = unsafe { usage.assume_init() };
    let rss = usage.ru_maxrss as u64;
    if cfg!(target_os = "linux") { rss * 1024 } else { rss }
}

fn print_rss(label: &str) {
    let bytes = max_rss_bytes();
    let mib = bytes as f64 / ONE_MIB as f64;
    println!("  [peak rss] {label}: {mib:.1} MiB ({bytes} bytes)");
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
    let file_size = std::fs::metadata(doc_path)
        .unwrap_or_else(|e| panic!("fixture {} not accessible: {e}", doc_path.display()))
        .len() as usize;
    println!("using fixture at {} ({} bytes)", doc_path.display(), file_size);
    print_rss("baseline (before Lb::init)");

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
        mib_per_sec(file_size, import_elapsed)
    );
    print_rss("after import_files");

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
        mib_per_sec(file_size, sync_elapsed)
    );
    print_rss("after core.sync (push)");

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
        mib_per_sec(file_size, pull_elapsed)
    );
    print_rss("after core2.sync (pull metadata)");

    let read_start = Instant::now();
    let downloaded = core2.read_document(doc_id, false).await.unwrap();
    let read_elapsed = read_start.elapsed();
    println!(
        "read_document:     {:?} ({:.1} MiB/s)",
        read_elapsed,
        mib_per_sec(file_size, read_elapsed)
    );
    print_rss("after read_document (decrypted Vec in memory)");

    assert_eq!(downloaded.len(), file_size, "downloaded size differs from fixture");

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

    // Also exercise the on-disk export path. `read_document` returns a
    // `Vec<u8>` so it can't catch the Linux short-write bug in
    // `import_export::export_file_recursively` where a single `.write(...)`
    // on a >2 GiB slice was capped at `MAX_RW_COUNT` (~2.15 GB). Doing a
    // real export to disk and comparing the on-disk hash to the fixture
    // hash proves the `write_all` fix actually flushes the full buffer.
    let export_dir = std::path::PathBuf::from(format!("/tmp/{}", Uuid::new_v4()));
    std::fs::create_dir(&export_dir).unwrap();
    println!("exporting to disk at {} ...", export_dir.display());
    let export_start = Instant::now();
    core2
        .export_file(doc_id, export_dir.clone(), false, &None::<fn(ExportFileInfo)>)
        .await
        .unwrap();
    let export_elapsed = export_start.elapsed();
    let exported_path = export_dir.join(doc_path.file_name().unwrap());
    let exported_size = std::fs::metadata(&exported_path).unwrap().len() as usize;
    println!(
        "export_file:       {:?} ({:.1} MiB/s) -> {} bytes",
        export_elapsed,
        mib_per_sec(file_size, export_elapsed),
        exported_size
    );
    assert_eq!(
        exported_size, file_size,
        "exported size differs from fixture (Linux short-write bug?)"
    );

    let exported_hash = {
        let mut h = Sha256::new();
        let mut f = std::fs::File::open(&exported_path).unwrap();
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
    assert_eq!(exported_hash, fixture_hash, "exported bytes don't match fixture");
    println!("on-disk export verified: sha256 = {:x}", exported_hash);
    print_rss("after export_file (peak across whole test)");

    // Clean up the export tmp dir on success. (On failure we leave it for
    // post-mortem inspection.)
    let _ = std::fs::remove_dir_all(&export_dir);
}
