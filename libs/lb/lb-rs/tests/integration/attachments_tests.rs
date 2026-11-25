use lb_rs::io::server::Server;
use lb_rs::model::ValidationFailure;
use lb_rs::model::account::Account;
use lb_rs::model::api::UpsertRequestV2;
use lb_rs::model::core_config::Config;
use lb_rs::model::errors::LbErrKind;
use lb_rs::model::file::ShareMode;
use lb_rs::model::file_like::FileLike;
use lb_rs::model::file_metadata::{FileDiff, FileType};
use lb_rs::model::meta::Meta;
use lb_rs::model::signed_meta::SignedMeta;
use lb_rs::model::tree_like::TreeLike;
use test_utils::*;

#[tokio::test]
async fn creation_is_off_by_default() {
    let core = test_core_with_account().await;
    let doc = core.create_at_path("/plan.md").await.unwrap();

    let err = core.create_attachments_folder(&doc.id).await.unwrap_err();
    assert_eq!(err.kind, LbErrKind::InsufficientPermission);
}

#[tokio::test]
async fn created_beside_the_document_and_named_for_it() {
    let core = test_core_with_attachments().await;
    let doc = core.create_at_path("/notes/plan.md").await.unwrap();

    let folder = core.create_attachments_folder(&doc.id).await.unwrap();

    assert_eq!(folder.parent, doc.parent);
    assert_eq!(folder.name, "plan-attachments");
    assert!(folder.is_folder());
    assert_eq!(
        core.get_attachments_folder(&doc.id)
            .await
            .unwrap()
            .unwrap()
            .id,
        folder.id
    );
}

#[tokio::test]
async fn a_taken_name_is_numbered() {
    let core = test_core_with_attachments().await;
    core.create_at_path("/plan-attachments/").await.unwrap();
    let doc = core.create_at_path("/plan.md").await.unwrap();

    let folder = core.create_attachments_folder(&doc.id).await.unwrap();

    assert_eq!(folder.name, "plan-attachments 2");
}

#[tokio::test]
async fn one_per_document() {
    let core = test_core_with_attachments().await;
    let doc = core.create_at_path("/plan.md").await.unwrap();
    core.create_attachments_folder(&doc.id).await.unwrap();

    let err = core.create_attachments_folder(&doc.id).await.unwrap_err();
    assert_eq!(
        err.kind,
        LbErrKind::Validation(ValidationFailure::DuplicateAttachmentsFolder { document: doc.id })
    );
}

#[tokio::test]
async fn keeps_its_name_when_its_document_is_renamed() {
    let core = test_core_with_attachments().await;
    let doc = core.create_at_path("/plan.md").await.unwrap();
    let folder = core.create_attachments_folder(&doc.id).await.unwrap();

    core.rename_file(&doc.id, "trip.md").await.unwrap();

    assert_eq!(core.get_file_by_id(folder.id).await.unwrap().name, "plan-attachments");
}

#[tokio::test]
async fn moved_where_its_name_is_taken_it_is_numbered() {
    let core = test_core_with_attachments().await;
    let doc = core.create_at_path("/plan.md").await.unwrap();
    let folder = core.create_attachments_folder(&doc.id).await.unwrap();
    let squat = core
        .create_at_path("/notes/plan-attachments/")
        .await
        .unwrap();

    core.move_file(&doc.id, &squat.parent).await.unwrap();

    let folder = core.get_file_by_id(folder.id).await.unwrap();
    assert_eq!(folder.parent, squat.parent);
    assert_eq!(folder.name, "plan-attachments 2");
}

#[tokio::test]
async fn only_the_folder_is_written_as_v2() {
    let core = test_core_with_attachments().await;
    let doc = core.create_at_path("/plan.md").await.unwrap();
    let folder = core.create_attachments_folder(&doc.id).await.unwrap();
    core.create_at_path("/plan-attachments/image.png")
        .await
        .unwrap();
    core.sync().await.unwrap();

    // a device with creation off reads it like one with it on
    let other = test_core_from(&core).await;
    let tx = other.ro_tx().await;
    let db = tx.db();
    for id in db.base_metadata.ids() {
        let file = db.base_metadata.find(&id).unwrap();
        let of = file.attachment_of().map(|of| of.id);
        assert_eq!(of, (id == folder.id).then_some(doc.id), "{id}");
        let v2 = matches!(file.timestamped_value.value, Meta::V2 { .. });
        assert_eq!(v2, id == folder.id, "{id}");
    }
    drop(tx);
    assert_eq!(
        other
            .get_attachments_folder(&doc.id)
            .await
            .unwrap()
            .unwrap()
            .id,
        folder.id
    );
}

