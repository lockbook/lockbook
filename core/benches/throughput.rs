use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use lockbook_core::model::file_metadata::FileType::Document;
use lockbook_core::model::state::Config;
use lockbook_core::repo::file_metadata_repo::FileMetadataRepo;
use lockbook_core::service::account_service::AccountService;
use lockbook_core::service::file_service::FileService;
use lockbook_core::service::sync_service::SyncService;
use lockbook_core::storage::db_provider::{to_backend, Backend};
use lockbook_core::{
    connect_to_db, DefaultAccountService, DefaultFileMetadataRepo, DefaultFileService,
    DefaultSyncService,
};
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
    let id: String = random_string();

    let config = &Config {
        writeable_path: format!("/tmp/throughput{}", id),
    };

    let db = &connect_to_db(config).unwrap();
    let sled = &to_backend(db);
    let file = &Backend::File(config);

    let mut group = c.benchmark_group("Throughput");

    for backend in vec![file, sled] {
        let backend_string = match backend {
            Backend::Sled(_) => "Sled",
            Backend::File(_) => "File",
        };

        let _ = DefaultAccountService::create_account(
            backend,
            format!("throughput{}", random_string()).as_str(),
            env::var("API_URL")
                .expect("API_URL must be defined!")
                .as_str(),
        )
        .unwrap();
        let _ = DefaultSyncService::sync(backend).unwrap();
        let root = DefaultFileMetadataRepo::get_root(backend).unwrap().unwrap();

        for x in vec![1, 1000, 10000, 100000, 1000000] {
            let bytes = rand::thread_rng()
                .sample_iter(&Alphanumeric)
                .take(x)
                .collect::<String>()
                .into_bytes();

            // File to be used in benchmark
            let file =
                DefaultFileService::create(backend, &Uuid::new_v4().to_string(), root.id, Document)
                    .unwrap();

            group.throughput(Throughput::Bytes(bytes.len() as u64));
            group.bench_with_input(
                BenchmarkId::new(format!("{}-Write", backend_string), bytes.len()),
                &bytes,
                |b, _| {
                    b.iter(|| {
                        let _ =
                            DefaultFileService::write_document(backend, file.id, &bytes.clone())
                                .unwrap();
                    });
                },
            );

            group.bench_with_input(
                BenchmarkId::new(format!("{}-Read", backend_string), bytes.len()),
                &bytes,
                |b, _| {
                    DefaultFileService::write_document(backend, file.id, &bytes.clone()).unwrap();
                    b.iter(|| {
                        let _ = DefaultFileService::read_document(backend, file.id).unwrap();
                    });
                },
            );
        }
    }

    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default();
    targets = bench_throughput
}
criterion_main!(benches);
