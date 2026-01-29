use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group};
use lb_rs::model::file_metadata::FileType;
use uuid::Uuid;

use crate::blocking_core;

const BYTES_LEN_1: u64 = 100;
const BYTES_LEN_2: u64 = BYTES_LEN_1 * 10;
const BYTES_LEN_3: u64 = BYTES_LEN_1 * 20;
const BYTES_LEN_4: u64 = BYTES_LEN_1 * 50;
const BYTES_LEN_5: u64 = BYTES_LEN_1 * 100;
const BYTES_LEN_6: u64 = BYTES_LEN_1 * 1000;

fn write_file_benchmark(c: &mut Criterion) {
    let mut write_file_group = c.benchmark_group("write_file");
    for size in
        [BYTES_LEN_1, BYTES_LEN_2, BYTES_LEN_3, BYTES_LEN_4, BYTES_LEN_5, BYTES_LEN_6].iter()
    {
        let core = blocking_core();
        let root = core.get_root().unwrap();

        write_file_group.throughput(Throughput::Elements(*size));
        write_file_group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            b.iter(|| {
                let id = core
                    .create_file(
                        black_box(&Uuid::new_v4().to_string()),
                        black_box(&root.id),
                        black_box(FileType::Document),
                    )
                    .unwrap()
                    .id;

                let random_bytes: Vec<u8> = (0..*size).map(|_| rand::random::<u8>()).collect();

                core.write_document(black_box(id), black_box(random_bytes.as_slice()))
                    .unwrap();
            });
        });
    }
    write_file_group.finish();
}

fn benchmark_config() -> Criterion {
    Criterion::default().sample_size(10)
}

criterion_group! {
    name = benches;
    config = benchmark_config();
    targets = write_file_benchmark
}
