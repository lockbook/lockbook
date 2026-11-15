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
    assert_eq!(err.kind, LbErrKind::AttachmentsFoldersOff);
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
    assert_eq!(folder.name, "trip-attachments");
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
async fn sync_renames_it_when_its_document_is_renamed_for_a_conflict() {
    let a = test_core_with_attachments().await;
    let first = a.create_at_path("/a.md").await.unwrap();
    let second = a.create_at_path("/c.md").await.unwrap();
    a.create_attachments_folder(&first.id).await.unwrap();
    let folder = a.create_attachments_folder(&second.id).await.unwrap();
    a.sync().await.unwrap();
    let b = test_core_from(&a).await;

    a.rename_file(&first.id, "b.md").await.unwrap();
    a.sync().await.unwrap();
    b.rename_file(&second.id, "b.md").await.unwrap();
    b.sync().await.unwrap();
    a.sync().await.unwrap();

    assert_eq!(b.get_file_by_id(second.id).await.unwrap().name, "b 2.md");
    assert_eq!(b.get_file_by_id(folder.id).await.unwrap().name, "b 2-attachments");
    assert_dbs_equal(&a, &b).await;
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
async fn a_note_shared_alone_has_its_folder_beside_the_link() {
    let owner = test_core_with_attachments().await;
    let reader = test_core_with_account().await;
    let notes = owner.create_at_path("/notes/").await.unwrap();
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
    // the folder around them is shared too, but the reader leaves that share pending
    owner
        .share_file(notes.id, &reader_name, ShareMode::Read)
        .await
        .unwrap();
    owner.sync().await.unwrap();
    reader.sync().await.unwrap();

    let link = reader
        .create_link_at_path("/shared/renamed.md", doc.id)
        .await
        .unwrap();

    let folder_path = reader.get_path_by_id(folder.id).await.unwrap();
    assert_eq!(folder_path, "/shared/plan-attachments/");
    assert_eq!(
        reader.get_path_by_id(image.id).await.unwrap(),
        "/shared/plan-attachments/image.png"
    );
    assert_eq!(reader.get_by_path(&folder_path).await.unwrap().id, folder.id);
    assert_eq!(
        reader
            .get_by_path("/shared/plan-attachments/image.png")
            .await
            .unwrap()
            .id,
        image.id
    );
    assert_eq!(reader.get_file_by_id(folder.id).await.unwrap().parent, link.parent);
    let listed: Vec<_> = reader
        .list_metadatas()
        .await
        .unwrap()
        .into_iter()
        .map(|f| f.id)
        .collect();
    assert!(listed.contains(&folder.id) && listed.contains(&image.id));
    assert!(!listed.contains(&notes.id));
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
async fn a_writer_of_a_note_shared_alone_adds_images() {
    let owner = test_core_with_attachments().await;
    let writer = test_core_with_attachments().await;
    let doc = owner.create_at_path("/notes/plan.md").await.unwrap();
    let other = owner.create_at_path("/notes/other.md").await.unwrap();
    let folder = owner.create_attachments_folder(&doc.id).await.unwrap();
    let writer_name = writer.get_account().unwrap().username.clone();
    for id in [doc.id, other.id] {
        owner
            .share_file(id, &writer_name, ShareMode::Write)
            .await
            .unwrap();
    }
    owner.sync().await.unwrap();
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
