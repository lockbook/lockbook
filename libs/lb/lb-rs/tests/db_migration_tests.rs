use db_rs::View;
use db_rs_old::{Config as OldConfig, Db};
use lb_rs::Lb;
use lb_rs::io::{self, legacy::CoreV4};
use lb_rs::model::account::Account;
use lb_rs::model::file_like::FileLike;
use lb_rs::model::file_metadata::Owner;
use lb_rs::model::meta::Meta;
use lb_rs::service::activity::DocEvent;
use lb_rs::service::lb_id::LbID;
use std::fs;

#[test]
fn copies_every_core_table_and_reopens_without_reimporting() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path();
    let account = Account::new("migration".into(), "http://localhost".into());
    let meta = Meta::create_root(&account)
        .unwrap()
        .sign_with(&account)
        .unwrap();
    let root = *meta.id();
    let owner = Owner(account.public_key());
    let id = LbID::generate();
    let events = [DocEvent::Read(root, 10), DocEvent::Write(root, 20)];
    {
        let mut old = CoreV4::init(OldConfig::in_folder(path)).unwrap();
        let tx = old.begin_transaction().unwrap();
        old.account.insert(account.clone()).unwrap();
        old.last_synced.insert(42).unwrap();
        old.root.insert(root).unwrap();
        old.local_metadata.insert(root, meta.clone()).unwrap();
        old.base_metadata.insert(root, meta.clone()).unwrap();
        old.pub_key_lookup
            .insert(owner, account.username.clone())
            .unwrap();
        for event in events {
            old.doc_events.push(event).unwrap();
        }
        old.id.insert(id).unwrap();
        old.pinned_files.push(root).unwrap();
        old.pinned_files.push(root).unwrap();
        old.last_extracted_panic.insert(99).unwrap();
        tx.drop_safely().unwrap();
    }
    {
        let mut new = io::init_with_migration(path).unwrap();
        assert!(!path.join("CoreV4.db").exists());
        assert_eq!(new.schema.account.as_ref(), Some(&account));
        assert_eq!(new.schema.last_synced.as_ref(), Some(&42));
        assert_eq!(new.schema.root.as_ref(), Some(&root));
        assert_eq!(
            bincode::serialize(new.schema.local_metadata.get(&root).unwrap()).unwrap(),
            bincode::serialize(&meta).unwrap()
        );
        assert_eq!(
            bincode::serialize(new.schema.base_metadata.get(&root).unwrap()).unwrap(),
            bincode::serialize(&meta).unwrap()
        );
        assert_eq!(new.schema.pub_key_lookup.get(&owner), Some(&account.username));
        assert_eq!(new.schema.doc_events.as_slice(), events);
        assert_eq!(new.schema.id.as_ref(), Some(&id));
        assert_eq!(new.schema.pinned_files.as_slice(), [root, root]);
        assert_eq!(new.schema.last_extracted_panic.as_ref(), Some(&99));
        let tx = new.write_tx().unwrap();
        new.schema.pinned_files.clear().unwrap();
        new.schema.last_synced.replace(43).unwrap();
        tx.end_tx(&mut new).unwrap();
        new.snapshot().unwrap();
    }
    let reopened = io::init_with_migration(path).unwrap();
    assert_eq!(reopened.schema.last_synced.as_ref(), Some(&43));
    assert!(reopened.schema.pinned_files.is_empty());
}

#[test]
fn migrates_when_new_database_has_no_account() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path();
    let empty = io::init_with_migration(path).unwrap();
    assert!(empty.schema.account.is_none());
    drop(empty);
    let account = Account::new("migration".into(), "http://localhost".into());
    {
        let mut old = CoreV4::init(OldConfig::in_folder(path)).unwrap();
        old.account.insert(account.clone()).unwrap();
    }

    let db = io::init_with_migration(path).unwrap();
    assert_eq!(db.schema.account.as_ref(), Some(&account));
    assert!(!path.join("CoreV4.db").exists());
}

#[test]
fn migrates_extensionless_log() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path();
    let account = Account::new("migration".into(), "http://localhost".into());
    {
        let mut old = CoreV4::init(OldConfig::in_folder(path)).unwrap();
        old.account.insert(account.clone()).unwrap();
    }
    let bytes = fs::read(path.join("CoreV4.db")).unwrap();
    // The extensionless format has no two-byte metadata header.
    fs::write(path.join("CoreV4"), &bytes[2..]).unwrap();
    fs::remove_file(path.join("CoreV4.db")).unwrap();

    let db = io::init_with_migration(path).unwrap();
    assert_eq!(db.schema.account.as_ref(), Some(&account));
    assert!(!path.join("CoreV4").exists());
    assert!(!path.join("CoreV4.db").exists());
}

#[test]
fn skips_migration_when_new_database_has_an_account() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path();
    let account = Account::new("current".into(), "http://localhost".into());
    {
        let mut db = io::init_with_migration(path).unwrap();
        let tx = db.write_tx().unwrap();
        db.schema.account.replace(account.clone()).unwrap();
        tx.end_tx(&mut db).unwrap();
    }
    {
        let mut old = CoreV4::init(OldConfig::in_folder(path)).unwrap();
        old.last_synced.insert(42).unwrap();
    }
    let original = fs::read(path.join("CoreV4.db")).unwrap();

    let db = io::init_with_migration(path).unwrap();
    assert_eq!(db.schema.account.as_ref(), Some(&account));
    assert!(db.schema.last_synced.is_none());
    assert_eq!(fs::read(path.join("CoreV4.db")).unwrap(), original);
}

#[tokio::test]
async fn lb_transaction_flushes_on_drop() {
    let config = test_utils::test_config();
    {
        let lb = Lb::init(config.clone()).await.unwrap();
        let mut tx = lb.begin_tx().await;
        tx.db().last_synced.replace(123).unwrap();
    }
    let lb = Lb::init(config).await.unwrap();
    assert_eq!(lb.ro_tx().await.db().last_synced.as_ref(), Some(&123));
}
