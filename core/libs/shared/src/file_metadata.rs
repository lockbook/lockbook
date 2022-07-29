use libsecp256k1::PublicKey;
use std::clone::Clone;
use std::collections::HashMap;
use std::fmt;
use std::fmt::Display;
use std::hash::{Hash, Hasher};
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::access_info::{EncryptedFolderAccessKey, UserAccessInfo};
use crate::account::{Account, Username};
use crate::clock::get_time;
use crate::crypto::{AESKey, ECSigned};
use crate::file_like::FileLike;
use crate::secret_filename::SecretFileName;
use crate::signed_file::SignedFile;
use crate::{pubkey, symkey, SharedResult};

pub type DocumentHmac = [u8; 32];

#[derive(Serialize, Deserialize, PartialEq, Clone, Debug)]
pub struct FileMetadata {
    pub id: Uuid,
    pub file_type: FileType,
    pub parent: Uuid,
    pub name: SecretFileName,
    pub owner: Owner,
    pub is_deleted: bool,
    pub document_hmac: Option<DocumentHmac>,
    pub user_access_keys: HashMap<Username, UserAccessInfo>,
    pub folder_access_keys: EncryptedFolderAccessKey,
}

impl FileMetadata {
    pub fn create_root(account: &Account) -> SharedResult<Self> {
        let id = Uuid::new_v4();
        let key = symkey::generate_key();
        let pub_key = account.public_key();

        Ok(FileMetadata {
            id,
            file_type: FileType::Document,
            parent: id,
            name: SecretFileName::from_str(&account.username, &key)?,
            owner: Owner(pub_key),
            is_deleted: false,
            document_hmac: None,
            user_access_keys: UserAccessInfo::encrypt(account, &pub_key, &key)?,
            folder_access_keys: symkey::encrypt(&key, &key)?,
        })
    }

    pub fn create(
        pub_key: &PublicKey, parent: Uuid, parent_key: &AESKey, name: &str, file_type: FileType,
    ) -> SharedResult<Self> {
        let id = Uuid::new_v4();
        let key = symkey::generate_key();

        Ok(FileMetadata {
            id,
            file_type,
            parent,
            name: SecretFileName::from_str(name, parent_key)?,
            owner: Owner(*pub_key),
            is_deleted: false,
            document_hmac: None,
            user_access_keys: Default::default(),
            folder_access_keys: symkey::encrypt(parent_key, &key)?,
        })
    }

    pub fn sign(self, account: &Account) -> SharedResult<SignedFile> {
        pubkey::sign(&account.private_key, self, get_time)
    }
}

impl Display for FileMetadata {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.display())
    }
}

#[derive(Serialize, Deserialize, Eq, Clone, Copy, Debug)]
pub struct Owner(pub PublicKey);

impl Hash for Owner {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.serialize().hash(state)
    }
}

impl PartialEq for Owner {
    fn eq(&self, other: &Self) -> bool {
        self.0.serialize() == other.0.serialize()
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Deserialize, Serialize, Copy)]
pub enum FileType {
    Document,
    Folder,
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

#[derive(Serialize, Deserialize, PartialEq, Clone, Debug)]
pub struct FileDiff {
    pub old: Option<SignedFile>,
    pub new: SignedFile,
}

#[derive(PartialEq)]
pub enum Diff {
    New,
    Id,
    Parent,
    Name,
    Owner,
    Deleted,
    Hmac,
    UserKeys,
    FolderKeys,
}

impl FileDiff {
    pub fn diff(&self) -> Vec<Diff> {
        let new = &self.new;
        use Diff::*;
        match &self.old {
            None => vec![New],
            Some(old) => {
                let mut changes = vec![];
                if old.id() != new.id() {
                    changes.push(Id)
                }

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

                if old.user_access_keys() != new.user_access_keys() {
                    changes.push(UserKeys);
                }

                if old.folder_access_keys() != new.folder_access_keys() {
                    changes.push(FolderKeys);
                }
                changes
            }
        }
    }

    pub fn new(new: &ECSigned<FileMetadata>) -> Self {
        let old = None;
        let new = new.clone();
        Self { old, new }
    }

    pub fn edit(old: &ECSigned<FileMetadata>, new: &ECSigned<FileMetadata>) -> Self {
        let old = Some(old.clone());
        let new = new.clone();
        Self { old, new }
    }
}
