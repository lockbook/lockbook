use crate::{get_dirty_ids, slices_equal_ignore_order, test_core_from};
use lb_rs::Lb;
use lb_rs::model::api::GetUpdatesRequest;
use lb_rs::model::file_like::FileLike;
use lb_rs::model::file_metadata::{FileType, Owner};
use lb_rs::model::path_ops::Filter::DocumentsOnly;
use lb_rs::model::staged::StagedTreeLikeMut;
use lb_rs::model::tree_like::TreeLike;
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

// todo: from the perspective of the fuzzer, this probably expects to additionally compare that
// a core's documents are equal. This may be giving us a false sense of security
pub async fn cores_equal(left: &Lb, right: &Lb) {
    assert_eq!(&left.get_account().unwrap(), &right.get_account().unwrap());
    assert_eq!(&left.root().await.unwrap(), &right.root().await.unwrap());

    let mut left_tx = left.begin_tx().await;
    let mut right_tx = right.begin_tx().await;

    assert_eq!(&left_tx.db().local_metadata.get(), &right_tx.db().local_metadata.get());
    assert_eq!(&left_tx.db().base_metadata.get(), &right_tx.db().base_metadata.get());
}

pub async fn new_synced_client_core_equal(lb: &Lb) {
    let new_client = test_core_from(lb).await;

    let tx = lb.ro_tx().await;
    let db = tx.db();

    let account = db.account.get().unwrap().clone();
    let mut local = db.base_metadata.stage(&db.local_metadata).to_lazy();
    local.validate(Owner(account.public_key())).unwrap();

    drop(tx);

    cores_equal(lb, &new_client).await;
}

pub async fn all_ids(core: &Lb, expected_ids: &[Uuid]) {
    let mut expected_ids: Vec<Uuid> = expected_ids.to_vec();
    let mut actual_ids: Vec<Uuid> = core
        .list_metadatas()
        .await
        .unwrap()
        .iter()
        .map(|f| f.id)
        .collect();

    actual_ids.sort();
    expected_ids.sort();
    if actual_ids != expected_ids {
        panic!("ids did not match expectation. expected={expected_ids:?}; actual={actual_ids:?}");
    }
}

pub async fn all_children_ids(core: &Lb, id: &Uuid, expected_ids: &[Uuid]) {
    let mut expected_ids: Vec<Uuid> = expected_ids.to_vec();
    let mut actual_ids: Vec<Uuid> = core
        .get_children(id)
        .await
        .unwrap()
        .iter()
        .map(|f| f.id)
        .collect();

    actual_ids.sort();
    expected_ids.sort();
    if actual_ids != expected_ids {
        panic!("ids did not match expectation. expected={expected_ids:?}; actual={actual_ids:?}");
    }
}

pub async fn all_recursive_children_ids(core: &Lb, id: Uuid, expected_ids: &[Uuid]) {
    let mut expected_ids: Vec<Uuid> = expected_ids.to_vec();
    let mut actual_ids: Vec<Uuid> = core
        .get_and_get_children_recursively(&id)
        .await
        .unwrap()
        .iter()
        .map(|f| f.id)
        .collect();

    actual_ids.sort();
    expected_ids.sort();
    if actual_ids != expected_ids {
        panic!("ids did not match expectation. expected={expected_ids:?}; actual={actual_ids:?}");
    }
}

pub async fn all_paths(core: &Lb, expected_paths: &[&str]) {
    let mut expected_paths: Vec<String> = expected_paths
        .iter()
        .map(|&path| String::from(path))
        .collect();
    let mut actual_paths: Vec<String> = core.list_paths(None).await.unwrap();

    actual_paths.sort();
    expected_paths.sort();
    if actual_paths != expected_paths {
        panic!(
            "paths did not match expectation. expected={expected_paths:?}; actual={actual_paths:?}"
        );
    }
}