fn rewritten_as_v2(file: &SignedMeta, account: &Account) -> SignedMeta {
    let Meta::V1 {
        id,
        file_type,
        parent,
        name,
        owner,
        is_deleted,
        doc_size,
        doc_hmac,
        user_access_keys,
        folder_access_key,
    } = file.timestamped_value.value.clone()
    else {
        panic!("already V2")
    };
    Meta::V2 {
        id,
        file_type,
        parent,
        name,
        owner,
        is_deleted,
        doc_size,
        doc_hmac,
        user_access_keys,
        folder_access_key,
        attachment_of: None,
    }
    .sign_with(account)
    .unwrap()
}

#[tokio::test]
async fn any_file_can_be_rewritten_as_v2() {
    let core = test_core_with_account().await;
    let account = core.get_account().unwrap();
    let doc = core.create_at_path("/notes/plan.md").await.unwrap();
    core.write_document(doc.id, b"plan").await.unwrap();
    core.sync().await.unwrap();

    // the root, a folder and a document are rewritten under a device with a rename to push
    let updates: Vec<_> = {
        let tx = core.ro_tx().await;
        let base = &tx.db().base_metadata;
        base.ids()
            .into_iter()
            .map(|id| {
                let old = base.find(&id).unwrap().clone();
                let new = rewritten_as_v2(&old, account);
                assert!(old.timestamped_value.value != new.timestamped_value.value);
                FileDiff::edit(old, new)
            })
            .collect()
    };
    assert_eq!(updates.len(), 3);
    core.rename_file(&doc.id, "idea.md").await.unwrap();
    Server::upsert_file_metadata_v2(&core.client, account, UpsertRequestV2 { updates })
        .await
        .unwrap();
    core.sync().await.unwrap();

    let other = test_core_from(&core).await;
    for device in [&core, &other] {
        let tx = device.ro_tx().await;
        let base = &tx.db().base_metadata;
        for id in base.ids() {
            let file = &base.find(&id).unwrap().timestamped_value.value;
            assert!(matches!(file, Meta::V2 { attachment_of: None, .. }), "{id}");
        }
        drop(tx);
        let doc = device.get_by_path("/notes/idea.md").await.unwrap();
        assert_eq!(device.read_document(doc.id, false).await.unwrap(), b"plan");
    }

    // and they are edited as before
    let root = other.root().await.unwrap();
    other.move_file(&doc.id, &root.id).await.unwrap();
    other.write_document(doc.id, b"idea").await.unwrap();
    other.sync().await.unwrap();
    core.sync().await.unwrap();
    let doc = core.get_by_path("/idea.md").await.unwrap();
    assert_eq!(core.read_document(doc.id, false).await.unwrap(), b"idea");
}

#[tokio::test]
async fn follows_its_document() {
    let a = test_core_with_attachments().await;
    let doc = a.create_at_path("/plan.md").await.unwrap();
    let folder = a.create_attachments_folder(&doc.id).await.unwrap();
    let dest = a.create_at_path("/notes/").await.unwrap();
    a.sync().await.unwrap();
    let b = test_core_from(&a).await;

    a.rename_file(&doc.id, "trip.md").await.unwrap();
    a.move_file(&doc.id, &dest.id).await.unwrap();
    a.sync().await.unwrap();
    b.sync().await.unwrap();

    let folder = b.get_file_by_id(folder.id).await.unwrap();
    assert_eq!(folder.parent, dest.id);
    assert_eq!(folder.name, "plan-attachments");
    assert_dbs_equal(&a, &b).await;
}

