extern crate rand;
extern crate rsa;
extern crate uuid;
use rsa::RSAPublicKey;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct EncryptedValue {
    pub garbage: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, Hash)]
pub struct DecryptedValue {
    pub secret: String,
}

impl From<&str> for DecryptedValue {
    fn from(s: &str) -> Self {
        DecryptedValue {
            secret: s.to_string(),
        }
    }
}

impl From<String> for DecryptedValue {
    fn from(secret: String) -> Self {
        DecryptedValue { secret }
    }
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

impl From<DecryptedValue> for AesKey {
    fn from(decrypted: DecryptedValue) -> Self {
        AesKey {
            key: decrypted.secret,
        }
    }
}

impl AesKey {
    pub fn to_decrypted_value(&self) -> DecryptedValue {
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
    pub folder_id: Uuid, // TODO remove this?
    pub access_key: EncryptedValueWithNonce,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Document {
    pub content: EncryptedValueWithNonce,
}
