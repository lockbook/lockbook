//! The release that reads V2 metadata keeps syncing after the forced upgrade, alongside this one,
//! which writes it. Each test puts one device on each and checks they agree.
// the read release reaches the server over HTTP
#![cfg(not(feature = "in-process-tests"))]

use lb_rs::Lb;
use lb_rs::model::file::ShareMode;
use lb_rs::model::file_like::FileLike;
use lb_rs::model::file_metadata::FileType;
use lb_rs_read::Lb as ReadLb;
use lb_rs_read::model::core_config::{ClientType, Config};
use test_utils::*;
use uuid::Uuid;

async fn read_device() -> ReadLb {
    ReadLb::init(Config {
        writeable_path: format!("/tmp/{}", Uuid::new_v4()),
        logs: false,
        stdout_logs: false,
        colored_logs: false,
        background_work: false,
        client_name: "tests".into(),
        build_info: None,
        client_type: ClientType::Unknown,
        ..Default::default()
    })
    .await
    .unwrap()
}

/// Another device on the same account, on the read release.
async fn read_device_of(core: &Lb) -> ReadLb {
    let device = read_device().await;
    let key = core.export_account_private_key().unwrap();
    device.import_account(&key, Some(&url())).await.unwrap();
    device.sync().await.unwrap();
    device
}

macro_rules! files {
    ($core:expr) => {{
        let mut files: Vec<(String, String, String)> = $core
            .list_metadatas()
            .await
            .unwrap()
            .into_iter()
            .map(|f| (f.id.to_string(), f.parent.to_string(), f.name))
            .collect();
        files.sort();
        files
    }};
}

async fn attached(core: &Lb, folder: Uuid) -> Option<Uuid> {
    let tx = core.ro_tx().await;
    let db = tx.db();
    db.base_metadata
        .get(&folder)
        .and_then(|f| f.attachments_of())
        .map(|of| of.id)
}

/// plan.md with plan.assets beside it, holding chart.png.
struct Plan {
    doc: Uuid,
    folder: Uuid,
    image: Uuid,
}

async fn plan(core: &Lb, parent: Uuid) -> Plan {
    let doc = core
        .create_file("plan.md", &parent, FileType::Document)
        .await
        .unwrap()
        .id;
    core.write_document(doc, b"![chart](plan.assets/chart.png)")
        .await
        .unwrap();
    let folder = core
        .create_attachments_folder(&doc, "plan.assets")
        .await
        .unwrap()
        .id;
    let image = core
        .create_file("chart.png", &folder, FileType::Document)
        .await
        .unwrap()
        .id;
    core.write_document(image, b"png").await.unwrap();
    core.sync().await.unwrap();
    Plan { doc, folder, image }
}

#[tokio::test]
async fn a_read_device_edits_attachments_without_losing_the_link() {
    let core = test_core_with_account().await;
    let root = core.root().await.unwrap().id;
    let plan = plan(&core, root).await;
    let old = read_device_of(&core).await;

    assert_eq!(old.read_document(plan.image, false).await.unwrap(), b"png");
    old.rename_file(&plan.image, "chart-2.png").await.unwrap();
    old.write_document(plan.doc, b"![chart](plan.assets/chart-2.png)")
        .await
        .unwrap();
    let archive = old.create_at_path("archive/").await.unwrap().id;
    old.move_file(&plan.folder, &archive).await.unwrap();
    old.sync().await.unwrap();
    core.sync().await.unwrap();

    assert_eq!(files!(core), files!(old));
    assert_eq!(attached(&core, plan.folder).await, Some(plan.doc));
}

#[tokio::test]
async fn a_read_device_sees_attachments_deleted_with_their_document() {
    let core = test_core_with_account().await;
    let root = core.root().await.unwrap().id;
    let plan = plan(&core, root).await;
    let old = read_device_of(&core).await;

    core.delete(&plan.doc).await.unwrap();
    core.sync().await.unwrap();
    old.sync().await.unwrap();

    assert!(old.get_file_by_id(plan.image).await.is_err());
    assert_eq!(files!(core), files!(old));
}

#[tokio::test]
async fn a_read_device_deleting_a_document_deletes_its_attachments() {
    let core = test_core_with_account().await;
    let root = core.root().await.unwrap().id;
    let plan = plan(&core, root).await;
    let old = read_device_of(&core).await;

    old.delete(&plan.doc).await.unwrap();
    old.sync().await.unwrap();
    core.sync().await.unwrap();

    assert!(core.get_file_by_id(plan.image).await.is_err());
    assert_eq!(files!(core), files!(old));
}

