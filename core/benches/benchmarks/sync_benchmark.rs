use criterion::{black_box, criterion_group, BenchmarkId, Criterion, Throughput};
use lockbook_core::service::test_utils::{
    create_account, test_config, GEN_FILES_BENCH_SIZE_1, GEN_FILES_BENCH_SIZE_2,
    GEN_FILES_BENCH_SIZE_3, GEN_FILES_BENCH_SIZE_4, GEN_FILES_BENCH_SIZE_5, GEN_FILES_BENCH_SIZE_6,
};
use lockbook_models::file_metadata::FileType;
use uuid::Uuid;

const BYTES_IN_EACH_FILE: u64 = 1000;

fn sync_benchmark(c: &mut Criterion) {
    let mut sync_group = c.benchmark_group("sync");
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
        sync_group.throughput(Throughput::Elements(*size));
        sync_group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.iter(|| {
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

                lockbook_core::sync_all(black_box(&db), black_box(None)).unwrap()
            });
        });
    }
    sync_group.finish();
}

criterion_group!(benches, sync_benchmark);
