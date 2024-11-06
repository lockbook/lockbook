use lb_rs::logic::usage::bytes_to_human;

#[tokio::test]
async fn bytes_to_human_kb() {
    assert_eq!(bytes_to_human(2000), "2 KB".to_string());
}

#[tokio::test]
async fn bytes_to_human_mb() {
    assert_eq!(bytes_to_human(2000000), "2 MB".to_string());
}

#[tokio::test]
async fn bytes_to_human_gb() {
    assert_eq!(bytes_to_human(2000000000), "2 GB".to_string());
}
