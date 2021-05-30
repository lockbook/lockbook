use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use lockbook_core::model::state::Config;
use lockbook_core::repo::file_metadata_repo;
use lockbook_core::service::account_service::AccountService;
use lockbook_core::service::file_service::FileService;
use lockbook_core::service::sync_service::SyncService;
use lockbook_core::{DefaultAccountService, DefaultFileService, DefaultSyncService};
use lockbook_models::file_metadata::FileType::Document;
use rand::distributions::Alphanumeric;
use rand::{self, Rng};
use std::env;
use uuid::Uuid;

fn random_string() -> String {
    Uuid::new_v4()
        .to_string()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect()
}
pub fn bench_throughput(c: &mut Criterion) {
    let config = &Config {
        writeable_path: format!("/tmp/throughput{}", random_string()),
    };

    let mut group = c.benchmark_group("Throughput");

    let config_string = "File";

    let _ = DefaultAccountService::create_account(
        config,
        format!("throughput{}", random_string()).as_str(),
        env::var("API_URL")
            .expect("API_URL must be defined!")
            .as_str(),
    )
    .unwrap();
    let _ = DefaultSyncService::sync(config, None).unwrap();
    let root = file_metadata_repo::get_root(config).unwrap().unwrap();

    for x in vec![1, 1000, 10000, 100000, 1000000] {
        let bytes = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(x)
            .collect::<String>()
            .into_bytes();

        // File to be used in benchmark
        let file =
            DefaultFileService::create(config, &Uuid::new_v4().to_string(), root.id, Document)
                .unwrap();

        group.throughput(Throughput::Bytes(bytes.len() as u64));
        group.bench_with_input(
            BenchmarkId::new(format!("{}-Write", config_string), bytes.len()),
            &bytes,
            |b, _| {
                b.iter(|| {
                    let _ = DefaultFileService::write_document(config, file.id, &bytes.clone())
                        .unwrap();
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new(format!("{}-Read", config_string), bytes.len()),
            &bytes,
            |b, _| {
                DefaultFileService::write_document(config, file.id, &bytes.clone()).unwrap();
                b.iter(|| {
                    let _ = DefaultFileService::read_document(config, file.id).unwrap();
                });
            },
        );
    }

    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default();
    targets = bench_throughput
}
criterion_main!(benches);
