use criterion::criterion_main;

mod benchmarks;

criterion_main! {
    benchmarks::create_file_benchmark::benches,
    benchmarks::open_app_benchmark::benches,
    benchmarks::sync_benchmark::benches,
    benchmarks::test_repo_integrity_benchmark::benches,
    benchmarks::write_file_benchmark::benches,
}
