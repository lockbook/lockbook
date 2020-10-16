use cpuprofiler::PROFILER;
use criterion::profiler::Profiler;
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use lockbook_core::model::crypto::DecryptedValue;
use lockbook_core::model::file_metadata::FileType::Document;
use lockbook_core::model::state::Config;
use lockbook_core::repo::file_metadata_repo::FileMetadataRepo;
use lockbook_core::service::account_service::AccountService;
use lockbook_core::service::file_service::FileService;
use lockbook_core::service::sync_service::SyncService;
use lockbook_core::{
    connect_to_db, DefaultAccountService, DefaultFileMetadataRepo, DefaultFileService,
    DefaultSyncService,
};
use rand::distributions::Alphanumeric;
use rand::{self, Rng};
use std::fs;
use std::path::Path;
use uuid::Uuid;

struct CpuProfiler;
impl Profiler for CpuProfiler {
    fn start_profiling(&mut self, benchmark_id: &str, benchmark_dir: &Path) {
        fs::create_dir_all(benchmark_dir).unwrap();

        let profile_name = format!(
            "./{}/{}.profile",
            benchmark_dir.to_str().unwrap(),
            benchmark_id.to_string().replace("/", ".")
        );

        PROFILER.lock().unwrap().start(profile_name).unwrap();
    }

    fn stop_profiling(&mut self, benchmark_id: &str, benchmark_dir: &Path) {
        PROFILER.lock().unwrap().stop().unwrap();
    }
}

pub fn bench_performator(c: &mut Criterion) {
    let config = &Config {
        writeable_path: format!("/tmp/perf-{}", Uuid::new_v4().to_string()),
    };

    let db = &connect_to_db(config).unwrap();

    let account_string = "\
DQAAAAAAAABwZXJmb3JtYW5hdG9yEwAAAAAAAABodHRwOi8vYXJiaXRlcjo4MDAwQAAAAAAAAAD3YFgKz8Ju3TFV8urdQr4koul424BR8SUdwpHBVdPLSTBADhBttUhSeDa/fDaa+NEnuz/FH\
F1HSSUzgIZW+ok6kJkkwGZGtubYTArzUJgCQZQgtMYzRFTPb/WuaXxDzdk+9AxGXyUaFtnnJ/bAA3WsJUTW4445ztG8+QkAuYx5mf23F0Aixhr7IPV2N+K6+SLtGlz78LJMmxN3ZjMYU0RPHn\
tS32pBK497Ir8nnvabyR8My8DwPolGtKBY+6uNOamPwvBmApIGbXoRcJxHWt0BM5UI2QfdVHaZCV1nJfBhn4b2PvPf3i/O4lJYwOw267UDpvN47FNLPXo34WTI1t7aAQAAAAAAAAABAAEAQAA\
AAAAAAACRWrzfrIDzlsn1L4Z/pKYLqYpD1lcqtttsNAAj07LnUKx7V2XPYf206IUKzWmRsXyAeTUz1M0dMcZP3ZHihHRkbOUir3y5vuY6qaJxDiG3xUCA5NKQy0y58tuI2uQP9w5ZAkzaS/Eh\
G2fxFzltVG4P2TLupshfu+ufgP8FK0+5oFlCDwfCNfIyliJhEk/F7EyqPcJ4phG7oFzf9wex4ZqLUvpy2hAxOnO9VbrMelzZss8hdD6n9ODuMbsHaTvaUpJ0c/XYtGvpqoiQc4/nWCOXYnIlF\
UMDdXV3gGzaQl0yJc+dqFQJ0si0V7pY01Eyd+ne3kSOnE09T+pSge2MltKhAgAAAAAAAAAgAAAAAAAAAH9qG5cJuITKaKf87JTCSgXcQI7Ujwd8b0V+yPtcEcJXg0fDmJ2RarcPonD9jMqPgV\
W9b+VNGt5e8urXMQb384rzGPAqBxjgsceRXbgIwwES9CW6NY1qm8mRYcpKwSJ1bfY7zSMzLod52hyE4+41E7CWLaUxLUGof0KV6g3FETv6IAAAAAAAAACJHWSIusgVenaApKxY270ZFfwXCzO\
agoQ0YqkyZJ2wZE78FctCkLor8o3aWN/2ycsVMdUMOvrwec515LTv2KcQcyR3P/JPVCd2BinEYMR2kF/xtxDP6Qj1jrMeT4GsLFqxjswQ5Tb2eqUgj43xQVFkrw5OnyFXSC+u0eTpMKzq3w==";

    let _ = DefaultAccountService::import_account(db, account_string).unwrap();
    let _ = DefaultSyncService::sync(db).unwrap();
    let root = DefaultFileMetadataRepo::get_root(db).unwrap().unwrap();

    let mut group = c.benchmark_group("simple");

    let bytes = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(1000)
        .collect::<String>();

    group.throughput(Throughput::Bytes(bytes.len() as u64));
    group.bench_function("create_write_read", |b| {
        b.iter(|| {
            let file =
                DefaultFileService::create(db, &Uuid::new_v4().to_string(), root.id, Document)
                    .unwrap();

            let _ = DefaultFileService::write_document(
                db,
                file.id,
                &DecryptedValue::from(bytes.clone()),
            )
            .unwrap();

            let _ = DefaultFileService::read_document(db, file.id).unwrap();
        });
    });

    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default().with_profiler(CpuProfiler);
    targets = bench_performator
}
criterion_main!(benches);
