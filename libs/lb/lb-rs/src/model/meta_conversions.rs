use super::crypto::Timestamped;
use super::file_metadata::FileMetadata;
use super::meta::Meta;
use super::server_file::ServerFile;
use super::server_meta::ServerMeta;
use super::signed_file::SignedFile;
use super::signed_meta::SignedMeta;

impl From<FileMetadata> for Meta {
    fn from(value: FileMetadata) -> Self {
        Meta::V1 {
            id: value.id,
            file_type: value.file_type,
            parent: value.parent,
            name: value.name,
            owner: value.owner,
            is_deleted: value.is_deleted,
            doc_hmac: value.document_hmac,
            user_access_keys: value.user_access_keys,
            folder_access_key: value.folder_access_key,
            doc_size: None,
        }
    }
}

impl From<SignedFile> for SignedMeta {
    fn from(value: SignedFile) -> Self {
        let timestamped_value = Timestamped {
            value: value.timestamped_value.value.into(),
            timestamp: value.timestamped_value.timestamp,
        };

        Self { timestamped_value, signature: value.signature, public_key: value.public_key }
    }
}

impl From<ServerFile> for ServerMeta {
    fn from(value: ServerFile) -> Self {
        Self { file: value.file.into(), version: value.version }
    }
}