#[tokio::test]
async fn cannot_be_moved_or_renamed_itself() {
    let core = test_core_with_attachments().await;
    let doc = core.create_at_path("/plan.md").await.unwrap();
    let folder = core.create_attachments_folder(&doc.id).await.unwrap();
    let dest = core.create_at_path("/notes/").await.unwrap();

    let err = core.rename_file(&folder.id, "other").await.unwrap_err();
    assert_eq!(err.kind, LbErrKind::AttachmentsFolderFixed);
    let err = core.move_file(&folder.id, &dest.id).await.unwrap_err();
    assert_eq!(err.kind, LbErrKind::AttachmentsFolderFixed);

    // this is the question the Files app asks so it can grey out Rename and Move on the attachments
    // folder and on nothing else
    assert_eq!(core.attachments_of(&folder.id).await.unwrap(), Some(doc.id));
    assert_eq!(core.attachments_of(&doc.id).await.unwrap(), None);
    assert_eq!(core.attachments_of(&dest.id).await.unwrap(), None);
}

#[tokio::test]
async fn a_note_shared_alone_brings_its_folder() {
    let owner = test_core_with_attachments().await;
    let reader = test_core_with_account().await;
    let doc = owner.create_at_path("/notes/plan.md").await.unwrap();
    let folder = owner.create_attachments_folder(&doc.id).await.unwrap();
    let image = owner
        .create_at_path("/notes/plan-attachments/image.png")
        .await
        .unwrap();
    owner.write_document(image.id, b"pixels").await.unwrap();
    let reader_name = reader.get_account().unwrap().username.clone();
    owner
        .share_file(doc.id, &reader_name, ShareMode::Read)
        .await
        .unwrap();
    owner.sync().await.unwrap();

    reader.sync().await.unwrap();

    let pending = reader.get_pending_shares().await.unwrap();
    assert_eq!(pending.iter().map(|f| f.id).collect::<Vec<_>>(), [doc.id]);
    assert_eq!(reader.read_document(image.id, false).await.unwrap(), b"pixels");
    assert_eq!(
        reader
            .get_attachments_folder(&doc.id)
            .await
            .unwrap()
            .unwrap()
            .id,
        folder.id
    );

    // added later, still the reader's
    let later = owner
        .create_at_path("/notes/plan-attachments/later.png")
        .await
        .unwrap();
    owner.sync().await.unwrap();
    reader.sync().await.unwrap();
    reader.get_file_by_id(later.id).await.unwrap();

    // a deleted folder stops reaching the reader
    owner.delete(&folder.id).await.unwrap();
    owner.sync().await.unwrap();
    reader.sync().await.unwrap();
    assert!(reader.get_file_by_id(folder.id).await.is_err());
    assert!(reader.get_file_by_id(image.id).await.is_err());
    reader.get_file_by_id(doc.id).await.unwrap();
}

#[tokio::test]
async fn a_reader_lists_a_note_shared_alone_with_its_folder() {
    let owner = test_core_with_attachments().await;
    let reader = test_core_with_account().await;
    let doc = owner.create_at_path("/notes/plan.md").await.unwrap();
    let folder = owner.create_attachments_folder(&doc.id).await.unwrap();
    let image = owner
        .create_at_path("/notes/plan-attachments/image.png")
        .await
        .unwrap();
    let reader_name = reader.get_account().unwrap().username.clone();
    owner
        .share_file(doc.id, &reader_name, ShareMode::Read)
        .await
        .unwrap();
    owner.sync().await.unwrap();
    reader.sync().await.unwrap();
    reader
        .create_link_at_path("/shared/renamed.md", doc.id)
        .await
        .unwrap();

    // the app finds the note's folder by what it's for, whatever the reader named the note
    let attachments = reader.list_attachments().await.unwrap();
    assert_eq!(attachments.folders.get(&doc.id), Some(&folder.id));
    let mut ids: Vec<_> = attachments.files.iter().map(|f| f.id).collect();
    ids.sort();
    let mut expected = vec![folder.id, image.id];
    expected.sort();
    assert_eq!(ids, expected);
}

