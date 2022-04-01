use criterion::{black_box, criterion_group, BenchmarkId, Criterion, Throughput};
use lockbook_core::service::integrity_service;
use lockbook_core::service::test_utils::{
    create_account, test_config, GEN_FILES_BENCH_SIZE_1, GEN_FILES_BENCH_SIZE_2,
    GEN_FILES_BENCH_SIZE_3, GEN_FILES_BENCH_SIZE_4, GEN_FILES_BENCH_SIZE_5, GEN_FILES_BENCH_SIZE_6,
};
use lockbook_models::file_metadata::FileType;
use uuid::Uuid;

const BYTES_IN_EACH_FILE: u64 = 1000;

fn test_repo_integrity_benchmark(c: &mut Criterion) {
    let mut test_repo_integrity_group = c.benchmark_group("test_repo_integrity");
    for size in [
        GEN_FILES_BENCH_SIZE_1,
        GEN_FILES_BENCH_SIZE_2,
        GEN_FILES_BENCH_SIZE_3,
        GEN_FILES_BENCH_SIZE_4,
        GEN_FILES_BENCH_SIZE_5,
        GEN_FILES_BENCH_SIZE_6,
    ]
    .iter()
    {
        test_repo_integrity_group.throughput(Throughput::Elements(*size));
        test_repo_integrity_group.bench_with_input(
            BenchmarkId::from_parameter(size),
            size,
            |b, &size| {
                let db = test_config();
                let (_, root) = create_account(&db);

                for _ in 0..size {
                    let id = lockbook_core::create_file(
                        black_box(&db),
                        black_box(&Uuid::new_v4().to_string()),
                        black_box(root.id),
                        black_box(FileType::Document),
                    )
                    .unwrap()
                    .id;
                    let random_bytes: Vec<u8> = (0..BYTES_IN_EACH_FILE)
                        .map(|_| rand::random::<u8>())
                        .collect();
                    lockbook_core::write_document(black_box(&db), id, random_bytes.as_slice())
                        .unwrap();
                }

                b.iter(|| {
                    integrity_service::test_repo_integrity(black_box(&db)).unwrap();
                });
            },
        );
    }
    test_repo_integrity_group.finish();
}

criterion_group!(benches, test_repo_integrity_benchmark);
