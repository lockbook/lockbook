use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use uuid::Uuid;
use lockbook_core::model::state::Config;
use lockbook_core::service::test_utils::{generate_account, test_config};
use lockbook_models::file_metadata::{DecryptedFileMetadata, FileType};

fn create_file(config: &Config, parent: &Uuid, file_type: FileType) -> DecryptedFileMetadata {
    lockbook_core::create_file(config, &Uuid::new_v4().to_string(), *parent, file_type).unwrap()
}



fn criterion_benchmark(c: &mut Criterion) {
    let create_file_min_size: u64 = 10;

    let mut create_file_group = c.benchmark_group("create_file");
    for size in [create_file_min_size, 10 * create_file_min_size, 20 * create_file_min_size, 50 * create_file_min_size, 100 * create_file_min_size].iter() {
        create_file_group.throughput(Throughput::Elements(*size));
        create_file_group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let (db, root) = create_test_account();

            b.iter(|| create_file(black_box(&db), black_box(&root.id), black_box(FileType::Document)));
        });
    }
    create_file_group.finish();

    let mut create_file_group = c.benchmark_group("open_app");
    for size in [create_file_min_size, 10 * create_file_min_size, 20 * create_file_min_size, 50 * create_file_min_size, 100 * create_file_min_size].iter() {
        create_file_group.throughput(Throughput::Elements(*size));
        create_file_group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let (db, root) = create_test_account();

            for _ in 0..size {
                create_file(black_box(&db), black_box(&root.id), black_box(FileType::Document))
            }

            b.iter(|| {
                lockbook_core::get_db_state(&db).unwrap();
                lockbook_core::get_account(&db).unwrap();
                lockbook_core::list_
            });
        });
    }
    create_file_group.finish();
}

fn create_benchmark_group<O, F: FnMut()>(c: &mut Criterion, benchmark_name: &str, before_action: F, action: F) {
    let mut create_file_group = c.benchmark_group(benchmark_name);
    for size in [create_file_min_size, 10 * create_file_min_size, 20 * create_file_min_size, 50 * create_file_min_size, 100 * create_file_min_size].iter() {
        create_file_group.throughput(Throughput::Elements(*size));
        create_file_group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let (db, root) = create_test_account();

            for _ in 0..size {
                create_file(black_box(&db), black_box(&root.id), black_box(FileType::Document))
            }

            b.iter(|| {
                lockbook_core::get_db_state(&db).unwrap();
                lockbook_core::get_account(&db).unwrap();
                lockbook_core::list_
            });
        });
    }
    create_file_group.finish();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
