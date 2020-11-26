# Performance Monitoring

Our performance monitoring stack creates nice SVGs based on run scenarios defined in `core/benches`
The following tools are used:

[Criterion.rs](https://bheisler.github.io/criterion.rs/book/user_guide/profiling.html) we use Criterion, a rust package, to make bench-markable sections of code and benchmark-style tests.

[AtheMathmo/cpuprofiler](https://github.com/AtheMathmo/cpuprofiler) to provide binding for rust to explicitly start and stop the cpu profiler. This allows for only profiling critical sections instead of the whole binary.

[gperftools/gperftools](https://github.com/gperftools/gperftools) a version of Google Performance Tools. This is the actual cpu profiler that lets us collect stack information about our program.

[libunwind](https://www.nongnu.org/libunwind/) which allows for introspection of the running process on our architecture.

[google/pprof](https://github.com/google/pprof) finally a fun one, this create our cools diagrams or collapsed stack traces from our stack trace for human consumption.

