extern crate rand;
extern crate rsa;
use rsa::RSAPublicKey;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct EncryptedValue {
    pub garbage: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct DecryptedValue {
    pub secret: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct SignedValue {
    pub content: String,
    pub signature: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct EncryptedValueWithNonce {
    pub garbage: String,
    // https://cryptologie.net/article/361/breaking-https-aes-gcm-or-a-part-of-it/
    pub nonce: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct AesKey {
    pub key: String,
}

impl AesKey {
    pub(crate) fn to_decrypted_value(&self) -> DecryptedValue {
        DecryptedValue {
            secret: self.key.clone(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct UserAccessInfo {
    pub username: String,
    pub public_key: RSAPublicKey,
    pub access_key: EncryptedValue,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct FolderAccessInfo {
    pub folder_id: Uuid,
    pub access_key: EncryptedValueWithNonce,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct EncryptedFile {
    pub content: EncryptedValueWithNonce,
}
