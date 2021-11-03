pub mod exhaustive_sync;

#[cfg(test)]
pub mod sync_fuzzer2 {

    use crate::exhaustive_sync::experiment::Experiment;

    #[test]
    /// Run with: cargo test --release exhaustive_test_sync -- --nocapture --ignored
    fn exhaustive_test_sync() {
        Experiment::default().kick_off();
    }
}
