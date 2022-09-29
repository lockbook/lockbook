#[cfg(feature = "no-network")]
pub mod exhaustive_sync;

#[cfg(feature = "no-network")]
#[cfg(test)]
pub mod sync_fuzzer2 {
    use crate::exhaustive_sync::experiment::Experiment;

    #[test]
    #[ignore]
    /// Run with: (export "API_URL=http://localhost:8000" && cargo test --release exhaustive_test_sync -- --nocapture --ignored)
    fn exhaustive_test_sync() {
        Experiment::default().kick_off();
    }
}
