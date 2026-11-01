use lb_rs::io::network::ApiError;
use lb_rs::model::ValidationFailure;
use lb_rs::model::access_info::{UserAccessInfo, UserAccessMode};
use lb_rs::model::account::Account;
use lb_rs::model::api::{UpsertError, UpsertRequestV2};
use lb_rs::model::crypto::AESKey;
use lb_rs::model::errors::LbErrKind;
use lb_rs::model::file_like::FileLike;
use lb_rs::model::file_metadata::{FileDiff, FileType, Owner};
use lb_rs::model::meta::{AttachmentsOf, Meta};
use lb_rs::model::secret_filename::SecretFileName;
use lb_rs::model::signed_meta::SignedMeta;
use lb_rs::model::symkey;
use lb_rs::model::tree_like::TreeLike;
use lb_rs::service::keychain::Keychain;
use test_utils::*;
use uuid::Uuid;

fn to_v2(meta: Meta, attachments_of: Option<AttachmentsOf>) -> Meta {
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
    } = meta
    else {
        panic!("expected V1");
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
        content_size: None,
        attachments_of,
    }
}

fn child(
    account: &Account, parent: Uuid, parent_key: &AESKey, name: &str, file_type: FileType,
) -> (Meta, AESKey) {
    let key = symkey::generate_key();
    let meta = Meta::create(
        Uuid::new_v4(),
        key,
        &account.public_key(),
        parent,
        parent_key,
        name,
        file_type,
    )
    .unwrap();
    (meta, key)
}

fn root(account: &Account) -> (Meta, AESKey) {
    let root = Meta::create_root(account).unwrap();
    let key = vec![root.clone().sign_with(account).unwrap()]
        .to_lazy()
        .decrypt_key(root.id(), &Keychain::from(Some(account)))
        .unwrap();
    (root, key)
}

fn attachments(doc: &Meta, doc_key: &AESKey, folder: Meta, folder_key: &AESKey) -> Meta {
    let key = symkey::encrypt(doc_key, folder_key).unwrap();
    to_v2(folder, Some(AttachmentsOf { id: *doc.id(), key }))
}

fn share(meta: &mut Meta, key: &AESKey, by: &Account, with: &Account, mode: UserAccessMode) {
    meta.user_access_keys_mut().push(
        UserAccessInfo::encrypt(by, &by.public_key(), &with.public_key(), key, mode).unwrap(),
    );
}

fn reparent(meta: &mut Meta, key: &AESKey, parent: &Meta, parent_key: &AESKey, name: &str) {
    meta.set_parent(*parent.id());
    meta.set_folder_access_keys(symkey::encrypt(parent_key, key).unwrap());
    meta.set_name(SecretFileName::from_str(name, key, parent_key).unwrap());
}

/// Alice's plan.md, its plan.assets folder beside it, and chart.png inside, shared with bob.
struct Plan {
    alice: Account,
    bob: Account,
    root: Meta,
    root_key: AESKey,
    doc: Meta,
    doc_key: AESKey,
    folder: Meta,
    image: Meta,
}

fn plan(mode: UserAccessMode) -> Plan {
    let alice = Account::new(random_name(), url());
    let bob = Account::new(random_name(), url());
    let (root, root_key) = root(&alice);
    let (mut doc, doc_key) = child(&alice, *root.id(), &root_key, "plan.md", FileType::Document);
    share(&mut doc, &doc_key, &alice, &bob, mode);
    let (folder, folder_key) =
        child(&alice, *root.id(), &root_key, "plan.assets", FileType::Folder);
    let folder = attachments(&doc, &doc_key, folder, &folder_key);
    let (image, _) = child(&alice, *folder.id(), &folder_key, "chart.png", FileType::Document);
    Plan { alice, bob, root, root_key, doc, doc_key, folder, image }
}

fn signed(files: &[&Meta], by: &Account) -> Vec<SignedMeta> {
    files
        .iter()
        .map(|meta| (*meta).clone().sign_with(by).unwrap())
        .collect()
}

#[test]
fn v1_keeps_its_bincode_tag() {
    let alice = Account::new(random_name(), url());
    let v1 = Meta::create_root(&alice).unwrap();
    let v2 = to_v2(v1.clone(), None);

    // stored V1 metadata decodes because V1 keeps the first variant tag
    assert_eq!(bincode::serialize(&v1).unwrap()[..4], [0, 0, 0, 0]);
    assert_eq!(bincode::serialize(&v2).unwrap()[..4], [1, 0, 0, 0]);
}

