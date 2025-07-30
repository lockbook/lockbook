use std::fmt::Debug;

use uuid::Uuid;

use crate::model::access_info::{EncryptedFolderAccessKey, UserAccessInfo, UserAccessMode};
use crate::model::file_metadata::{DocumentHmac, FileMetadata, FileType, Owner};
use crate::model::secret_filename::SecretFileName;
use crate::model::server_file::ServerFile;
use crate::model::signed_file::SignedFile;

use super::meta::Meta;
use super::server_meta::ServerMeta;
use super::signed_meta::SignedMeta;

pub trait FileLike: PartialEq + Debug + Clone {
    fn id(&self) -> &Uuid;
    fn file_type(&self) -> FileType;
    fn parent(&self) -> &Uuid;
    fn secret_name(&self) -> &SecretFileName;
    fn owner(&self) -> Owner;
    fn explicitly_deleted(&self) -> bool;
    fn document_hmac(&self) -> Option<&DocumentHmac>;
    fn user_access_keys(&self) -> &Vec<UserAccessInfo>;
    fn folder_access_key(&self) -> &EncryptedFolderAccessKey;

    fn display(&self) -> String {
        match self.file_type() {
            FileType::Folder => format!("id: {}/", self.id()),
            FileType::Document => format!("id: {}", self.id()),
            FileType::Link { target } => format!("id: {}, target: {}", self.id(), target),
        }
    }

    fn is_folder(&self) -> bool {
        self.file_type() == FileType::Folder
    }

    fn is_link(&self) -> bool {
        matches!(self.file_type(), FileType::Link { .. })
    }

    fn is_document(&self) -> bool {
        self.file_type() == FileType::Document
    }

    fn is_root(&self) -> bool {
        self.id() == self.parent()
    }

    fn access_mode(&self, pk: &Owner) -> Option<UserAccessMode> {
        self.user_access_keys()
            .iter()
            .filter(|k| !k.deleted)
            .find(|k| k.encrypted_for == pk.0)
            .map(|k| k.mode)
    }

    fn is_shared(&self) -> bool {
        self.user_access_keys()
            .iter()
            .any(|k| !k.deleted && k.encrypted_for != k.encrypted_by)
    }
}

impl FileLike for Meta {
    fn id(&self) -> &Uuid {
        match self {
            Meta::V1 { id, .. } => id,
        }
    }

    fn file_type(&self) -> FileType {
        match self {
            Meta::V1 { file_type, .. } => *file_type,
        }
    }

    fn parent(&self) -> &Uuid {
        match self {
            Meta::V1 { parent, .. } => parent,
        }
    }

    fn secret_name(&self) -> &SecretFileName {
        match self {
            Meta::V1 { name, .. } => name,
        }
    }

    fn owner(&self) -> Owner {
        match self {
            Meta::V1 { owner, .. } => *owner,
        }
    }

    fn explicitly_deleted(&self) -> bool {
        match self {
            Meta::V1 { is_deleted, .. } => *is_deleted,
        }
    }

    fn document_hmac(&self) -> Option<&DocumentHmac> {
        match self {
            Meta::V1 { doc_hmac: document_hmac, .. } => document_hmac.as_ref(),
        }
    }

    fn user_access_keys(&self) -> &Vec<UserAccessInfo> {
        match self {
            Meta::V1 { user_access_keys, .. } => user_access_keys,
        }
    }

    fn folder_access_key(&self) -> &EncryptedFolderAccessKey {
        match self {
            Meta::V1 { folder_access_key, .. } => folder_access_key,
        }
    }
}

impl<F> FileLike for F
where
    F: AsRef<FileMetadata> + PartialEq + Debug + Clone,
{
    fn id(&self) -> &Uuid {
        let fm: &FileMetadata = self.as_ref();
        &fm.id
    }

    fn file_type(&self) -> FileType {
        let fm: &FileMetadata = self.as_ref();
        fm.file_type
    }

    fn parent(&self) -> &Uuid {
        let fm: &FileMetadata = self.as_ref();
        &fm.parent
    }

    fn secret_name(&self) -> &SecretFileName {
        let fm: &FileMetadata = self.as_ref();
        &fm.name
    }

    fn owner(&self) -> Owner {
        let fm: &FileMetadata = self.as_ref();
        fm.owner
    }

    fn explicitly_deleted(&self) -> bool {
        let fm: &FileMetadata = self.as_ref();
        fm.is_deleted
    }

    fn document_hmac(&self) -> Option<&DocumentHmac> {
        let fm: &FileMetadata = self.as_ref();
        fm.document_hmac.as_ref()
    }

    fn user_access_keys(&self) -> &Vec<UserAccessInfo> {
        let fm: &FileMetadata = self.as_ref();
        &fm.user_access_keys
    }

    fn folder_access_key(&self) -> &EncryptedFolderAccessKey {
        let fm: &FileMetadata = self.as_ref();
        &fm.folder_access_key
    }
}

impl AsRef<FileMetadata> for FileMetadata {
    fn as_ref(&self) -> &FileMetadata {
        self
    }
}

impl AsRef<FileMetadata> for SignedFile {
    fn as_ref(&self) -> &FileMetadata {
        &self.timestamped_value.value
    }
}

impl AsRef<FileMetadata> for ServerFile {
    fn as_ref(&self) -> &FileMetadata {
        self.file.as_ref()
    }
}

impl FileLike for SignedMeta {
    fn id(&self) -> &Uuid {
        self.timestamped_value.value.id()
    }

    fn file_type(&self) -> FileType {
        self.timestamped_value.value.file_type()
    }

    fn parent(&self) -> &Uuid {
        self.timestamped_value.value.parent()
    }

    fn secret_name(&self) -> &SecretFileName {
        self.timestamped_value.value.secret_name()
    }

    fn owner(&self) -> Owner {
        self.timestamped_value.value.owner()
    }

    fn explicitly_deleted(&self) -> bool {
        self.timestamped_value.value.explicitly_deleted()
    }

    fn document_hmac(&self) -> Option<&DocumentHmac> {
        self.timestamped_value.value.document_hmac()
    }

    fn user_access_keys(&self) -> &Vec<UserAccessInfo> {
        self.timestamped_value.value.user_access_keys()
    }

    fn folder_access_key(&self) -> &EncryptedFolderAccessKey {
        self.timestamped_value.value.folder_access_key()
    }
}

impl FileLike for ServerMeta {
    fn id(&self) -> &Uuid {
        self.file.timestamped_value.value.id()
    }

    fn file_type(&self) -> FileType {
        self.file.timestamped_value.value.file_type()
    }

    fn parent(&self) -> &Uuid {
        self.file.timestamped_value.value.parent()
    }

    fn secret_name(&self) -> &SecretFileName {
        self.file.timestamped_value.value.secret_name()
    }

    fn owner(&self) -> Owner {
        self.file.timestamped_value.value.owner()
    }

    fn explicitly_deleted(&self) -> bool {
        self.file.timestamped_value.value.explicitly_deleted()
    }

    fn document_hmac(&self) -> Option<&DocumentHmac> {
        self.file.timestamped_value.value.document_hmac()
    }

    fn user_access_keys(&self) -> &Vec<UserAccessInfo> {
        self.file.timestamped_value.value.user_access_keys()
    }

    fn folder_access_key(&self) -> &EncryptedFolderAccessKey {
        self.file.timestamped_value.value.folder_access_key()
    }
}
