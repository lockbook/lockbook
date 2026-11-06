// Migrates an on-disk database.
#![cfg(not(feature = "in-process-tests"))]

use db_rs::View;
use db_rs_old::{Config as OldConfig, Db};
use lb_rs::Lb;
use lb_rs::io::{self, legacy::CoreV4};
use lb_rs::model::account::Account;
use lb_rs::model::file_like::FileLike;
use lb_rs::model::file_metadata::{FileType, Owner};
use lb_rs::model::meta::Meta;
use lb_rs::model::symkey;
use lb_rs::service::activity::DocEvent;
use lb_rs::service::lb_id::LbID;
use std::fs;
use uuid::Uuid;

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

#[test]
fn moves_document_versions_out_of_v1_metadata() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path();
    let account = Account::new("migration".into(), "http://localhost".into());
    let owner = account.public_key();
    let root_key = symkey::generate_key();
    let root = Meta::create_root(&account).unwrap();
    let root_id = *root.id();
    let document = |name: &str| {
        Meta::create(
            Uuid::new_v4(),
            symkey::generate_key(),
            &owner,
            root_id,
            &root_key,
            name,
            FileType::Document,
        )
        .unwrap()
    };
    let edited = document("edited.md");
    let deleted = document("deleted.md");
    let untouched = document("untouched.md");
    let v1 = |meta: &Meta, hmac: u8, size: usize| {
        meta.clone()
            .into_v1(Some([hmac; 32]), Some(size))
            .sign_as_is(&account)
            .unwrap()
    };
    let mut deleted_locally = deleted.clone();
    deleted_locally.set_deleted(true);
    {
        let mut db = io::init_with_migration(path).unwrap();
        let tx = db.write_tx().unwrap();
        let schema = &mut db.schema;
        schema.account.replace(account.clone()).unwrap();
        schema.root.replace(root_id).unwrap();
        let root = root
            .clone()
            .into_v1(None, None)
            .sign_as_is(&account)
            .unwrap();
        schema.base_metadata.insert(root_id, root).unwrap();
        schema
            .base_metadata
            .insert(*edited.id(), v1(&edited, 1, 10))
            .unwrap();
        schema
            .local_metadata
            .insert(*edited.id(), v1(&edited, 2, 12))
            .unwrap();
        schema
            .base_metadata
            .insert(*deleted.id(), v1(&deleted, 3, 30))
            .unwrap();
        schema
            .local_metadata
            .insert(*deleted.id(), v1(&deleted_locally, 4, 40))
            .unwrap();
        schema
            .base_metadata
            .insert(*untouched.id(), v1(&untouched, 5, 50))
            .unwrap();
        tx.end_tx(&mut db).unwrap();
    }

    let db = io::init_with_migration(path).unwrap();
    let info = |meta: &Meta| *db.schema.doc_info.get(meta.id()).unwrap();

    let edited_info = info(&edited);
    assert_eq!(edited_info.base_hmac(), Some([1; 32]));
    assert_eq!(edited_info.remote_hmac(), Some([1; 32]));
    assert_eq!(edited_info.base.unwrap().stored_size, 10);
    assert_eq!(edited_info.local.unwrap().hmac, [2; 32]);
    assert_eq!(edited_info.local.unwrap().compressed_size, 12);
    assert!(db.schema.local_metadata.get(edited.id()).is_none());

    let deleted_info = info(&deleted);
    assert_eq!(deleted_info.base_hmac(), Some([3; 32]));
    assert_eq!(deleted_info.local.unwrap().hmac, [4; 32]);
    let deleted_local = db.schema.local_metadata.get(deleted.id()).unwrap();
    assert!(deleted_local.timestamped_value.value.is_v3());
    assert!(deleted_local.explicitly_deleted());

    let untouched_info = info(&untouched);
    assert_eq!(untouched_info.current_hmac(), Some([5; 32]));
    assert!(untouched_info.local.is_none());
    assert!(db.schema.doc_info.get(&root_id).is_none());
    let migrated = db.schema.doc_info.iter().count();
    drop(db);

    let reopened = io::init_with_migration(path).unwrap();
    assert_eq!(reopened.schema.doc_info.iter().count(), migrated);
    assert_eq!(*reopened.schema.doc_info.get(edited.id()).unwrap(), edited_info);
}
