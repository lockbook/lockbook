use lockbook_core::model::repo::RepoSource;
use lockbook_core::repo::document_repo;
use lockbook_models::file_metadata::FileType;
use lockbook_models::file_metadata::FileType::Folder;
use test_utils::*;

#[test]
fn report_usage() {
    let core = test_core_with_account();
    let root = core.get_root().unwrap();

    let file = core
        .create_file(&random_name(), root.id, FileType::Document)
        .unwrap();
    core.write_document(file.id, "0000000000".as_bytes())
        .unwrap();

    assert!(core.get_usage().unwrap().usages.is_empty(), "Returned non-empty usage!");

    core.sync(None).unwrap();

    let local_encrypted = document_repo::get(&core.config, RepoSource::Base, file.id)
        .unwrap()
        .value;

    assert_eq!(core.get_usage().unwrap().usages[0].file_id, file.id);
    assert_eq!(core.get_usage().unwrap().usages.len(), 1);
    assert_eq!(core.get_usage().unwrap().usages[0].size_bytes, local_encrypted.len() as u64)
}

#[test]
fn usage_go_back_down_after_delete() {
    let core = test_core_with_account();
    let root = core.get_root().unwrap();

    let file = core
        .create_file(&random_name(), root.id, FileType::Document)
        .unwrap();
    core.write_document(file.id, &String::from("0000000000").into_bytes())
        .unwrap();

    core.sync(None).unwrap();
    core.delete_file(file.id).unwrap();
    core.sync(None).unwrap();

    assert!(core.get_usage().unwrap().usages.is_empty());
}

#[test]
fn usage_go_back_down_after_delete_folder() {
    let core = test_core_with_account();
    let root = core.get_root().unwrap();

    let folder = core.create_file("folder", root.id, Folder).unwrap();
    let file = core
        .create_file(&random_name(), root.id, FileType::Document)
        .unwrap();
    core.write_document(file.id, &String::from("0000000000").into_bytes())
        .unwrap();
    let file2 = core
        .create_file(&random_name(), folder.id, FileType::Document)
        .unwrap();
    core.write_document(file2.id, &String::from("0000000000").into_bytes())
        .unwrap();
    let file3 = core
        .create_file(&random_name(), folder.id, FileType::Document)
        .unwrap();
    core.write_document(file3.id, &String::from("0000000000").into_bytes())
        .unwrap();

    core.sync(None).unwrap();
    let usages = core.get_usage().unwrap();
    assert_eq!(usages.usages.len(), 3);
    core.delete_file(folder.id).unwrap();
    for usage in usages.usages {
        assert_ne!(usage.size_bytes, 0);
    }
    core.sync(None).unwrap();

    document_repo::get(&core.config, RepoSource::Base, file.id).unwrap();

    let usage = core.get_usage().unwrap_or_else(|err| panic!("{:?}", err));

    assert_eq!(usage.usages.len(), 1);
}

#[test]
fn usage_new_files_have_no_size() {
    let core = test_core_with_account();
    let root = core.get_root().unwrap();
    core.create_file(&random_name(), root.id, FileType::Document)
        .unwrap();

    assert!(core.get_usage().unwrap().usages.is_empty(), "Returned non-empty usage!");

    core.sync(None).unwrap();

    let total_usage = core
        .get_usage()
        .unwrap()
        .usages
        .iter()
        .filter(|usage| usage.file_id != root.id)
        .map(|usage| usage.size_bytes)
        .sum::<u64>();

    assert_eq!(total_usage, 32, "Returned a file size that is not the default 32 bytes!");
}
