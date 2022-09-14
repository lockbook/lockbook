use crate::{get_dirty_ids, slices_equal_ignore_order, test_core_from};
use hmdb::transaction::Transaction;
use lockbook_core::repo::schema::OneKey;
use lockbook_core::Core;
use lockbook_shared::file_like::FileLike;
use lockbook_shared::file_metadata::FileType;
use lockbook_shared::path_ops::Filter::DocumentsOnly;
use lockbook_shared::tree_like::{Stagable, TreeLike};
use uuid::Uuid;

#[macro_export]
macro_rules! assert_matches (
    ($actual:expr, $expected:pat) => {
        // Only compute actual once
        let actual_value = $actual;
        match actual_value {
            $expected => {},
            _ => panic!("assertion failed: {:?} did not match expectation", actual_value)
        }
    }
);

pub fn cores_equal(left: &Core, right: &Core) {
    assert_eq!(&left.db.account.get_all().unwrap(), &right.db.account.get_all().unwrap());
    assert_eq!(&left.db.root.get_all().unwrap(), &right.db.root.get_all().unwrap());
    assert_eq!(
        &left.db.local_metadata.get_all().unwrap(),
        &right.db.local_metadata.get_all().unwrap()
    );
    assert_eq!(
        &left.db.base_metadata.get_all().unwrap(),
        &right.db.base_metadata.get_all().unwrap()
    );
}

pub fn new_synced_client_core_equal(core: &Core) {
    let new_client = test_core_from(core);
    new_client.validate().unwrap();
    cores_equal(core, &new_client);
}

pub fn all_ids(core: &Core, expected_ids: &[Uuid]) {
    let mut expected_ids: Vec<Uuid> = expected_ids.to_vec();
    let mut actual_ids: Vec<Uuid> = core
        .list_metadatas()
        .unwrap()
        .iter()
        .map(|f| f.id)
        .collect();

    actual_ids.sort();
    expected_ids.sort();
    if actual_ids != expected_ids {
        panic!(
            "ids did not match expectation. expected={:?}; actual={:?}",
            expected_ids, actual_ids
        );
    }
}

pub fn all_children_ids(core: &Core, id: Uuid, expected_ids: &[Uuid]) {
    let mut expected_ids: Vec<Uuid> = expected_ids.to_vec();
    let mut actual_ids: Vec<Uuid> = core
        .get_children(id)
        .unwrap()
        .iter()
        .map(|f| f.id)
        .collect();

    actual_ids.sort();
    expected_ids.sort();
    if actual_ids != expected_ids {
        panic!(
            "ids did not match expectation. expected={:?}; actual={:?}",
            expected_ids, actual_ids
        );
    }
}

pub fn all_recursive_children_ids(core: &Core, id: Uuid, expected_ids: &[Uuid]) {
    let mut expected_ids: Vec<Uuid> = expected_ids.to_vec();
    let mut actual_ids: Vec<Uuid> = core
        .get_and_get_children_recursively(id)
        .unwrap()
        .iter()
        .map(|f| f.id)
        .collect();

    actual_ids.sort();
    expected_ids.sort();
    if actual_ids != expected_ids {
        panic!(
            "ids did not match expectation. expected={:?}; actual={:?}",
            expected_ids, actual_ids
        );
    }
}

pub fn all_paths(core: &Core, expected_paths: &[&str]) {
    let mut expected_paths: Vec<String> = expected_paths
        .iter()
        .map(|&path| String::from(path))
        .collect();
    let mut actual_paths: Vec<String> = core.list_paths(None).unwrap();

    actual_paths.sort();
    expected_paths.sort();
    if actual_paths != expected_paths {
        panic!(
            "paths did not match expectation. expected={:?}; actual={:?}",
            expected_paths, actual_paths
        );
    }
}

pub fn all_document_contents(db: &Core, expected_contents_by_path: &[(&str, &[u8])]) {
    let expected_contents_by_path = expected_contents_by_path
        .iter()
        .map(|&(path, contents)| (path.to_string(), contents.to_vec()))
        .collect::<Vec<(String, Vec<u8>)>>();
    let actual_contents_by_path = db
        .list_paths(Some(DocumentsOnly))
        .unwrap()
        .iter()
        .map(|path| (path.clone(), db.read_document(db.get_by_path(path).unwrap().id).unwrap()))
        .collect::<Vec<(String, Vec<u8>)>>();
    if !slices_equal_ignore_order(&actual_contents_by_path, &expected_contents_by_path) {
        panic!(
            "document contents did not match expectation. expected={:?}; actual={:?}",
            expected_contents_by_path
                .into_iter()
                .map(|(path, contents)| (path, String::from_utf8_lossy(&contents).to_string()))
                .collect::<Vec<(String, String)>>(),
            actual_contents_by_path
                .into_iter()
                .map(|(path, contents)| (path, String::from_utf8_lossy(&contents).to_string()))
                .collect::<Vec<(String, String)>>(),
        );
    }
}

