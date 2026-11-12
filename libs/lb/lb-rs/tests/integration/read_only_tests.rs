use std::fs;
use std::path::Path;

use lb_rs::Lb;
use lb_rs::model::core_config::Config;
use test_utils::http::test_core_with_account;

#[tokio::test]
async fn read_only_instance_reads_pins_without_writing() {
    if test_utils::without_server() {
        return;
    }
    let writer = test_core_with_account().await;
    let doc = writer.create_at_path("pinned.md").await.unwrap();
    writer.pin_file(doc.id).await.unwrap();
    let database = Path::new(&writer.config.writeable_path).join("CoreV5");
    let before = files(&database);

    let reader = Lb::init(Config { read_only: true, ..writer.config.clone() })
        .await
        .unwrap();

    assert_eq!(reader.list_pinned().await.unwrap(), vec![doc.id]);
    assert_eq!(reader.get_file_by_id(doc.id).await.unwrap().name, "pinned.md");
    assert_eq!(files(&database), before);
}

#[tokio::test]
async fn read_only_instance_needs_an_existing_database() {
    let config = Config { read_only: true, ..test_utils::test_config() };

    assert!(Lb::init(config.clone()).await.is_err());
    assert!(!Path::new(&config.writeable_path).join("CoreV5").exists());
}

fn files(directory: &Path) -> Vec<(String, u64)> {
    let mut files: Vec<_> = fs::read_dir(directory)
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            (entry.file_name().into_string().unwrap(), entry.metadata().unwrap().len())
        })
        .collect();
    files.sort();
    files
}
