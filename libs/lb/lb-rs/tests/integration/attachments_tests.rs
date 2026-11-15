use lb_rs::model::ValidationFailure;
use lb_rs::model::errors::LbErrKind;
use lb_rs::model::file::ShareMode;
use lb_rs::model::file_like::FileLike;
use lb_rs::model::file_metadata::FileType;
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
        let of = file.attachments_of().map(|of| of.id);
        assert_eq!(of, (id == folder.id).then_some(doc.id), "{id}");
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
}

#[tokio::test]
async fn the_second_device_folds_its_folder_into_the_servers() {
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
