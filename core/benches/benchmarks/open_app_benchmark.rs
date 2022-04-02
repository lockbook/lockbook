use criterion::{black_box, criterion_group, Criterion};
use lockbook_core::service::test_utils::{create_account, test_config, MAX_FILES_PER_BENCH};
use lockbook_models::file_metadata::FileType;
use uuid::Uuid;

fn open_app_benchmark(c: &mut Criterion) {
    get_state_benchmark(c);
    get_account_benchmark(c);
    list_metadatas_benchmark(c);
    list_paths_benchmark(c);
}

fn get_state_benchmark(c: &mut Criterion) {
    let db = test_config();
    create_account(&db);

    c.bench_function("open_app_get_state", |b| {
        b.iter(|| lockbook_core::get_db_state(&db).unwrap())
    });
}

fn get_account_benchmark(c: &mut Criterion) {
    let db = test_config();
    create_account(&db);

    c.bench_function("open_app_get_account", |b| {
        b.iter(|| lockbook_core::get_account(black_box(&db)))
    });
}

fn list_metadatas_benchmark(c: &mut Criterion) {
    let mut list_metadatas_group = c.benchmark_group("open_app_list_metadatas");
    for size in 1..=MAX_FILES_PER_BENCH {
        let db = test_config();
        let (_, root) = create_account(&db);

        for _ in 0..size {
            lockbook_core::create_file(
                black_box(&db),
                black_box(&Uuid::new_v4().to_string()),
                black_box(root.id),
                black_box(FileType::Document),
            )
            .unwrap();
        }

        list_metadatas_group.bench_function(size.to_string(), |b| {
            b.iter(|| lockbook_core::list_metadatas(&db).unwrap())
        });
    }
    list_metadatas_group.finish();
}

fn list_paths_benchmark(c: &mut Criterion) {
    let mut list_paths_group = c.benchmark_group("open_app_list_paths");
    for size in 1..=MAX_FILES_PER_BENCH {
        let db = test_config();
        let (_, root) = create_account(&db);

        for _ in 0..size {
            lockbook_core::create_file(
                black_box(&db),
                black_box(&Uuid::new_v4().to_string()),
                black_box(root.id),
                black_box(FileType::Document),
            )
            .unwrap();
        }

        list_paths_group.bench_function(size.to_string(), |b| {
            b.iter(|| lockbook_core::list_paths(black_box(&db), black_box(None)).unwrap())
        });
    }
    list_paths_group.finish();
}

fn benchmark_config() -> Criterion {
    Criterion::default().sample_size(20)
}

criterion_group! {
    name = benches;
    config = benchmark_config();
    targets = open_app_benchmark
}
