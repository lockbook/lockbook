use cpuprofiler::PROFILER;
use criterion::profiler::Profiler;
use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use lockbook_core::model::state::Config;
use lockbook_core::repo::file_metadata_repo::FileMetadataRepo;
use lockbook_core::service::account_service::AccountService;
use lockbook_core::service::file_service::FileService;
use lockbook_core::service::sync_service::SyncService;
use lockbook_core::{
    DefaultAccountService, DefaultFileMetadataRepo, DefaultFileService, DefaultSyncService,
};
use lockbook_models::file_metadata::FileType::Document;
use rand::distributions::Alphanumeric;
use rand::{self, Rng};
use std::env;
use std::path::Path;
use uuid::Uuid;

struct CpuProfiler;
impl Profiler for CpuProfiler {
    fn start_profiling(&mut self, benchmark_id: &str, _benchmark_dir: &Path) {
        let profile_name = format!("./{}.profile", benchmark_id.to_string().replace("/", "-"));

        PROFILER.lock().unwrap().start(profile_name).unwrap();
    }

    fn stop_profiling(&mut self, _benchmark_id: &str, _benchmark_dir: &Path) {
        PROFILER.lock().unwrap().stop().unwrap();
    }
}

pub fn bench_performator(c: &mut Criterion) {
    let config = &Config {
        writeable_path: format!("/tmp/perf-{}", Uuid::new_v4().to_string()),
    };

    let backend = FileBackend::connect_to_db(config);

    let _ = DefaultAccountService::create_account(
        backend,
        "performator",
        env::var("API_URL")
            .expect("API_URL must be defined!")
            .as_str(),
    )
    .unwrap();
    let _ = DefaultSyncService::sync(backend).unwrap();
    let root = DefaultFileMetadataRepo::get_root(backend).unwrap().unwrap();

    let mut group = c.benchmark_group("simple");

    let bytes = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(1000)
        .collect::<String>()
        .into_bytes();

    group.throughput(Throughput::Bytes(bytes.len() as u64));
    group.bench_function("create_write_read", |b| {
        b.iter(|| {
            let file =
                DefaultFileService::create(backend, &Uuid::new_v4().to_string(), root.id, Document)
                    .unwrap();

            let _ = DefaultFileService::write_document(backend, file.id, &bytes.clone()).unwrap();

            let _ = DefaultFileService::read_document(backend, file.id).unwrap();
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
