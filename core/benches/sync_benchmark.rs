use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use uuid::Uuid;
use lockbook_core::service::test_utils::{create_test_account, GEN_FILES_BENCH_SIZE_1, GEN_FILES_BENCH_SIZE_2, GEN_FILES_BENCH_SIZE_3, GEN_FILES_BENCH_SIZE_4, GEN_FILES_BENCH_SIZE_5, GEN_FILES_BENCH_SIZE_6};
use lockbook_models::file_metadata::FileType;

fn sync_benchmark(c: &mut Criterion) {
    let mut list_metadatas_group = c.benchmark_group("open_app_list_files");
    for size in [GEN_FILES_BENCH_SIZE_1, GEN_FILES_BENCH_SIZE_2, GEN_FILES_BENCH_SIZE_3, GEN_FILES_BENCH_SIZE_4, GEN_FILES_BENCH_SIZE_5, GEN_FILES_BENCH_SIZE_6CREATE_FILES_SIZE_6].iter() {
        let (db, root) = create_test_account();

        for _ in 0..size {
            lockbook_core::create_file(black_box(&db), black_box(&Uuid::new_v4().to_string()), black_box(root.id), black_box(FileType::Document))
        }

        list_paths_group.bench_function(size.to_string(), |b| b.iter(|| lockbook_core::sync_all(&db, None)));
    }
    list_metadatas_group.finish();
}

criterion_group!(benches, sync_benchmark);
criterion_main!(benches);