pub fn all_pending_shares(core: &Core, expected_names: &[&str]) {
    if expected_names.iter().any(|&path| path.contains('/')) {
        panic!(
            "improper call to assert_all_pending_shares; expected_names must not contain with '/'. expected_names={:?}",
            expected_names
        );
    }
    let mut expected_names: Vec<String> = expected_names
        .iter()
        .map(|&name| String::from(name))
        .collect();
    let mut actual_names: Vec<String> = core
        .get_pending_shares()
        .unwrap()
        .into_iter()
        .map(|f| f.name)
        .collect();
    actual_names.sort();
    expected_names.sort();
    if actual_names != expected_names {
        panic!(
            "pending share names did not match expectation. expected={:?}; actual={:?}",
            expected_names, actual_names
        );
    }
}

pub fn local_work_paths(db: &Core, expected_paths: &[&'static str]) {
    let dirty = get_dirty_ids(db, false);

    let mut expected_paths = expected_paths.to_vec();
    let mut actual_paths = db
        .db
        .transaction(|tx| {
            let account = tx.account.get(&OneKey {}).unwrap();
            let mut local = tx.base_metadata.stage(&mut tx.local_metadata).to_lazy();
            dirty
                .iter()
                .filter(|id| !matches!(local.find(id).unwrap().file_type(), FileType::Link { .. }))
                .collect::<Vec<_>>()
                .iter()
                .filter(|id| !local.in_pending_share(id).unwrap())
                .collect::<Vec<_>>()
                .iter()
                .map(|id| local.id_to_path(id, account))
                .collect::<Result<Vec<String>, _>>()
                .unwrap()
        })
        .unwrap();
    actual_paths.sort_unstable();
    expected_paths.sort_unstable();
    if actual_paths != expected_paths {
        panic!(
            "local work paths did not match expectation. expected={:?}; actual={:?}",
            expected_paths, actual_paths
        );
    }
}

pub fn server_work_paths(db: &Core, expected_paths: &[&'static str]) {
    let mut expected_paths = expected_paths.to_vec();
    let mut actual_paths = db
        .db
        .transaction(|tx| {
            let context = db.context(tx).unwrap();
            let account = context.tx.account.get(&OneKey {}).unwrap();
            let remote_changes = context.get_updates().unwrap().file_metadata;
            let mut remote = context.tx.base_metadata.stage(remote_changes).to_lazy();
            remote
                .tree
                .staged
                .owned_ids()
                .iter()
                .filter(|id| !matches!(remote.find(id).unwrap().file_type(), FileType::Link { .. }))
                .collect::<Vec<_>>()
                .iter()
                .filter(|id| !remote.in_pending_share(id).unwrap())
                .collect::<Vec<_>>()
                .iter()
                .map(|id| remote.id_to_path(id, account))
                .collect::<Result<Vec<String>, _>>()
                .unwrap()
        })
        .unwrap();
    actual_paths.sort_unstable();
    expected_paths.sort_unstable();
    if actual_paths != expected_paths {
        panic!(
            "server work paths did not match expectation. expected={:?}; actual={:?}",
            expected_paths, actual_paths
        );
    }
}

pub fn deleted_files_pruned(_: &Core) {
    // todo: unskip
    // core.db
    //     .transaction(|tx| {
    //         let context = core.context(tx).unwrap();
    //         let mut base = context.tx.base_metadata.to_lazy();
    //         let deleted_base_ids = base
    //             .owned_ids()
    //             .into_iter()
    //             .filter(|id| base.calculate_deleted(id).unwrap())
    //             .collect::<Vec<Uuid>>();
    //         if !deleted_base_ids.is_empty() {
    //             panic!("some deleted files are not pruned:{:?}", deleted_base_ids);
    //         }
    //         let mut local = base.stage(&mut context.tx.local_metadata);
    //         let deleted_local_ids = local
    //             .owned_ids()
    //             .into_iter()
    //             .filter(|id| local.calculate_deleted(id).unwrap())
    //             .collect::<Vec<Uuid>>();
    //         if !deleted_local_ids.is_empty() {
    //             panic!("some deleted files are not pruned:{:?}", deleted_local_ids);
    //         }
    //     })
    //     .unwrap();
}