#[tokio::test]
async fn the_folder_of_a_note_shared_alone_has_no_path() {
    let owner = test_core_with_attachments().await;
    let reader = test_core_with_account().await;
    let reader =
        test_core_from_with(&reader, Config { list_attachments: true, ..test_config() }).await;
    let doc = owner.create_at_path("/notes/plan.md").await.unwrap();
    let folder = owner.create_attachments_folder(&doc.id).await.unwrap();
    let image = owner
        .create_at_path("/notes/plan-attachments/image.png")
        .await
        .unwrap();
    owner.write_document(image.id, b"pixels").await.unwrap();
    let reader_name = reader.get_account().unwrap().username.clone();
    owner
        .share_file(doc.id, &reader_name, ShareMode::Read)
        .await
        .unwrap();
    owner.sync().await.unwrap();
    reader.sync().await.unwrap();
    let link = reader
        .create_link_at_path("/shared/renamed.md", doc.id)
        .await
        .unwrap();

    // reached through the note
    assert_eq!(
        reader
            .get_attachments_folder(&doc.id)
            .await
            .unwrap()
            .unwrap()
            .id,
        folder.id
    );
    assert_eq!(reader.read_document(image.id, false).await.unwrap(), b"pixels");

    // nowhere in the reader's tree
    assert!(reader.get_path_by_id(folder.id).await.is_err());
    assert!(reader.get_path_by_id(image.id).await.is_err());
    assert!(
        reader
            .get_by_path("/shared/plan-attachments/")
            .await
            .is_err()
    );
    reader.list_paths(None).await.unwrap();
    let listed: Vec<_> = reader
        .list_metadatas()
        .await
        .unwrap()
        .into_iter()
        .map(|f| f.id)
        .collect();
    assert!(!listed.contains(&folder.id) && !listed.contains(&image.id));
    let children: Vec<_> = reader
        .get_children(&link.parent)
        .await
        .unwrap()
        .into_iter()
        .map(|f| f.id)
        .collect();
    assert_eq!(children, [doc.id]);

    // an export takes it with the note
    let dest = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
    std::fs::create_dir_all(&dest).unwrap();
    reader
        .export_file(link.parent, dest.clone(), false, &Some(|_| {}))
        .await
        .unwrap();
    assert!(dest.join("shared/renamed.md").exists());
    assert_eq!(std::fs::read(dest.join("shared/plan-attachments/image.png")).unwrap(), b"pixels");
    std::fs::remove_dir_all(dest).unwrap();
}

#[tokio::test]
async fn a_deleted_folder_stops_reaching_a_reader_before_a_new_one_does() {
    let owner = test_core_with_attachments().await;
    let reader = test_core_with_account().await;
    let doc = owner.create_at_path("/notes/plan.md").await.unwrap();
    let old = owner.create_attachments_folder(&doc.id).await.unwrap();
    let old_image = owner
        .create_at_path("/notes/plan-attachments/image.png")
        .await
        .unwrap();
    let reader_name = reader.get_account().unwrap().username.clone();
    owner
        .share_file(doc.id, &reader_name, ShareMode::Read)
        .await
        .unwrap();
    owner.sync().await.unwrap();
    reader.sync().await.unwrap();
    assert_eq!(
        reader
            .get_attachments_folder(&doc.id)
            .await
            .unwrap()
            .unwrap()
            .id,
        old.id
    );

    // replaced in one sync, so the reader learns of both in one
    owner.delete(&old.id).await.unwrap();
    let new = owner.create_attachments_folder(&doc.id).await.unwrap();
    let new_image = owner
        .create_at_path("/notes/plan-attachments/image.png")
        .await
        .unwrap();
    owner.write_document(new_image.id, b"pixels").await.unwrap();
    owner.sync().await.unwrap();
    reader.sync().await.unwrap();

    assert_eq!(
        reader
            .get_attachments_folder(&doc.id)
            .await
            .unwrap()
            .unwrap()
            .id,
        new.id
    );
    assert!(reader.get_file_by_id(old.id).await.is_err());
    assert!(reader.get_file_by_id(old_image.id).await.is_err());
    assert_eq!(reader.read_document(new_image.id, false).await.unwrap(), b"pixels");
}

