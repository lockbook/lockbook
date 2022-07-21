use crate::access_info::{EncryptedFolderAccessKey, UserAccessInfo};
use crate::account::Username;
use crate::file_metadata::{FileMetadata, FileType, Owner};
use crate::secret_filename::SecretFileName;
use crate::server_file::ServerFile;
use crate::signed_file::SignedFile;
use crate::staged::StagedFile;
use std::collections::HashMap;
use uuid::Uuid;

pub trait FileLike: PartialEq {
    fn id(&self) -> Uuid;
    fn file_type(&self) -> FileType;
    fn parent(&self) -> Uuid;
    fn secret_name(&self) -> &SecretFileName;
    fn owner(&self) -> Owner;
    fn explicitly_deleted(&self) -> bool;
    fn display(&self) -> String;
    fn user_access_keys(&self) -> &HashMap<Username, UserAccessInfo>;
    fn folder_access_keys(&self) -> &EncryptedFolderAccessKey;

    fn is_folder(&self) -> bool {
        self.file_type() == FileType::Folder
    }

    fn is_document(&self) -> bool {
        self.file_type() == FileType::Document
    }

    fn is_root(&self) -> bool {
        self.id() == self.parent()
    }
}

impl FileLike for FileMetadata {
    fn id(&self) -> Uuid {
        self.id
    }

    fn file_type(&self) -> FileType {
        self.file_type
    }

    fn parent(&self) -> Uuid {
        self.parent
    }

    fn secret_name(&self) -> &SecretFileName {
        &self.name
    }

    fn owner(&self) -> Owner {
        self.owner
    }

    fn explicitly_deleted(&self) -> bool {
        self.is_deleted
    }

    fn display(&self) -> String {
        match self.file_type() {
            FileType::Folder => format!("id: {}/", self.id),
            FileType::Document => format!("id: {}", self.id),
        }
    }

    fn user_access_keys(&self) -> &HashMap<Username, UserAccessInfo> {
        &self.user_access_keys
    }

    fn folder_access_keys(&self) -> &EncryptedFolderAccessKey {
        &self.folder_access_keys
    }
}

impl FileLike for SignedFile {
    fn id(&self) -> Uuid {
        self.timestamped_value.value.id()
    }

    fn file_type(&self) -> FileType {
        self.timestamped_value.value.file_type()
    }

    fn parent(&self) -> Uuid {
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

    fn display(&self) -> String {
        self.timestamped_value.value.display()
    }

    fn user_access_keys(&self) -> &HashMap<Username, UserAccessInfo> {
        self.timestamped_value.value.user_access_keys()
    }

    fn folder_access_keys(&self) -> &EncryptedFolderAccessKey {
        self.timestamped_value.value.folder_access_keys()
    }
}

impl FileLike for ServerFile {
    fn id(&self) -> Uuid {
        self.file.id()
    }

    fn file_type(&self) -> FileType {
        self.file.file_type()
    }

    fn parent(&self) -> Uuid {
        self.file.parent()
    }

    fn secret_name(&self) -> &SecretFileName {
        self.file.secret_name()
    }

    fn owner(&self) -> Owner {
        self.file.owner()
    }

    fn explicitly_deleted(&self) -> bool {
        self.file.explicitly_deleted()
    }

    fn display(&self) -> String {
        self.file.display()
    }

    fn user_access_keys(&self) -> &HashMap<Username, UserAccessInfo> {
        self.file.user_access_keys()
    }

    fn folder_access_keys(&self) -> &EncryptedFolderAccessKey {
        self.file.folder_access_keys()
    }
}

impl<'a, F: FileLike> FileLike for &'a F {
    fn id(&self) -> Uuid {
        (*self).id()
    }

    fn file_type(&self) -> FileType {
        (*self).file_type()
    }

    fn parent(&self) -> Uuid {
        (*self).parent()
    }

    fn secret_name(&self) -> &SecretFileName {
        (*self).secret_name()
    }

    fn owner(&self) -> Owner {
        (*self).owner()
    }

    fn explicitly_deleted(&self) -> bool {
        (*self).explicitly_deleted()
    }

    fn display(&self) -> String {
        (*self).display()
    }

    fn user_access_keys(&self) -> &HashMap<Username, UserAccessInfo> {
        (*self).user_access_keys()
    }

    fn folder_access_keys(&self) -> &EncryptedFolderAccessKey {
        (*self).folder_access_keys()
    }
}

impl<Base: FileLike, Staged: FileLike> FileLike for StagedFile<Base, Staged> {
    fn id(&self) -> Uuid {
        match self {
            StagedFile::Base(file) => file.id(),
            StagedFile::Staged(file) => file.id(),
            StagedFile::Both { base: _, staged: file } => file.id(),
        }
    }

    fn file_type(&self) -> FileType {
        match self {
            StagedFile::Base(file) => file.file_type(),
            StagedFile::Staged(file) => file.file_type(),
            StagedFile::Both { base: _, staged: file } => file.file_type(),
        }
    }

    fn parent(&self) -> Uuid {
        match self {
            StagedFile::Base(file) => file.parent(),
            StagedFile::Staged(file) => file.parent(),
            StagedFile::Both { base: _, staged: file } => file.parent(),
        }
    }

    fn secret_name(&self) -> &SecretFileName {
        match self {
            StagedFile::Base(file) => file.secret_name(),
            StagedFile::Staged(file) => file.secret_name(),
            StagedFile::Both { base: _, staged: file } => file.secret_name(),
        }
    }

    fn owner(&self) -> Owner {
        match self {
            StagedFile::Base(file) => file.owner(),
            StagedFile::Staged(file) => file.owner(),
            StagedFile::Both { base: _, staged: file } => file.owner(),
        }
    }

    fn explicitly_deleted(&self) -> bool {
        match self {
            StagedFile::Base(file) => file.explicitly_deleted(),
            StagedFile::Staged(file) => file.explicitly_deleted(),
            StagedFile::Both { base: _, staged: file } => file.explicitly_deleted(),
        }
    }

    fn display(&self) -> String {
        match self {
            StagedFile::Base(file) => file.display(),
            StagedFile::Staged(file) => file.display(),
            StagedFile::Both { base: _, staged: file } => file.display(),
        }
    }

    fn user_access_keys(&self) -> &HashMap<Username, UserAccessInfo> {
        match self {
            StagedFile::Base(file) => file.user_access_keys(),
            StagedFile::Staged(file) => file.user_access_keys(),
            StagedFile::Both { base: _, staged: file } => file.user_access_keys(),
        }
    }

    fn folder_access_keys(&self) -> &EncryptedFolderAccessKey {
        match self {
            StagedFile::Base(file) => file.folder_access_keys(),
            StagedFile::Staged(file) => file.folder_access_keys(),
            StagedFile::Both { base: _, staged: file } => file.folder_access_keys(),
        }
    }
}
