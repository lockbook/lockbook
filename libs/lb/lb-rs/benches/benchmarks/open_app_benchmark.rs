use crate::*;
use criterion::{black_box, criterion_group, Criterion};
use lb_rs::shared::file_metadata::FileType;
use lb_rs::Core;
use test_utils::*;
use uuid::Uuid;

fn open_app_benchmark(c: &mut Criterion) {
    get_state_benchmark(c);
    get_account_benchmark(c);
    list_metadatas_benchmark(c);
    list_paths_benchmark(c);
}

fn get_state_benchmark(c: &mut Criterion) {
    let core = test_core_with_account();

    c.bench_function("open_app_get_state", |b| {
        b.iter(|| Core::init(&core.get_config().unwrap()).unwrap())
    });
}

fn get_account_benchmark(c: &mut Criterion) {
    let core = test_core_with_account();

    c.bench_function("open_app_get_account", |b| {
        b.iter(|| {
            let core2 = Core::init(&core.get_config().unwrap()).unwrap();
            core2.get_account().unwrap();
        })
    });
}

fn list_metadatas_benchmark(c: &mut Criterion) {
    let mut list_metadatas_group = c.benchmark_group("open_app_list_metadatas");
    for size in [
        CREATE_FILES_BENCH_1,
        CREATE_FILES_BENCH_2,
        CREATE_FILES_BENCH_3,
        CREATE_FILES_BENCH_4,
        CREATE_FILES_BENCH_5,
        CREATE_FILES_BENCH_6,
    ]
    .iter()
    {
        let core1 = test_core_with_account();
        let root = core1.get_root().unwrap();
        for _ in 0..*size {
            core1
                .create_file(
                    black_box(&Uuid::new_v4().to_string()),
                    black_box(root.id),
                    black_box(FileType::Document),
                )
                .unwrap();
        }

        list_metadatas_group.bench_function(size.to_string(), |b| {
            b.iter(|| {
                let core2 = Core::init(&core1.get_config().unwrap()).unwrap();
                core2.list_metadatas().unwrap();
            })
        });
    }
    list_metadatas_group.finish();
}

fn list_paths_benchmark(c: &mut Criterion) {
    let mut list_paths_group = c.benchmark_group("open_app_list_paths");
    for size in [
        CREATE_FILES_BENCH_1,
        CREATE_FILES_BENCH_2,
        CREATE_FILES_BENCH_3,
        CREATE_FILES_BENCH_4,
        CREATE_FILES_BENCH_5,
        CREATE_FILES_BENCH_6,
    ]
    .iter()
    {
        let core1 = test_core_with_account();
        let root = core1.get_root().unwrap();

        for _ in 0..*size {
            core1
                .create_file(
                    black_box(&Uuid::new_v4().to_string()),
                    black_box(root.id),
                    black_box(FileType::Document),
                )
                .unwrap();
        }

        list_paths_group.bench_function(size.to_string(), |b| {
            b.iter(|| {
                let core2 = Core::init(&core1.get_config().unwrap()).unwrap();
                core2.list_paths(None).unwrap();
            })
        });
    }
    list_paths_group.finish();
}

fn benchmark_config() -> Criterion {
    Criterion::default().sample_size(10)
}

criterion_group! {
    name = benches;
    config = benchmark_config();
    targets = open_app_benchmark
}