#[test]
fn a_change_to_a_v2_field_is_a_change() {
    let plan = plan(UserAccessMode::Read);
    let mut moved = plan.folder.clone();
    let Meta::V2 { attachments_of, .. } = &mut moved else { panic!("expected V2") };
    attachments_of.as_mut().unwrap().id = Uuid::new_v4();

    // sync drops remote changes equal to what it has, so this must not be equal
    assert_ne!(plan.folder, moved);
}

#[tokio::test]
async fn a_recipient_reads_attachments_through_the_document() {
    let plan = plan(UserAccessMode::Read);
    let bob = Keychain::from(Some(&plan.bob));

    // bob is sent the document, its attachments folder, and what's in it, but not alice's root
    let bob_view = signed(&[&plan.doc, &plan.folder, &plan.image], &plan.alice);

    let mut tree = bob_view.clone().to_lazy();
    assert_eq!(tree.name(plan.folder.id(), &bob).unwrap(), "plan.assets");
    assert_eq!(tree.name(plan.image.id(), &bob).unwrap(), "chart.png");
    // until clients show attachments, listings leave them out
    assert!(tree.is_invisible_id(*plan.folder.id()).unwrap());

    bob_view
        .to_lazy()
        .stage(None)
        .validate(Owner(plan.bob.public_key()))
        .unwrap();
}

/// plan.md sits in a folder shared with bob; its attachments sit outside it, where bob may see
/// them read-only through another share or not at all.
fn work(plan: &Plan, mode: UserAccessMode, archive: Option<UserAccessMode>) -> Vec<SignedMeta> {
    let (mut work, work_key) =
        child(&plan.alice, *plan.root.id(), &plan.root_key, "work", FileType::Folder);
    share(&mut work, &work_key, &plan.alice, &plan.bob, mode);
    let mut doc = plan.doc.clone();
    reparent(&mut doc, &plan.doc_key, &work, &work_key, "plan.md");
    let mut files = vec![work, doc];
    if let Some(archive_mode) = archive {
        let folder_key = signed(&[&plan.root, &plan.folder], &plan.alice)
            .to_lazy()
            .decrypt_key(plan.folder.id(), &Keychain::from(Some(&plan.alice)))
            .unwrap();
        let (mut archive, archive_key) =
            child(&plan.alice, *plan.root.id(), &plan.root_key, "archive", FileType::Folder);
        share(&mut archive, &archive_key, &plan.alice, &plan.bob, archive_mode);
        let mut folder = plan.folder.clone();
        reparent(&mut folder, &folder_key, &archive, &archive_key, "plan.assets");
        files.extend([archive, folder]);
    } else {
        files.push(plan.folder.clone());
    }
    files.push(plan.image.clone());
    signed(&files.iter().collect::<Vec<_>>(), &plan.alice)
}

#[tokio::test]
async fn a_writer_of_a_document_can_change_its_attachments() {
    let plan = plan(UserAccessMode::Read);
    let bob = Keychain::from(Some(&plan.bob));
    for archive in [None, Some(UserAccessMode::Read)] {
        let mut tree = work(&plan, UserAccessMode::Write, archive)
            .to_lazy()
            .stage(vec![]);

        // the folder grants what the document grants
        tree.rename(plan.image.id(), "chart-2.png", &bob).unwrap();
        tree.delete(plan.doc.id(), &bob).unwrap();

        assert!(tree.find(plan.folder.id()).unwrap().explicitly_deleted());
    }
}

#[tokio::test]
async fn a_reader_of_a_document_cannot_change_its_attachments() {
    let plan = plan(UserAccessMode::Read);
    let bob = Keychain::from(Some(&plan.bob));
    let mut tree = work(&plan, UserAccessMode::Read, None)
        .to_lazy()
        .stage(vec![]);

    let rename = tree.rename(plan.image.id(), "chart-2.png", &bob);

    assert_eq!(rename.map_err(|err| err.kind), Err(LbErrKind::InsufficientPermission));
}

#[tokio::test]
async fn deleting_a_document_deletes_attachments_of_attachments() {
    // plan.assets holds notes.md, whose own attachments sit at the root
    let plan = plan(UserAccessMode::Read);
    let alice = Keychain::from(Some(&plan.alice));
    let folder_key = signed(&[&plan.root, &plan.folder], &plan.alice)
        .to_lazy()
        .decrypt_key(plan.folder.id(), &alice)
        .unwrap();
    let (notes, notes_key) =
        child(&plan.alice, *plan.folder.id(), &folder_key, "notes.md", FileType::Document);
    let (notes_folder, key) =
        child(&plan.alice, *plan.root.id(), &plan.root_key, "notes.assets", FileType::Folder);
    let notes_folder = attachments(&notes, &notes_key, notes_folder, &key);
    let mut tree = signed(
        &[&plan.root, &plan.doc, &plan.folder, &plan.image, &notes, &notes_folder],
        &plan.alice,
    )
    .to_lazy()
    .stage(vec![]);

    tree.delete(plan.doc.id(), &alice).unwrap();

    assert!(tree.find(plan.folder.id()).unwrap().explicitly_deleted());
    assert!(tree.find(notes_folder.id()).unwrap().explicitly_deleted());
}