#[tokio::test]
async fn an_offline_edit_to_a_deleted_attachment_still_syncs() {
    let core = test_core_with_account().await;
    let root = core.root().await.unwrap().id;
    let plan = plan(&core, root).await;
    let old = read_device_of(&core).await;

    core.delete(&plan.doc).await.unwrap();
    core.sync().await.unwrap();
    old.write_document(plan.image, b"edited offline")
        .await
        .unwrap();
    old.sync().await.unwrap();
    core.sync().await.unwrap();

    assert!(old.get_file_by_id(plan.image).await.is_err());
    assert_eq!(files!(core), files!(old));
}

#[tokio::test]
async fn files_the_read_release_writes_keep_working() {
    let core = test_core_with_account().await;
    let old = read_device_of(&core).await;

    let doc = old.create_at_path("notes.md").await.unwrap().id;
    old.write_document(doc, b"notes").await.unwrap();
    old.sync().await.unwrap();
    core.sync().await.unwrap();
    core.rename_file(&doc, "notes-2.md").await.unwrap();
    let folder = core
        .create_attachments_folder(&doc, "notes.assets")
        .await
        .unwrap()
        .id;
    core.sync().await.unwrap();
    old.sync().await.unwrap();
    old.write_document(doc, b"notes, again").await.unwrap();
    old.sync().await.unwrap();
    core.sync().await.unwrap();

    assert_eq!(core.read_document(doc, false).await.unwrap(), b"notes, again");
    assert_eq!(files!(core), files!(old));
    assert_eq!(attached(&core, folder).await, Some(doc));
}

#[tokio::test]
async fn a_recipient_on_the_read_release_works_in_shared_attachments() {
    let alice = test_core_with_account().await;
    let bob = read_device().await;
    let bob_name = random_name();
    bob.create_account(&bob_name, &url(), false).await.unwrap();
    let root = alice.root().await.unwrap().id;
    let plan = plan(&alice, root).await;
    alice
        .share_file(plan.doc, &bob_name, ShareMode::Write)
        .await
        .unwrap();
    alice.sync().await.unwrap();

    bob.sync().await.unwrap();
    assert_eq!(bob.read_document(plan.image, false).await.unwrap(), b"png");
    bob.rename_file(&plan.image, "chart-2.png").await.unwrap();
    bob.write_document(plan.image, b"png, edited")
        .await
        .unwrap();
    bob.sync().await.unwrap();
    alice.sync().await.unwrap();

    assert_eq!(alice.get_file_by_id(plan.image).await.unwrap().name, "chart-2.png");
    assert_eq!(alice.read_document(plan.image, false).await.unwrap(), b"png, edited");
}

#[tokio::test]
async fn a_recipient_on_the_read_release_takes_a_replaced_folder() {
    let alice = test_core_with_account().await;
    let bob = read_device().await;
    let bob_name = random_name();
    bob.create_account(&bob_name, &url(), false).await.unwrap();
    let root = alice.root().await.unwrap().id;
    let plan = plan(&alice, root).await;
    alice
        .share_file(plan.doc, &bob_name, ShareMode::Read)
        .await
        .unwrap();
    let boxed = alice.create_at_path("box/").await.unwrap().id;
    alice.move_file(&plan.folder, &boxed).await.unwrap();
    alice.sync().await.unwrap();
    bob.sync().await.unwrap();

    // bob can't see box, so the server stops sending him the old folder
    alice.delete(&boxed).await.unwrap();
    let folder = alice
        .create_attachments_folder(&plan.doc, "plan.assets")
        .await
        .unwrap()
        .id;
    let image = alice
        .create_file("chart.png", &folder, FileType::Document)
        .await
        .unwrap()
        .id;
    alice.write_document(image, b"new png").await.unwrap();
    alice.sync().await.unwrap();
    bob.sync().await.unwrap();

    assert_eq!(bob.read_document(image, false).await.unwrap(), b"new png");
    assert!(bob.get_file_by_id(plan.folder).await.is_err());
    assert!(bob.get_file_by_id(plan.image).await.is_err());
}

