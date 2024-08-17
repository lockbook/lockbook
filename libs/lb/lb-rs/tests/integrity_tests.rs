use lb_rs::TestRepoError::*;
use lb_rs::Warning::*;
use lb_rs::shared::file_like::FileLike;
use lb_rs::shared::file_metadata::FileType::Document;
use lb_rs::shared::secret_filename::SecretFileName;
use lb_rs::shared::tree_like::TreeLike;
use rand::Rng;
use test_utils::*;

#[test]
fn test_integrity_no_problems() {
    let core = test_core_with_account();
    core.validate().unwrap();
}

#[test]
fn test_integrity_no_problems_but_more_complicated() {
    let core = test_core_with_account();
    core.create_at_path("test.md").unwrap();
    core.validate().unwrap();
}

#[test]
fn test_no_account() {
    let core = test_core();
    assert_matches!(core.validate(), Err(NoAccount));
}

#[test]
fn test_no_root() {
    let core = test_core_with_account();
    core.in_tx(|s| {
        s.db.base_metadata.clear().unwrap();
        s.db.root.clear().unwrap();
        Ok(())
    })
    .unwrap();
    assert_matches!(core.validate(), Err(NoRootFolder));
}

#[test]
fn test_orphaned_children() {
    let core = test_core_with_account();

    core.create_at_path("folder1/folder2/document1.md").unwrap();
    core.validate().unwrap();

    let parent = core.get_by_path("folder1").unwrap().id;
    core.in_tx(|s| {
        s.db.local_metadata.remove(&parent).unwrap();
        Ok(())
    })
    .unwrap();
    assert_matches!(core.validate(), Err(FileOrphaned(_)));
}

#[test]
fn test_invalid_file_name_slash() {
    let core = test_core_with_account();
    let doc = core.create_at_path("document1.md").unwrap();
    core.in_tx(|s| {
        let mut tree = s.db.base_metadata.stage(&mut s.db.local_metadata).to_lazy();
        let key = tree
            .decrypt_key(&doc.id, s.db.account.get().unwrap())
            .unwrap();
        let parent = tree
            .decrypt_key(&doc.parent, s.db.account.get().unwrap())
            .unwrap();
        let new_name = SecretFileName::from_str("te/st", &key, &parent).unwrap();
        let mut doc = tree.find(&doc.id).unwrap().clone();
        doc.timestamped_value.value.name = new_name;
        tree.stage(Some(doc)).promote().unwrap();
        Ok(())
    })
    .unwrap();

    assert_matches!(core.validate(), Err(FileNameContainsSlash(_)));
}

#[test]
fn empty_filename() {
    let core = test_core_with_account();
    let doc = core.create_at_path("document1.md").unwrap();
    core.in_tx(|s| {
        let mut tree = s.db.base_metadata.stage(&mut s.db.local_metadata).to_lazy();
        let key = tree
            .decrypt_key(&doc.id, s.db.account.get().unwrap())
            .unwrap();
        let parent = tree
            .decrypt_key(&doc.parent, s.db.account.get().unwrap())
            .unwrap();
        let new_name = SecretFileName::from_str("", &key, &parent).unwrap();
        let mut doc = tree.find(&doc.id).unwrap().clone();
        doc.timestamped_value.value.name = new_name;
        tree.stage(Some(doc)).promote().unwrap();
        Ok(())
    })
    .unwrap();

    assert_matches!(core.validate(), Err(FileNameEmpty(_)));
}

#[test]
fn test_cycle() {
    let core = test_core_with_account();
    core.create_at_path("folder1/folder2/document1.md").unwrap();
    let parent = core.get_by_path("folder1").unwrap().id;
    core.in_tx(|s| {
        s.db.local_metadata.get().get(&parent).unwrap();
        Ok(())
    })
    .unwrap();
    let mut parent = core
        .in_tx(|s| Ok(s.db.local_metadata.get().get(&parent).unwrap().clone()))
        .unwrap();
    let child = core.get_by_path("folder1/folder2").unwrap();
    parent.timestamped_value.value.parent = child.id;
    core.in_tx(|s| {
        s.db.local_metadata.insert(*parent.id(), parent).unwrap();
        Ok(())
    })
    .unwrap();
    assert_matches!(core.validate(), Err(CycleDetected(_)));
}

#[test]
fn test_documents_treated_as_folders() {
    let core = test_core_with_account();
    core.create_at_path("folder1/folder2/document1.md").unwrap();
    let parent = core.get_by_path("folder1").unwrap();
    let mut parent = core
        .in_tx(|s| Ok(s.db.local_metadata.get().get(&parent.id).unwrap().clone()))
        .unwrap();
    parent.timestamped_value.value.file_type = Document;
    core.in_tx(|s| {
        s.db.local_metadata.insert(*parent.id(), parent).unwrap();
        Ok(())
    })
    .unwrap();
    assert_matches!(core.validate(), Err(DocumentTreatedAsFolder(_)));
}

#[test]
fn test_name_conflict() {
    let core = test_core_with_account();
    let doc = core.create_at_path("document1.md").unwrap();
    core.create_at_path("document2.md").unwrap();
    core.in_tx(|s| {
        let mut tree = s.db.base_metadata.stage(&mut s.db.local_metadata).to_lazy();
        let key = tree
            .decrypt_key(&doc.id, s.db.account.get().unwrap())
            .unwrap();
        let parent = tree
            .decrypt_key(&doc.parent, s.db.account.get().unwrap())
            .unwrap();
        let new_name = SecretFileName::from_str("document2.md", &key, &parent).unwrap();
        let mut doc = tree.find(&doc.id).unwrap().clone();
        doc.timestamped_value.value.name = new_name;
        tree.stage(Some(doc)).promote().unwrap();
        Ok(())
    })
    .unwrap();
    assert_matches!(core.validate(), Err(PathConflict(_)));
}

#[test]
fn test_empty_file() {
    let core = test_core_with_account();
    let doc = core.create_at_path("document.txt").unwrap();
    core.write_document(doc.id, &[]).unwrap();
    let warnings = core.validate();

    assert_matches!(warnings.as_ref().map(|w| &w[..]), Ok([EmptyFile(_)]));
}

#[test]
fn test_invalid_utf8() {
    let core = test_core_with_account();
    let doc = core.create_at_path("document.txt").unwrap();
    core.write_document(doc.id, rand::thread_rng().gen::<[u8; 32]>().as_ref())
        .unwrap();
    let warnings = core.validate();
    assert_matches!(warnings.as_ref().map(|w| &w[..]), Ok([InvalidUTF8(_)]));
}

#[test]
fn test_invalid_utf8_ignores_non_utf_file_extensions() {
    let core = test_core_with_account();
    let doc = core.create_at_path("document.png").unwrap();
    core.write_document(doc.id, rand::thread_rng().gen::<[u8; 32]>().as_ref())
        .unwrap();
    let warnings = core.validate();
    assert_matches!(warnings.as_ref().map(|w| &w[..]), Ok([]));
}

#[test]
fn test_invalid_drawing() {
    let core = test_core_with_account();
    let doc = core.create_at_path("document.draw").unwrap();
    core.write_document(doc.id, rand::thread_rng().gen::<[u8; 32]>().as_ref())
        .unwrap();
    let warnings = core.validate();
    assert_matches!(warnings.as_ref().map(|w| &w[..]), Ok([UnreadableDrawing(_)]));
}
