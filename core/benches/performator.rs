use cpuprofiler::PROFILER;
use criterion::profiler::Profiler;
use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use lockbook_core::model::state::Config;
use lockbook_core::repo::remote_metadata_repo;
use lockbook_core::service::{account_service, file_service, sync_service};
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

    let _ = account_service::create_account(
        &config,
        "performator",
        env::var("API_URL")
            .expect("API_URL must be defined!")
            .as_str(),
    )
    .unwrap();
    let _ = sync_service::sync(config, None).unwrap();
    let root = remote_metadata_repo::get_root(config).unwrap().unwrap();

    let mut group = c.benchmark_group("simple");

    let bytes = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(1000)
        .collect::<String>()
        .into_bytes();

    group.throughput(Throughput::Bytes(bytes.len() as u64));
    group.bench_function("create_write_read", |b| {
        b.iter(|| {
            let file = file_service::create(config, &Uuid::new_v4().to_string(), root.id, Document)
                .unwrap();

            let _ = file_service::write_document(config, file.id, &bytes.clone()).unwrap();

            let _ = file_service::read_document(config, file.id).unwrap();
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
