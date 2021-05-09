use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use lockbook_core::model::state::Config;
use lockbook_core::repo::file_metadata_repo::FileMetadataRepo;
use lockbook_core::service::account_service::AccountService;
use lockbook_core::service::file_service::FileService;
use lockbook_core::service::sync_service::SyncService;
use lockbook_core::storage::db_provider::{Backend, DbProvider};
use lockbook_core::{
    DefaultAccountService, DefaultDbProvider, DefaultFileMetadataRepo, DefaultFileService,
    DefaultSyncService,
};
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
    let cfg_sled = &Config {
        writeable_path: format!("/tmp/throughput{}", random_string()),
    };
    let cfg_file = &Config {
        writeable_path: format!("/tmp/throughput{}", random_string()),
    };

    let db = &DefaultDbProvider::connect_to_db(cfg_sled).unwrap();
    let sled = &Backend::Sled(db);
    let file = &Backend::File(cfg_file);

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
        let _ = DefaultSyncService::sync(backend, None).unwrap();
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