#[tokio::test]
async fn a_writers_new_folder_follows_its_note_out_of_a_folder_that_leaves_the_share() {
    let owner = test_core_with_attachments().await;
    let writer = test_core_with_attachments().await;
    let trip = owner.create_at_path("/Trip/").await.unwrap();
    let flights = owner.create_at_path("/Trip/flights/").await.unwrap();
    let doc = owner
        .create_at_path("/Trip/flights/outbound.md")
        .await
        .unwrap();
    let notes = owner.create_at_path("/Notes/").await.unwrap();
    let writer_name = writer.get_account().unwrap().username.clone();
    owner
        .share_file(trip.id, &writer_name, ShareMode::Write)
        .await
        .unwrap();
    owner.sync().await.unwrap();
    writer.sync().await.unwrap();
    writer.create_link_at_path("/Trip", trip.id).await.unwrap();
    writer.sync().await.unwrap();

    // offline, the writer adds an image; meanwhile the owner tidies the trip
    let folder = writer.create_attachments_folder(&doc.id).await.unwrap();
    let image = writer
        .create_file("image.png", &folder.id, FileType::Document)
        .await
        .unwrap();
    writer.write_document(image.id, b"pixels").await.unwrap();
    owner.move_file(&doc.id, &trip.id).await.unwrap();
    owner.sync().await.unwrap();
    owner.move_file(&flights.id, &notes.id).await.unwrap();
    owner.sync().await.unwrap();

    writer.sync().await.unwrap();
    owner.sync().await.unwrap();

    for core in [&writer, &owner] {
        let found = core.get_attachments_folder(&doc.id).await.unwrap().unwrap();
        assert_eq!(found.id, folder.id);
        assert_eq!(core.get_file_by_id(folder.id).await.unwrap().parent, trip.id);
        assert_eq!(core.read_document(image.id, false).await.unwrap(), b"pixels");
    }
}

#[tokio::test]
async fn a_writer_of_a_note_shared_alone_adds_images() {
    let owner = test_core_with_attachments().await;
    let writer = test_core_with_attachments().await;
    let doc = owner.create_at_path("/notes/plan.md").await.unwrap();
    let other = owner.create_at_path("/notes/other.md").await.unwrap();
    let folder = owner.create_attachments_folder(&doc.id).await.unwrap();
    let writer_name = writer.get_account().unwrap().username.clone();
    owner
        .share_file(doc.id, &writer_name, ShareMode::Write)
        .await
        .unwrap();
    owner.sync().await.unwrap();
    // shared from a device that makes no folders, so `other` has none
    let off = test_core_from(&owner).await;
    off.share_file(other.id, &writer_name, ShareMode::Write)
        .await
        .unwrap();
    off.sync().await.unwrap();
    writer.sync().await.unwrap();

    let image = writer
        .create_file("image.png", &folder.id, FileType::Document)
        .await
        .unwrap();
    writer.write_document(image.id, b"pixels").await.unwrap();
    writer.sync().await.unwrap();
    owner.sync().await.unwrap();
    assert_eq!(owner.read_document(image.id, false).await.unwrap(), b"pixels");

    // the note's parent isn't the writer's, so neither is the folder's place
    let err = writer
        .create_attachments_folder(&other.id)
        .await
        .unwrap_err();
    assert_eq!(err.kind, LbErrKind::InsufficientPermission);
}

#[tokio::test]
async fn the_second_devices_folder_is_replaced_by_the_servers() {
    let a = test_core_with_attachments().await;
    let doc = a.create_at_path("/plan.md").await.unwrap();
    a.sync().await.unwrap();
    let b = test_core_from_with_attachments(&a).await;

    let first = a.create_attachments_folder(&doc.id).await.unwrap();
    let from_a = a
        .create_file("image.png", &first.id, FileType::Document)
        .await
        .unwrap();
    a.sync().await.unwrap();
    let second = b.create_attachments_folder(&doc.id).await.unwrap();
    let from_b = b
        .create_file("image.png", &second.id, FileType::Document)
        .await
        .unwrap();
    b.write_document(from_b.id, b"b's").await.unwrap();
    b.sync().await.unwrap();
    a.sync().await.unwrap();

    assert_eq!(b.get_attachments_folder(&doc.id).await.unwrap().unwrap().id, first.id);
    assert!(b.get_file_by_id(second.id).await.is_err());
    let from_b = b.get_file_by_id(from_b.id).await.unwrap();
    assert_eq!(from_b.parent, first.id);
    assert_eq!(from_b.name, "image 2.png");
    assert_eq!(b.get_file_by_id(from_a.id).await.unwrap().parent, first.id);
    assert_eq!(a.read_document(from_b.id, false).await.unwrap(), b"b's");
    assert_dbs_equal(&a, &b).await;
}

