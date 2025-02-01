use std::fmt::Debug;

use uuid::Uuid;

use crate::model::secret_filename::SecretFileName;
use crate::model::server_file::ServerFile;
use crate::model::signed_file::SignedFile;
use crate::model::access_info::{EncryptedFolderAccessKey, UserAccessInfo, UserAccessMode};
use crate::model::file_metadata::{DocumentHmac, FileMetadata, FileType, Owner};

pub trait FileLike: PartialEq + Debug + Clone + AsRef<FileMetadata> {
    fn id(&self) -> &Uuid;
    fn file_type(&self) -> FileType;
    fn parent(&self) -> &Uuid;
    fn secret_name(&self) -> &SecretFileName;
    fn owner(&self) -> Owner;
    fn explicitly_deleted(&self) -> bool;
    fn document_hmac(&self) -> Option<&DocumentHmac>;
    fn display(&self) -> String;
    fn user_access_keys(&self) -> &Vec<UserAccessInfo>;
    fn folder_access_key(&self) -> &EncryptedFolderAccessKey;

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

    fn display(&self) -> String {
        let fm: &FileMetadata = self.as_ref();
        match fm.file_type() {
            FileType::Folder => format!("id: {}/", fm.id),
            FileType::Document => format!("id: {}", fm.id),
            FileType::Link { target } => format!("id: {}, target: {}", fm.id, target),
        }
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
