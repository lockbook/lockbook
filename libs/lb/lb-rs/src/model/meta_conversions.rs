use super::{
    crypto::Timestamped, file_metadata::FileMetadata, meta::Meta, server_file::ServerFile,
    server_meta::ServerMeta, signed_file::SignedFile, signed_meta::SignedMeta,
};

impl From<FileMetadata> for Meta {
    fn from(value: FileMetadata) -> Self {
        Meta::V1 {
            id: value.id,
            file_type: value.file_type,
            parent: value.parent,
            name: value.name,
            owner: value.owner,
            is_deleted: value.is_deleted,
            document_hmac: value.document_hmac,
            user_access_keys: value.user_access_keys,
            folder_access_key: value.folder_access_key,
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
        Self { file: todo!(), version: value.version }
    }
}
