use libsecp256k1::PublicKey;
use lockbook_models::tree::FileMetadata;
use uuid::Uuid;

pub fn public_key(username: &str) -> String {
    format!("username:{}:public_key", username)
}

pub fn username(pk: &PublicKey) -> String {
    format!("public_key:{}:username", stringify_public_key(pk))
}

pub fn owned_files(pk: &PublicKey) -> String {
    format!("public_key:{}:owned_files", stringify_public_key(pk))
}

pub fn data_cap(pk: &PublicKey) -> String {
    format!("public_key:{}:data_cap", stringify_public_key(pk))
}

pub fn file(id: Uuid) -> String {
    format!("file_id:{}:metadata", id)
}

pub fn size(id: Uuid) -> String {
    format!("file_id:{}:size", id)
}

pub fn meta<File: FileMetadata>(meta: &File) -> String {
    file(meta.id())
}

fn stringify_public_key(pk: &PublicKey) -> String {
    base64::encode(pk.serialize_compressed())
}