pub async fn all_document_contents(db: &Lb, expected_contents_by_path: &[(&str, &[u8])]) {
    let expected_contents_by_path = expected_contents_by_path
        .iter()
        .map(|&(path, contents)| (path.to_string(), contents.to_vec()))
        .collect::<Vec<(String, Vec<u8>)>>();
    let actual_contents_by_path = {
        let paths = db.list_paths(Some(DocumentsOnly)).await.unwrap();
        let mut this = Vec::new();
        for path in paths {
            let doc = db.get_by_path(&path).await.unwrap();
            this.push((path.clone(), db.read_document(doc.id, false).await.unwrap()));
        }
        this
    };
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

pub async fn all_pending_shares(core: &Lb, expected_names: &[&str]) {
    if expected_names.iter().any(|&path| path.contains('/')) {
        panic!(
            "improper call to assert_all_pending_shares; expected_names must not contain with '/'. expected_names={expected_names:?}"
        );
    }
    let mut expected_names: Vec<String> = expected_names
        .iter()
        .map(|&name| String::from(name))
        .collect();
    let mut actual_names: Vec<String> = core
        .get_pending_shares()
        .await
        .unwrap()
        .into_iter()
        .map(|f| f.name)
        .collect();
    actual_names.sort();
    expected_names.sort();
    if actual_names != expected_names {
        panic!(
            "pending share names did not match expectation. expected={expected_names:?}; actual={actual_names:?}"
        );
    }
}

pub async fn local_work_paths(lb: &Lb, expected_paths: &[&'static str]) {
    let dirty = get_dirty_ids(lb, false).await;
    let mut expected_paths = expected_paths.to_vec();

    let tx = lb.ro_tx().await;
    let db = tx.db();

    let mut local = db.base_metadata.stage(&db.local_metadata).to_lazy();
    let mut actual_paths = dirty
        .iter()
        .filter(|id| !local.find(id).unwrap().is_link())
        .collect::<Vec<_>>()
        .iter()
        .filter(|id| !local.in_pending_share(id).unwrap())
        .collect::<Vec<_>>()
        .iter()
        .map(|id| local.id_to_path(id, &lb.keychain))
        .collect::<Result<Vec<String>, _>>()
        .unwrap();
    actual_paths.sort_unstable();
    expected_paths.sort_unstable();
    if actual_paths != expected_paths {
        panic!(
            "local work paths did not match expectation. expected={expected_paths:?}; actual={actual_paths:?}"
        );
    }
}

pub async fn server_work_paths(core: &Lb, expected_paths: &[&'static str]) {
    let mut expected_paths = expected_paths.to_vec();

    let tx = core.ro_tx().await;
    let db = tx.db();

    let account = db.account.get().unwrap();
    let remote_changes = core
        .client
        .request(
            account,
            GetUpdatesRequest {
                since_metadata_version: db.last_synced.get().copied().unwrap_or_default() as u64,
            },
        )
        .await
        .unwrap()
        .file_metadata;
    let mut remote = db
        .base_metadata
        .stage(remote_changes)
        .pruned()
        .unwrap()
        .to_lazy();
    let mut actual_paths = remote
        .tree
        .staged
        .ids()
        .iter()
        .filter(|id| !matches!(remote.find(id).unwrap().file_type(), FileType::Link { .. }))
        .collect::<Vec<_>>()
        .iter()
        .filter(|id| !remote.in_pending_share(id).unwrap())
        .collect::<Vec<_>>()
        .iter()
        .map(|id| remote.id_to_path(id, &core.keychain))
        .collect::<Result<Vec<String>, _>>()
        .unwrap();
    actual_paths.sort_unstable();
    expected_paths.sort_unstable();
    if actual_paths != expected_paths {
        panic!(
            "server work paths did not match expectation. expected={expected_paths:?}; actual={actual_paths:?}"
        );
    }
}

pub fn deleted_files_pruned(_: &Lb) {
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
