use lb_rs::{model::core_config::Config, Lb};

#[tokio::test]
// #[ignore]
async fn pending_shares_perf() {
    let lb = Lb::init(Config::cli_config()).await.unwrap();
    for _ in 0..1000 {
        lb.get_pending_shares().await.unwrap();
    }
}
