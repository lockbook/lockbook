use lb_rs::model::access_info::{UserAccessInfo, UserAccessMode};
use lb_rs::model::account::Account;
use lb_rs::model::file_like::FileLike;
use lb_rs::model::file_metadata::{FileType, Owner};
use lb_rs::model::{meta::Meta, symkey};
use lb_rs::service::share::SharingContact;
use test_utils::{local, test_core};
use uuid::Uuid;

// Metadata-only fixtures exercise the actual query without a server or document bodies.
fn node(owner: &Account, parent: Uuid, kind: FileType) -> Meta {
    Meta::create(
        Uuid::new_v4(),
        symkey::generate_key(),
        &owner.public_key(),
        parent,
        &symkey::generate_key(),
        "file",
        kind,
    )
    .unwrap()
}

fn share(file: &mut Meta, by: &Account, with: &Account) {
    file.user_access_keys_mut().push(
        UserAccessInfo::encrypt(
            by,
            &by.public_key(),
            &with.public_key(),
            &symkey::generate_key(),
            UserAccessMode::Read,
        )
        .unwrap(),
    );
}

fn contact(name: &str, outgoing: u64, incoming: u64) -> SharingContact {
    SharingContact {
        username: name.into(),
        outgoing_file_count: outgoing,
        incoming_file_count: incoming,
        total_file_count: outgoing + incoming,
    }
}

#[tokio::test]
async fn account_contacts_count_documents_in_both_directions() {
    let lb = test_core().await;
    let core = local(&lb);
    let adam = Account::new("adam".into(), "unused".into());
    let alice = Account::new("alice".into(), "unused".into());
    let bob = Account::new("bob".into(), "unused".into());
    let carol = Account::new("carol".into(), "unused".into());
    let dave = Account::new("dave".into(), "unused".into());
    core.keychain.cache_account(adam.clone()).await.unwrap();
    let root = Meta::create_root(&adam).unwrap();
    let mut folder = node(&adam, *root.id(), FileType::Folder);
    share(&mut folder, &adam, &alice);
    let mut first = node(&adam, *folder.id(), FileType::Document);
    share(&mut first, &adam, &alice); // Direct + inherited counts once.
    share(&mut first, &adam, &dave);
    first.user_access_keys_mut().last_mut().unwrap().deleted = true;
    let nested = node(&adam, *folder.id(), FileType::Folder);
    let second = node(&adam, *nested.id(), FileType::Document);
    let mut deleted = node(&adam, *folder.id(), FileType::Document);
    deleted.set_deleted(true);
    let mut outgoing = node(&adam, *root.id(), FileType::Document);
    share(&mut outgoing, &adam, &bob);
    let mut incoming = node(&bob, Uuid::new_v4(), FileType::Folder);
    share(&mut incoming, &bob, &adam);
    share(&mut incoming, &bob, &carol); // A third party is not Adam's contact.
    let received1 = node(&bob, *incoming.id(), FileType::Document);
    let received2 = node(&bob, *incoming.id(), FileType::Document);
    // Placing an accepted link under Alice's folder must not give Alice credit for Bob's files.
    let mut link = node(&adam, *folder.id(), FileType::Link { target: *incoming.id() });
    let mut pending = node(&carol, Uuid::new_v4(), FileType::Document);
    share(&mut pending, &carol, &adam);

    let mut tx = core.begin_tx().await;
    let db = tx.db();
    db.account.insert(adam.clone()).unwrap();
    db.root.insert(*root.id()).unwrap();
    for account in [&adam, &alice, &bob, &carol, &dave] {
        db.pub_key_lookup
            .insert(Owner(account.public_key()), account.username.clone())
            .unwrap();
    }
    for (owner, files) in [
        (&adam, vec![root, folder.clone(), first, nested, second, deleted, outgoing, link.clone()]),
        (&bob, vec![incoming.clone(), received1, received2]),
        (&carol, vec![pending]),
    ] {
        for file in files {
            db.base_metadata
                .insert(*file.id(), file.sign_with(owner).unwrap())
                .unwrap();
        }
    }
    tx.end();

    assert_eq!(
        lb.get_sharing_contacts().await.unwrap(),
        vec![contact("bob", 1, 2), contact("alice", 2, 0)]
    );

    // A deleted foreign share root must not count even while its accepted link remains.
    incoming.set_deleted(true);
    let mut tx = core.begin_tx().await;
    tx.db()
        .local_metadata
        .insert(*incoming.id(), incoming.clone().sign_with(&bob).unwrap())
        .unwrap();
    tx.end();
    assert_eq!(
        lb.get_sharing_contacts().await.unwrap(),
        vec![contact("alice", 2, 0), contact("bob", 1, 0)]
    );

    incoming.set_deleted(false);
    link.set_deleted(true); // Removing acceptance makes the incoming tree pending again.
    folder.user_access_keys_mut()[0].deleted = true;
    let mut tx = core.begin_tx().await;
    for (owner, file) in [(&bob, incoming), (&adam, link), (&adam, folder)] {
        tx.db()
            .local_metadata
            .insert(*file.id(), file.sign_with(owner).unwrap())
            .unwrap();
    }
    tx.end();
    assert_eq!(
        lb.get_sharing_contacts().await.unwrap(),
        vec![contact("alice", 1, 0), contact("bob", 1, 0)]
    );
}

#[tokio::test]
async fn contacts_require_an_account() {
    let lb = test_core().await;
    assert_eq!(
        lb.get_sharing_contacts().await.unwrap_err().kind,
        lb_rs::model::errors::LbErrKind::AccountNonexistent
    );
}
