use lb_rs::Lb;
use lb_rs::model::core_config::Config;

#[tokio::test]
#[ignore]
async fn pending_shares_perf() {
    let lb = Lb::init(Config::cli_config("cli")).await.unwrap();
    for _ in 0..1000 {
        lb.get_pending_shares().await.unwrap();
    }
}

#[tokio::test]
#[ignore]
async fn debug_info_test() {
    let lb = Lb::init(Config::cli_config("cli")).await.unwrap();
    for _ in 0..2 {
        lb.debug_info("none".to_string()).await.unwrap();
    }
}
