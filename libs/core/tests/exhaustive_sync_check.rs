#[cfg(feature = "no-network")]
pub mod exhaustive_sync;

#[cfg(feature = "no-network")]
#[cfg(test)]
pub mod sync_fuzzer2 {
    use crate::exhaustive_sync::experiment::Experiment;

    #[test]
    fn exhaustive_test_sync() {
        Experiment::default().kick_off();
    }
}
