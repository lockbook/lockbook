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

pub fn stripe_user_info(pk: &PublicKey) -> String {
    format!("public_key:{}:stripe_user_info", stringify_public_key(pk))
}

pub fn public_key_from_stripe_customer_id(customer_id: &str) -> String {
    format!("stripe_customer_id:{}:public_key", customer_id)
}

pub fn android_purchase_token(pk: &PublicKey) -> String {
    format!("public_key:{}:android_purchase_token", stringify_public_key(pk))
}

pub fn size(id: Uuid) -> String {
    format!("file_id:{}:size", id)
}

pub fn meta<File: FileMetadata>(meta: &File) -> String {
    file(meta.id())
}

pub fn doc(id: Uuid, content_version: u64) -> String {
    format!("id-version:{}-{}:encrypted_document", id, content_version)
}

pub fn stringify_public_key(pk: &PublicKey) -> String {
    base64::encode(pk.serialize_compressed())
}
