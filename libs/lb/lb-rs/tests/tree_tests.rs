use lb_rs::model::account::Account;
use lb_rs::model::errors::LbResult;
use lb_rs::model::file_like::FileLike;
use lb_rs::model::file_metadata::FileMetadata;
use lb_rs::model::staged::StagedTreeLikeMut;
use lb_rs::model::tree_like::{TreeLike, TreeLikeMut};
use test_utils::*;
use uuid::Uuid;

#[tokio::test]
async fn tree_test() -> LbResult<()> {
    let account = &Account::new(random_name(), url());
    let file1 = FileMetadata::create_root(account)?;
    let file2 = FileMetadata::create_root(account)?;
    let file3 = FileMetadata::create_root(account)?;

    let mut files = vec![file1.clone(), file2.clone(), file3.clone()];

    files.find(&file1.id)?;
    files.find(&file3.id)?;
    files.find(&file3.id)?;

    assert!(files.maybe_find(&Uuid::new_v4()).is_none());

    assert_eq!(files.ids().len(), 3);

    TreeLikeMut::remove(&mut files, file2.id).unwrap();

    assert_eq!(files.ids().len(), 2);
    files.find(&file2.id).unwrap_err();
    assert!(files.maybe_find(&file2.id).is_none());

    Ok(())
}

#[tokio::test]
async fn test_stage_insert_reset() -> LbResult<()> {
    let account = &Account::new(random_name(), url());
    let file1 = FileMetadata::create_root(account)?;
    let mut file2 = FileMetadata::create_root(account)?;
    let file3 = FileMetadata::create_root(account)?;

    let files = vec![file1, file2.clone(), file3];

    let id = Uuid::new_v4();
    file2.parent = id;
    let mut files = files.stage(Some(file2.clone()));

    assert_eq!(files.find(file2.id())?.parent(), &id);
    assert_eq!(files.base.find(file2.id())?.parent(), file2.id());
    assert_eq!(files.ids().len(), 3);

    // Now reset the file

    file2.parent = file2.id;
    files.insert(file2.clone()).unwrap();
    assert_eq!(files.find(file2.id())?.parent(), file2.id());
    assert_eq!(files.base.find(file2.id())?.parent(), file2.id());
    assert!(files.staged.maybe_find(file2.id()).is_none());
    assert_eq!(files.ids().len(), 3);

    Ok(())
}

#[tokio::test]
async fn test_stage_reset() -> LbResult<()> {
    let account = &Account::new(random_name(), url());
    let file1 = FileMetadata::create_root(account)?;
    let file2 = FileMetadata::create_root(account)?;
    let file3 = FileMetadata::create_root(account)?;

    let files = vec![file1, file2.clone(), file3];

    let files = files.stage(Some(file2.clone())).pruned().unwrap();

    assert_eq!(files.find(file2.id())?.parent(), file2.id());
    assert_eq!(files.base.find(file2.id())?.parent(), file2.id());
    assert!(files.staged.maybe_find(file2.id()).is_none());

    assert_eq!(files.ids().len(), 3);

    Ok(())
}
