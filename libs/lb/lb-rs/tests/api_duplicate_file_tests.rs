use lb_rs::model::file_metadata::FileType;
use test_utils::*;

#[tokio::test]
async fn duplicate_files_copies_document_content_and_folder_descendants() {
    let core = test_core_with_account().await;
    let root = core.root().await.unwrap();
    let document = core
        .create_file("note.md", &root.id, FileType::Document)
        .await
        .unwrap();
    core.write_document(document.id, b"contents").await.unwrap();
    let folder = core
        .create_file("folder", &root.id, FileType::Folder)
        .await
        .unwrap();
    let nested = core
        .create_file("nested.md", &folder.id, FileType::Document)
        .await
        .unwrap();
    core.write_document(nested.id, b"nested contents")
        .await
        .unwrap();

    let duplicates = core
        .duplicate_files(vec![document.id, folder.id], root.id)
        .await
        .unwrap();

    assert_eq!(duplicates[0].name, "note-1.md");
    assert_eq!(core.read_document(duplicates[0].id, false).await.unwrap(), b"contents");
    assert_eq!(duplicates[1].name, "folder-1");
    let duplicated_nested = core
        .get_children(&duplicates[1].id)
        .await
        .unwrap()
        .pop()
        .unwrap();
    assert_eq!(duplicated_nested.name, "nested.md");
    assert_eq!(
        core.read_document(duplicated_nested.id, false)
            .await
            .unwrap(),
        b"nested contents"
    );
}
