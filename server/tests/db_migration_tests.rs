use db_rs::View;
use db_rs::config::Config;
use db_rs_old::{Config as OldConfig, Db};
use lb_rs::model::account::Account as CoreAccount;
use lb_rs::model::file_like::FileLike;
use lb_rs::model::file_metadata::Owner;
use lb_rs::model::meta::Meta;
use lb_rs::model::server_meta::IntoServerMeta;
use lb_rs::service::debug::DebugInfo;
use lb_rs::service::lb_id::LbID;
use lockbook_server_lib::defense::BandwidthReport;
use lockbook_server_lib::guard::ServerTx;
use lockbook_server_lib::schema::{self, Account, ServerDb, legacy::ServerV5};
use std::fs;
use uuid::Uuid;

#[test]
fn copies_every_server_table_including_empty_groups() {
    let config = Config::test();
    let path = &config.log_location;
    let account = CoreAccount::new("migration".into(), "http://localhost".into());
    let owner = Owner(account.public_key());
    let empty_owner =
        Owner(CoreAccount::new("empty".into(), "http://localhost".into()).public_key());
    let meta = Meta::create_root(&account)
        .unwrap()
        .sign_with(&account)
        .unwrap()
        .add_time(42);
    let root = *meta.id();
    let empty_root = Uuid::new_v4();
    let id = LbID::generate();
    let info = DebugInfo {
        lb_id: id,
        time: "today".into(),
        name: "migration".into(),
        last_synced: "never".into(),
        lb_version: "test".into(),
        rust_triple: "test".into(),
        os_info: "test".into(),
        lb_dir: "test".into(),
        server_url: "http://localhost".into(),
        integrity: "ok".into(),
        is_syncing: false,
        status: "ok".into(),
        panics: vec!["test panic".into()],
    };
    let mut bandwidth = BandwidthReport::default();
    bandwidth.increase_by(123);
    {
        let mut old = ServerV5::init(OldConfig::in_folder(path)).unwrap();
        let tx = old.begin_transaction().unwrap();
        old.usernames
            .insert(account.username.clone(), owner)
            .unwrap();
        old.metas.insert(root, meta.clone()).unwrap();
        old.google_play_ids.insert("google".into(), owner).unwrap();
        old.stripe_ids.insert("stripe".into(), owner).unwrap();
        old.app_store_ids.insert("apple".into(), owner).unwrap();
        old.last_seen.insert(owner, 99).unwrap();
        old.accounts
            .insert(
                owner,
                Account { username: account.username.clone(), billing_info: Default::default() },
            )
            .unwrap();
        old.owned_files.insert(owner, root).unwrap();
        old.owned_files.create_key(empty_owner).unwrap();
        old.shared_files.insert(owner, root).unwrap();
        old.shared_files.create_key(empty_owner).unwrap();
        old.file_children.insert(root, root).unwrap();
        old.file_children.create_key(empty_root).unwrap();
        old.server_egress.insert(bandwidth.clone()).unwrap();
        old.egress_by_owner.insert(owner, bandwidth).unwrap();
        old.scheduled_file_cleanups
            .insert((root, [7; 32]), 456)
            .unwrap();
        old.debug_info.insert(owner, id, info.clone()).unwrap();
        old.debug_info.create_key(empty_owner).unwrap();
        tx.drop_safely().unwrap();
    }
    let original = fs::read(path.join("ServerV5.db")).unwrap();
    {
        let mut db = schema::init_with_migration(path).unwrap();
        assert_eq!(db.schema.usernames.get("migration"), Some(&owner));
        assert_eq!(
            bincode::serialize(db.schema.metas.get(&root).unwrap()).unwrap(),
            bincode::serialize(&meta).unwrap()
        );
        assert_eq!(db.schema.google_play_ids.get("google"), Some(&owner));
        assert_eq!(db.schema.stripe_ids.get("stripe"), Some(&owner));
        assert_eq!(db.schema.app_store_ids.get("apple"), Some(&owner));
        assert_eq!(db.schema.last_seen.get(&owner), Some(&99));
        assert_eq!(db.schema.accounts.get(&owner).unwrap().username, account.username);
        assert!(db.schema.owned_files.get(&owner).unwrap().contains(&root));
        assert!(db.schema.owned_files.get(&empty_owner).unwrap().is_empty());
        assert!(db.schema.shared_files.get(&owner).unwrap().contains(&root));
        assert!(db.schema.shared_files.get(&empty_owner).unwrap().is_empty());
        assert!(db.schema.file_children.get(&root).unwrap().contains(&root));
        assert!(db.schema.file_children.get(&empty_root).unwrap().is_empty());
        assert_eq!(db.schema.server_egress.as_ref().unwrap().all_bandwidth(), 123);
        assert_eq!(
            db.schema
                .egress_by_owner
                .get(&owner)
                .unwrap()
                .all_bandwidth(),
            123
        );
        assert_eq!(db.schema.scheduled_file_cleanups.get(&(root, [7; 32])), Some(&456));
        assert_eq!(db.schema.debug_info.get(&owner).unwrap().get(&id), Some(&info));
        assert!(db.schema.debug_info.get(&empty_owner).unwrap().is_empty());
        let mut tx = ServerTx::begin(&mut db).unwrap();
        tx.usernames.remove(&account.username).unwrap();
        tx.end().unwrap();
    }
    let db = schema::init_with_migration(path).unwrap();
    assert!(db.schema.usernames.is_empty());
    assert_eq!(db.schema.metas.get(&root), Some(&meta));
    assert_eq!(fs::read(path.join("ServerV5.db")).unwrap(), original);
}

#[test]
fn server_transactions_flush_on_drop_and_catch_up() {
    let config = Config::test();
    let mut first = ServerDb::init(&config).unwrap();
    let mut second = ServerDb::init(&config).unwrap();
    let owner = Owner(CoreAccount::new("test".into(), "http://localhost".into()).public_key());
    {
        let mut tx = ServerTx::begin(&mut first).unwrap();
        tx.usernames.insert("first".into(), owner).unwrap();
    }
    let mut tx = ServerTx::begin(&mut second).unwrap();
    assert_eq!(tx.usernames.get("first"), Some(&owner));
    tx.usernames.insert("second".into(), owner).unwrap();
    tx.end().unwrap();

    let reopened = ServerDb::init(&config).unwrap();
    assert_eq!(reopened.schema.usernames.len(), 2);
    assert_eq!(reopened.last_modified(), 2);
}
