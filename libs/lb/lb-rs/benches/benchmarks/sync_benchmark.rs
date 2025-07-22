use crate::*;
use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group};
use lb_rs::model::file_metadata::FileType;
use uuid::Uuid;

const BYTES_IN_EACH_FILE: u64 = 1000;

fn sync_benchmark(c: &mut Criterion) {
    let mut sync_group = c.benchmark_group("sync");
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
        sync_group.throughput(Throughput::Elements(*size));
        sync_group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.iter(|| {
                let core = blocking_core();
                let root = core.get_root().unwrap();

                for _ in 0..size {
                    let id = core
                        .create_file(
                            black_box(&Uuid::new_v4().to_string()),
                            black_box(&root.id),
                            black_box(FileType::Document),
                        )
                        .unwrap()
                        .id;
                    let random_bytes: Vec<u8> = (0..BYTES_IN_EACH_FILE)
                        .map(|_| rand::random::<u8>())
                        .collect();
                    core.write_document(id, random_bytes.as_slice()).unwrap();
                }

                core.sync(black_box(None)).unwrap()
            });
        });
    }
    sync_group.finish();
}

fn benchmark_config() -> Criterion {
    Criterion::default().sample_size(10)
}

criterion_group! {
    name = benches;
    config = benchmark_config();
    targets = sync_benchmark
}