#[tokio::test]
async fn two_devices_creating_attachments_offline_end_with_one_folder() {
    let core = test_core_with_account().await;
    let doc = core.create_at_path("plan.md").await.unwrap().id;
    core.sync().await.unwrap();
    let other = test_core_from(&core).await;
    let old = read_device_of(&core).await;

    let first = core
        .create_attachments_folder(&doc, "plan.assets")
        .await
        .unwrap()
        .id;
    core.create_file("chart.png", &first, FileType::Document)
        .await
        .unwrap();
    let second = other
        .create_attachments_folder(&doc, "plan.assets")
        .await
        .unwrap()
        .id;
    let photo = other
        .create_file("photo.png", &second, FileType::Document)
        .await
        .unwrap()
        .id;
    core.sync().await.unwrap();
    other.sync().await.unwrap();
    core.sync().await.unwrap();
    old.sync().await.unwrap();

    assert_eq!(other.get_file_by_id(photo).await.unwrap().parent, first);
    assert!(other.get_file_by_id(second).await.is_err());
    assert_eq!(files!(core), files!(other));
    assert_eq!(files!(core), files!(old));
}

async fn shared_plan(mode: ShareMode) -> (Lb, ReadLb, Plan) {
    let alice = test_core_with_account().await;
    let bob = read_device().await;
    let bob_name = random_name();
    bob.create_account(&bob_name, &url(), false).await.unwrap();
    let root = alice.root().await.unwrap().id;
    let plan = plan(&alice, root).await;
    alice.share_file(plan.doc, &bob_name, mode).await.unwrap();
    alice.sync().await.unwrap();
    bob.sync().await.unwrap();
    (alice, bob, plan)
}

#[tokio::test]
async fn a_recipient_on_the_read_release_adds_attachments() {
    let (alice, bob, plan) = shared_plan(ShareMode::Write).await;

    let photo = bob
        .create_file(
            "photo.png",
            &plan.folder,
            lb_rs_read::model::file_metadata::FileType::Document,
        )
        .await
        .unwrap()
        .id;
    bob.write_document(photo, b"photo").await.unwrap();
    bob.sync().await.unwrap();
    alice.sync().await.unwrap();

    assert_eq!(alice.get_file_by_id(photo).await.unwrap().parent, plan.folder);
    assert_eq!(alice.read_document(photo, false).await.unwrap(), b"photo");
}

#[tokio::test]
async fn a_recipient_on_the_read_release_loses_attachments_with_the_share() {
    let (alice, bob, plan) = shared_plan(ShareMode::Read).await;
    let carol = read_device().await;
    let carol_name = random_name();
    carol
        .create_account(&carol_name, &url(), false)
        .await
        .unwrap();
    alice
        .share_file(plan.doc, &carol_name, ShareMode::Read)
        .await
        .unwrap();
    alice.sync().await.unwrap();
    carol.sync().await.unwrap();
    assert_eq!(carol.read_document(plan.image, false).await.unwrap(), b"png");

    bob.reject_share(&plan.doc).await.unwrap();
    bob.sync().await.unwrap();
    alice.unshare_file(plan.doc, &carol_name).await.unwrap();
    alice.sync().await.unwrap();
    carol.sync().await.unwrap();

    assert!(bob.get_file_by_id(plan.image).await.is_err());
    assert!(carol.get_file_by_id(plan.image).await.is_err());
}

#[tokio::test]
async fn a_folder_made_offline_joins_the_live_folder_not_a_deleted_one() {
    let core = test_core_with_account().await;
    let doc = core.create_at_path("plan.md").await.unwrap().id;
    core.sync().await.unwrap();
    let other = test_core_from(&core).await;

    let deleted = core
        .create_attachments_folder(&doc, "plan.assets")
        .await
        .unwrap()
        .id;
    core.sync().await.unwrap();
    core.delete(&deleted).await.unwrap();
    let live = core
        .create_attachments_folder(&doc, "plan.assets")
        .await
        .unwrap()
        .id;
    core.sync().await.unwrap();
    let folder = other
        .create_attachments_folder(&doc, "plan.assets")
        .await
        .unwrap()
        .id;
    let photo = other
        .create_file("photo.png", &folder, FileType::Document)
        .await
        .unwrap()
        .id;
    other.sync().await.unwrap();

    assert_eq!(other.get_file_by_id(photo).await.unwrap().parent, live);
}
