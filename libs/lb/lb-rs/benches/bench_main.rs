use criterion::criterion_main;
use lb_rs::blocking::Lb;
use test_utils::{random_name, test_config, url};

mod benchmarks;

pub const MAX_FILES_PER_BENCH: u64 = 6;

pub const CREATE_FILES_BENCH_1: u64 = 1;
pub const CREATE_FILES_BENCH_2: u64 = 10;
pub const CREATE_FILES_BENCH_3: u64 = 100;
pub const CREATE_FILES_BENCH_4: u64 = 500;
pub const CREATE_FILES_BENCH_5: u64 = 1000;
pub const CREATE_FILES_BENCH_6: u64 = 2000;

criterion_main! {
    benchmarks::create_file_benchmark::benches,
    benchmarks::open_app_benchmark::benches,
    benchmarks::sync_benchmark::benches,
    benchmarks::test_repo_integrity_benchmark::benches,
    benchmarks::write_file_benchmark::benches,
}

pub fn blocking_core() -> Lb {
    let lb = Lb::init(test_config()).unwrap();
    lb.create_account(&random_name(), &url(), false).unwrap();
    lb
}
