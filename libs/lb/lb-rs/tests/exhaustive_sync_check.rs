#[cfg(feature = "no-network")]
pub mod exhaustive_sync;

#[cfg(feature = "no-network")]
#[cfg(test)]
pub mod sync_fuzzer2 {
    use crate::exhaustive_sync::coordinator::Coordinator;

    #[ignore]
    #[tokio::test]
    async fn exhaustive_test_sync() {
        Coordinator::default().kick_off();
    }
}
