use crate::*;
use criterion::{black_box, criterion_group, BenchmarkId, Criterion, Throughput};
use lockbook_models::file_metadata::FileType;
use test_utils::*;
use uuid::Uuid;

fn create_file_benchmark(c: &mut Criterion) {
    let mut create_file_group = c.benchmark_group("create_file");
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
        let core = test_core_with_account();
        let root = core.get_root().unwrap();

        create_file_group.throughput(Throughput::Elements(*size));
        create_file_group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            b.iter(|| {
                for _ in 0..*size {
                    core.create_file(
                        black_box(&Uuid::new_v4().to_string()),
                        black_box(root.id),
                        black_box(FileType::Document),
                    )
                    .unwrap();
                }
            });
        });
    }
    create_file_group.finish();
}

fn benchmark_config() -> Criterion {
    Criterion::default().sample_size(10)
}

criterion_group! {
    name = benches;
    config = benchmark_config();
    targets = create_file_benchmark
}