#[tokio::test]
async fn a_note_moved_under_a_folder_moved_into_its_attachments_stays_put() {
    let a = test_core_with_attachments().await;
    let doc = a.create_at_path("/plan.md").await.unwrap();
    let folder = a.create_attachments_folder(&doc.id).await.unwrap();
    let x = a.create_at_path("/x/").await.unwrap();
    a.sync().await.unwrap();
    let b = test_core_from_with_attachments(&a).await;

    b.move_file(&x.id, &folder.id).await.unwrap();
    b.sync().await.unwrap();
    a.move_file(&doc.id, &x.id).await.unwrap();
    a.sync().await.unwrap();
    b.sync().await.unwrap();

    // the note's move would put its folder inside itself, so it's undone like any move that
    // forms a cycle
    let root = a.root().await.unwrap().id;
    assert_eq!(a.get_file_by_id(doc.id).await.unwrap().parent, root);
    assert_eq!(a.get_file_by_id(folder.id).await.unwrap().parent, root);
    assert_eq!(a.get_file_by_id(x.id).await.unwrap().parent, folder.id);
    assert_dbs_equal(&a, &b).await;
}

#[tokio::test]
async fn deleting_the_document_deletes_its_folder() {
    let owner = test_core_with_attachments().await;
    let writer = test_core_with_attachments().await;
    let doc = owner.create_at_path("/plan.md").await.unwrap();
    let folder = owner.create_attachments_folder(&doc.id).await.unwrap();
    let image = owner
        .create_file("image.png", &folder.id, FileType::Document)
        .await
        .unwrap();
    owner.write_document(image.id, b"pixels").await.unwrap();
    owner.sync().await.unwrap();
    let writer_name = writer.get_account().unwrap().username.clone();
    owner
        .share_file(doc.id, &writer_name, ShareMode::Write)
        .await
        .unwrap();
    owner.sync().await.unwrap();
    writer.sync().await.unwrap();
    let link = writer
        .create_link_at_path("/plan.md", doc.id)
        .await
        .unwrap();
    writer.sync().await.unwrap();

    // a second device of the owner's, which never saw the folder, deletes the document
    let other = test_core_from(&owner).await;
    let late = owner
        .create_file("late.png", &folder.id, FileType::Document)
        .await
        .unwrap();
    owner.sync().await.unwrap();
    other.delete(&doc.id).await.unwrap();
    other.sync().await.unwrap();
    owner.sync().await.unwrap();
    writer.sync().await.unwrap();

    for id in [folder.id, image.id, late.id] {
        assert!(owner.get_file_by_id(id).await.is_err(), "{id}");
        assert!(writer.get_file_by_id(id).await.is_err(), "{id}");
    }
    assert!(writer.get_file_by_id(link.id).await.is_err());

    // the document's gone, so nothing would get a new folder; a live document can, once its
    // folder is deleted
    let doc = owner.create_at_path("/next.md").await.unwrap();
    let folder = owner.create_attachments_folder(&doc.id).await.unwrap();
    owner.delete(&folder.id).await.unwrap();
    owner.create_attachments_folder(&doc.id).await.unwrap();
    owner.sync().await.unwrap();
}

