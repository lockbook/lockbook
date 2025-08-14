use std::fmt::{self, Debug, Formatter};
use std::hash::{Hash, Hasher};
use std::str::FromStr;

use libsecp256k1::PublicKey;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::access_info::{EncryptedFolderAccessKey, UserAccessInfo, UserAccessMode};
use super::account::Account;
use super::clock::get_time;
use super::errors::LbResult;
use crate::model::crypto::AESKey;
use crate::model::file_like::FileLike;
use crate::model::secret_filename::SecretFileName;
use crate::model::signed_file::SignedFile;
use crate::model::{pubkey, symkey};
use crate::service::keychain::Keychain;

pub type DocumentHmac = [u8; 32];

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FileMetadata {
    pub id: Uuid,
    pub file_type: FileType,
    pub parent: Uuid,
    pub name: SecretFileName,
    pub owner: Owner,
    pub is_deleted: bool,
    pub document_hmac: Option<DocumentHmac>,
    pub user_access_keys: Vec<UserAccessInfo>,
    pub folder_access_key: EncryptedFolderAccessKey,
}

impl FileMetadata {
    pub fn create_root(account: &Account) -> LbResult<Self> {
        let id = Uuid::new_v4();
        let key = symkey::generate_key();
        let pub_key = account.public_key();

        Ok(FileMetadata {
            id,
            file_type: FileType::Folder,
            parent: id,
            name: SecretFileName::from_str(&account.username, &key, &key)?,
            owner: Owner(pub_key),
            is_deleted: false,
            document_hmac: None,
            user_access_keys: vec![UserAccessInfo::encrypt(
                account,
                &pub_key,
                &pub_key,
                &key,
                UserAccessMode::Write,
            )?],
            folder_access_key: symkey::encrypt(&key, &key)?,
        })
    }

    pub fn create(
        id: Uuid, key: AESKey, owner: &PublicKey, parent: Uuid, parent_key: &AESKey, name: &str,
        file_type: FileType,
    ) -> LbResult<Self> {
        Ok(FileMetadata {
            id,
            file_type,
            parent,
            name: SecretFileName::from_str(name, &key, parent_key)?,
            owner: Owner(*owner),
            is_deleted: false,
            document_hmac: None,
            user_access_keys: Default::default(),
            folder_access_key: symkey::encrypt(parent_key, &key)?,
        })
    }

    pub fn sign(self, keychain: &Keychain) -> LbResult<SignedFile> {
        pubkey::sign(&keychain.get_account()?.private_key, &keychain.get_pk()?, self, get_time)
    }

    pub fn sign_with(self, account: &Account) -> LbResult<SignedFile> {
        pubkey::sign(&account.private_key, &account.public_key(), self, get_time)
    }
}

// This is impl'd to avoid comparing encrypted values
impl PartialEq for FileMetadata {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.file_type == other.file_type
            && self.parent == other.parent
            && self.name == other.name
            && self.owner == other.owner
            && self.is_deleted == other.is_deleted
            && self.document_hmac == other.document_hmac
            && self.user_access_keys == other.user_access_keys
    }
}

impl fmt::Display for FileMetadata {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.display())
    }
}

#[derive(Serialize, Deserialize, Eq, PartialEq, Clone, Copy)]
pub struct Owner(pub PublicKey);

impl Hash for Owner {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.serialize().hash(state)
    }
}

impl Debug for Owner {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "pub_key:{}", base64::encode(self.0.serialize_compressed()))
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Deserialize, Serialize, Copy)]
pub enum FileType {
    Document,
    Folder,
    Link { target: Uuid },
}

impl FromStr for FileType {
    type Err = ();
    fn from_str(input: &str) -> Result<FileType, Self::Err> {
        match input {
            "Document" => Ok(FileType::Document),
            "Folder" => Ok(FileType::Folder),
            _ => Err(()),
        }
    }
}

impl fmt::Display for FileType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct FileDiff<F: FileLike> {
    pub old: Option<F>,
    pub new: F,
}

impl<F: FileLike> Debug for FileDiff<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut result = &mut f.debug_struct("FileDiff");
        result = result.field("id", self.id());
        for diff in self.diff() {
            result = match diff {
                Diff::New => result.field("is_new", &true),
                Diff::Parent => result.field("new_parent", &self.new.parent()),
                Diff::Name => result.field("new_name", &self.new.secret_name()),
                Diff::Owner => result.field("new_owner", &self.new.owner()),
                Diff::Deleted => result.field("new_deleted", &self.new.explicitly_deleted()),
                Diff::Hmac => result.field("new_hmac", &self.new.document_hmac()),
                Diff::UserKeys => result.field("new_user_keys", &true),
            };
        }
        result.finish()
    }
}

#[derive(PartialEq, Eq, Debug)]
pub enum Diff {
    New,
    Parent,
    Name,
    Owner,
    Deleted,
    Hmac,
    UserKeys,
}

impl<F: FileLike> FileDiff<F> {
    pub fn id(&self) -> &Uuid {
        self.new.id()
    }

    pub fn diff(&self) -> Vec<Diff> {
        let new = &self.new;
        use Diff::*;
        match &self.old {
            None => vec![New],
            Some(old) => {
                let mut changes = vec![];

                if old.parent() != new.parent() {
                    changes.push(Parent)
                }

                if old.secret_name() != new.secret_name() {
                    changes.push(Name)
                }

                if old.owner() != new.owner() {
                    changes.push(Owner)
                }

                if old.explicitly_deleted() != new.explicitly_deleted() {
                    changes.push(Deleted)
                }

                if old.document_hmac() != new.document_hmac() {
                    changes.push(Hmac);
                }

                if old.user_access_keys() != new.user_access_keys() {
                    changes.push(UserKeys);
                }

                changes
            }
        }
    }

    pub fn new(new: F) -> Self {
        let old = None;
        Self { old, new }
    }

    pub fn edit(old: F, new: F) -> Self {
        let old = Some(old);
        Self { old, new }
    }
}