#[tokio::test]
async fn validation_refuses_attachments_that_break_the_rules() {
    let plan = plan(UserAccessMode::Read);
    let validate = |files: &[&Meta]| {
        signed(files, &plan.alice)
            .to_lazy()
            .stage(None)
            .validate(Owner(plan.alice.public_key()))
            .map_err(|err| err.kind)
    };

    // a second attachments folder for the same document
    let (second, key) =
        child(&plan.alice, *plan.root.id(), &plan.root_key, "more", FileType::Folder);
    let second = attachments(&plan.doc, &plan.doc_key, second, &key);
    assert_eq!(
        validate(&[&plan.root, &plan.doc, &plan.folder, &second]),
        Err(LbErrKind::Validation(ValidationFailure::DuplicateAttachments {
            document: *plan.doc.id()
        }))
    );

    // attachments of a folder
    let (of_folder, key) =
        child(&plan.alice, *plan.root.id(), &plan.root_key, "assets", FileType::Folder);
    let of_folder = attachments(&plan.folder, &plan.doc_key, of_folder, &key);
    assert_eq!(
        validate(&[&plan.root, &plan.folder, &of_folder]),
        Err(LbErrKind::Validation(ValidationFailure::InvalidAttachmentsFolder(*of_folder.id())))
    );
}

#[tokio::test]
async fn a_document_inside_its_own_attachments_fails_instead_of_looping() {
    let alice = Account::new(random_name(), url());
    let bob = Account::new(random_name(), url());

    let doc_key = symkey::generate_key();
    let (folder, folder_key) =
        child(&alice, Uuid::new_v4(), &symkey::generate_key(), "plan.assets", FileType::Folder);
    let doc = Meta::create(
        Uuid::new_v4(),
        doc_key,
        &alice.public_key(),
        *folder.id(),
        &folder_key,
        "plan.md",
        FileType::Document,
    )
    .unwrap();
    let folder = attachments(&doc, &doc_key, folder, &folder_key);

    let doc_id = *doc.id();
    let mut tree = signed(&[&doc, &folder], &alice).to_lazy();
    assert!(
        tree.decrypt_key(&doc_id, &Keychain::from(Some(&bob)))
            .is_err()
    );
}

#[tokio::test]
async fn writing_a_v2_document_records_its_content_size() {
    let core = test_core_with_account().await;
    let id = core.create_at_path("plan.md").await.unwrap().id;
    {
        let mut tx = core.begin_tx().await;
        let db = tx.db();
        let v1 = db.local_metadata.get(&id).unwrap().clone();
        let v2 = to_v2(v1.timestamped_value.value, None)
            .sign(&core.keychain)
            .unwrap();
        db.local_metadata.insert(id, v2).unwrap();
        tx.end();
    }

    core.write_document(id, b"hello").await.unwrap();

    let mut tx = core.begin_tx().await;
    let db = tx.db();
    let mut tree = (&db.base_metadata).to_staged(&db.local_metadata).to_lazy();
    let key = tree.decrypt_key(&id, &core.keychain).unwrap();
    let content_size = tree
        .find(&id)
        .unwrap()
        .timestamped_value
        .value
        .content_size();
    assert_eq!(symkey::decrypt(&key, content_size.unwrap()).unwrap(), 5u64);
}

#[tokio::test]
async fn the_server_refuses_v2_until_the_forced_upgrade() {
    let core = test_core_with_account().await;
    let account = core.get_account().unwrap();
    let id = core.create_at_path("test.md").await.unwrap().id;
    core.sync().await.unwrap();
    let old = core
        .begin_tx()
        .await
        .db()
        .base_metadata
        .get(&id)
        .unwrap()
        .clone();
    let new = to_v2(old.timestamped_value.value.clone(), None)
        .sign_with(account)
        .unwrap();

    let result = core
        .client
        .request(account, UpsertRequestV2 { updates: vec![FileDiff::edit(old, new)] })
        .await;
    assert_matches!(result, Err(ApiError::<UpsertError>::Endpoint(UpsertError::DiffMalformed)));
}
