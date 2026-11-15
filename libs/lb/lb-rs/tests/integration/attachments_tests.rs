use lb_rs::model::ValidationFailure;
use lb_rs::model::errors::LbErrKind;
use lb_rs::model::file_like::FileLike;
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