#[tokio::test]
async fn hidden_from_listings_unless_asked_for() {
    let core = test_core_with_attachments().await;
    let doc = core.create_at_path("/notes/plan.md").await.unwrap();
    let folder = core.create_attachments_folder(&doc.id).await.unwrap();
    let image = core
        .create_at_path("/notes/plan-attachments/image.png")
        .await
        .unwrap();
    let hidden = [folder.id, image.id];

    let listed = core.list_metadatas().await.unwrap();
    assert!(listed.iter().all(|f| !hidden.contains(&f.id)));
    assert!(listed.iter().any(|f| f.id == doc.id));
    let children = core.get_children(&doc.parent).await.unwrap();
    assert_eq!(children.iter().map(|f| f.id).collect::<Vec<_>>(), [doc.id]);
    let paths = core.list_paths(None).await.unwrap();
    assert!(!paths.iter().any(|p| p.contains("plan-attachments")), "{paths:?}");

    // still read by id and path
    assert_eq!(core.get_file_by_id(image.id).await.unwrap().id, image.id);
    assert_eq!(
        core.get_by_path("/notes/plan-attachments/image.png")
            .await
            .unwrap()
            .id,
        image.id
    );

    core.sync().await.unwrap();
    let listing =
        test_core_from_with(&core, Config { list_attachments: true, ..test_config() }).await;
    listing.sync().await.unwrap();
    let listed = listing.list_metadatas().await.unwrap();
    assert!(hidden.iter().all(|id| listed.iter().any(|f| f.id == *id)));
}

#[tokio::test]
async fn an_export_takes_each_notes_attachments() {
    let core = test_core_with_attachments().await;
    let notes = core.create_at_path("/notes/").await.unwrap();
    let doc = core.create_at_path("/notes/plan.md").await.unwrap();
    let folder = core.create_attachments_folder(&doc.id).await.unwrap();
    let image = core
        .create_file("image.png", &folder.id, FileType::Document)
        .await
        .unwrap();
    core.write_document(image.id, b"pixels").await.unwrap();

    let dest = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
    std::fs::create_dir_all(&dest).unwrap();
    core.export_file::<fn(_)>(notes.id, dest.clone(), false, &None)
        .await
        .unwrap();

    assert!(dest.join("notes/plan.md").exists());
    assert_eq!(std::fs::read(dest.join("notes/plan-attachments/image.png")).unwrap(), b"pixels");
    std::fs::remove_dir_all(dest).unwrap();
}

#[tokio::test]
async fn the_first_write_share_of_a_note_makes_its_folder() {
    let owner = test_core_with_attachments().await;
    let writer = test_core_with_attachments().await;
    let reader = test_core_with_account().await;
    let doc = owner.create_at_path("/notes/plan.md").await.unwrap();
    let reader_name = reader.get_account().unwrap().username.clone();
    owner
        .share_file(doc.id, &reader_name, ShareMode::Read)
        .await
        .unwrap();
    assert!(
        owner
            .get_attachments_folder(&doc.id)
            .await
            .unwrap()
            .is_none()
    );

    let writer_name = writer.get_account().unwrap().username.clone();
    owner
        .share_file(doc.id, &writer_name, ShareMode::Write)
        .await
        .unwrap();
    let folder = owner
        .get_attachments_folder(&doc.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(folder.name, "plan-attachments");
    owner.sync().await.unwrap();
    writer.sync().await.unwrap();
    assert_eq!(
        writer
            .get_attachments_folder(&doc.id)
            .await
            .unwrap()
            .unwrap()
            .id,
        folder.id
    );
}

#[tokio::test]
async fn notes_written_before_creation_turned_on_get_folders_on_sync() {
    let owner = test_core_with_account().await;
    let writer = test_core_with_account().await;
    let doc = owner.create_at_path("/notes/plan.md").await.unwrap();
    let writer_name = writer.get_account().unwrap().username.clone();
    owner
        .share_file(doc.id, &writer_name, ShareMode::Write)
        .await
        .unwrap();
    owner.sync().await.unwrap();
    assert!(
        owner
            .get_attachments_folder(&doc.id)
            .await
            .unwrap()
            .is_none()
    );

    let turned_on = test_core_from_with_attachments(&owner).await;
    turned_on.sync().await.unwrap();
    let folder = turned_on
        .get_attachments_folder(&doc.id)
        .await
        .unwrap()
        .unwrap();
    owner.sync().await.unwrap();
    writer.sync().await.unwrap();
    assert_eq!(
        owner
            .get_attachments_folder(&doc.id)
            .await
            .unwrap()
            .unwrap()
            .id,
        folder.id
    );
    assert_eq!(
        writer
            .get_attachments_folder(&doc.id)
            .await
            .unwrap()
            .unwrap()
            .id,
        folder.id
    );
}
