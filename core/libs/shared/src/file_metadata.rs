use libsecp256k1::PublicKey;
use std::clone::Clone;
use std::collections::HashMap;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::account::{Account, Username};
use crate::crypto::{AESKey, ECSigned, EncryptedFolderAccessKey, SecretFileName, UserAccessInfo};
use crate::tree::FileLike;

pub type EncryptedFiles = HashMap<Uuid, UnsignedFile>;
pub type DecryptedFiles = HashMap<Uuid, CoreFile>;

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

#[derive(Serialize, Deserialize, PartialEq, Clone)]
pub struct UnsignedFile {
    pub id: Uuid,
    pub file_type: FileType,
    pub parent: Uuid,
    pub name: SecretFileName,
    pub owner: Owner,
    pub is_deleted: bool,
    pub user_access_keys: HashMap<Username, UserAccessInfo>,
    pub folder_access_keys: EncryptedFolderAccessKey,
}

impl UnsignedFile {
    fn create_root(account: &Account) -> Result<Self, CoreError> {
        let id = Uuid::new_v4();
        let key = symkey::generate_key();
        let name = account.username.clone();
        Ok(UnsignedFile {
            id,
            file_type: FileType::Document,
            parent: id,
            name: encrypt_file_name(&name, &key)?,
            owner: Owner::from(account),
            is_deleted: false,
            user_access_keys: encrypt_user_access_keys(account, &key)?,
            folder_access_keys: encrypt_folder_access_keys(
                &target.decrypted_access_key,
                parent_key,
            )?,
        })
    }
}

#[derive(Serialize, Deserialize, PartialEq, Clone)]
pub struct SignedFile {
    pub file: ECSigned<UnsignedFile>,
    pub metadata_version: u64,
    pub content_version: u64,
}

impl FileLike for UnsignedFile {
    type Name = SecretFileName;

    fn id(&self) -> Uuid {
        self.id
    }
    fn file_type(&self) -> FileType {
        self.file_type
    }
    fn parent(&self) -> Uuid {
        self.parent
    }
    fn name(&self) -> Self::Name {
        self.name.clone()
    }
    fn owner(&self) -> Owner {
        self.owner.clone()
    }
    fn metadata_version(&self) -> u64 {
        self.metadata_version
    }
    fn content_version(&self) -> u64 {
        self.content_version
    }
    fn is_deleted(&self) -> bool {
        self.is_deleted
    }
    fn display(&self) -> String {
        match self.file_type() {
            FileType::Folder => format!("id: {}/", self.id),
            FileType::Document => format!("id: {}", self.id),
        }
    }
}

impl fmt::Display for UnsignedFile {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.display())
    }
}

impl fmt::Debug for UnsignedFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileMetadata")
            .field("id", &self.id)
            .field("file_type", &self.file_type)
            .field("parent", &self.parent)
            .field("metadata_version", &self.metadata_version)
            .field("content_version", &self.content_version)
            .field("deleted", &self.is_deleted)
            .finish()
    }
}

#[derive(Serialize, Deserialize, Eq, Clone, Debug)]
pub struct Owner(pub PublicKey);

impl From<&Account> for Owner {
    fn from(account: &Account) -> Self {
        Self(account.public_key())
    }
}

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

#[derive(Serialize, Deserialize, PartialEq, Eq, Hash, Clone)]
pub struct CoreFile {
    pub file: SignedFile,
    pub decrypted_name: String,
    pub decrypted_access_key: AESKey, // access key is the same whether it's decrypted for user or for folder
}

impl FileLike for CoreFile {
    type Name = String;

    fn id(&self) -> Uuid {
        self.id
    }
    fn file_type(&self) -> FileType {
        self.file_type
    }
    fn parent(&self) -> Uuid {
        self.parent
    }
    fn name(&self) -> Self::Name {
        self.decrypted_name.clone()
    }
    fn owner(&self) -> Owner {
        self.owner.clone()
    }
    fn metadata_version(&self) -> u64 {
        self.metadata_version
    }
    fn content_version(&self) -> u64 {
        self.content_version
    }
    fn is_deleted(&self) -> bool {
        self.deleted
    }
    fn display(&self) -> String {
        match self.file_type() {
            FileType::Folder => format!("{}/", self.decrypted_name),
            FileType::Document => self.decrypted_name.clone(),
        }
    }
}

impl fmt::Display for CoreFile {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.display())
    }
}

impl fmt::Debug for CoreFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DecryptedFileMetadata")
            .field("id", &self.id)
            .field("file_type", &self.file_type)
            .field("parent", &self.parent)
            .field("decrypted_name", &self.decrypted_name)
            .field("metadata_version", &self.metadata_version)
            .field("content_version", &self.content_version)
            .field("deleted", &self.deleted)
            .finish()
    }
}

#[derive(Serialize, Deserialize, PartialEq, Clone, Debug)]
pub struct FileDiff {
    pub old: Option<ECSigned<UnsignedFile>>,
    pub new: ECSigned<UnsignedFile>,
}

impl FileDiff {
    fn new(new: &ECSigned<UnsignedFile>) -> Self {
        let old = None;
        let new = new.clone();
        Self { old, new }
    }
    fn edit(old: &ECSigned<UnsignedFile>, new: &ECSigned<UnsignedFile>) -> Self {
        let old = Some(old.clone());
        let new = new.clone();
        Self { old, new }
    }
}
