use lockbook_shared::account::Account;
use lockbook_shared::file_like::FileLike;
use lockbook_shared::file_metadata::FileMetadata;
use lockbook_shared::tree_like::Stagable;
use lockbook_shared::SharedResult;
use test_utils::*;

#[test]
fn test_create_path() {
    let account = &Account::new(random_name(), url());
    let pk = &account.public_key();
    let root = FileMetadata::create_root(account)
        .unwrap()
        .sign(account)
        .unwrap();

    let mut tree = vec![root.clone()]
        .to_lazy()
        .stage(vec![])
        .create_at_path("test", root.id(), account, pk)
        .unwrap()
        .0
        .create_at_path("test2", root.id(), account, pk)
        .unwrap()
        .0
        .create_at_path("test3", root.id(), account, pk)
        .unwrap()
        .0;

    let paths = tree.list_paths(None, account).unwrap();

    println!("{:?}", paths);
}
