extern crate rand;
extern crate rsa;
use rsa::RSAPublicKey;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::marker::PhantomData;
use uuid::Uuid;

pub type AESKey = [u8; 32];
pub type DecryptedDocument = Vec<u8>;
pub type EncryptedDocument = AESEncrypted<DecryptedDocument>;
pub type EncryptedUserAccessKey = RSAEncrypted<AESKey>;
pub type EncryptedFolderAccessKey = AESEncrypted<AESKey>;

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct AESEncrypted<T: DeserializeOwned> {
    #[serde(with = "serde_bytes")]
    pub value: Vec<u8>,
    #[serde(with = "serde_bytes")]
    pub nonce: Vec<u8>,
    pub _t: PhantomData<T>,
}

impl<T: DeserializeOwned> AESEncrypted<T> {
    /// creates an AESEncrypted from a source of already-encrypted bytes
    pub fn new<V: Into<Vec<u8>>, N: Into<Vec<u8>>>(value: V, nonce: N) -> Self {
        AESEncrypted {
            value: value.into(),
            nonce: nonce.into(),
            _t: PhantomData,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct RSAEncrypted<T: DeserializeOwned> {
    #[serde(with = "serde_bytes")]
    pub value: Vec<u8>,
    pub _t: PhantomData<T>,
}

impl<T: DeserializeOwned> RSAEncrypted<T> {
    /// creates an RSAEncrypted from a source of already-encrypted bytes
    pub fn new<V: Into<Vec<u8>>>(value: V) -> Self {
        RSAEncrypted {
            value: value.into(),
            _t: PhantomData,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Timestamped<T> {
    pub value: T,
    pub timestamp: u64,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct RSASigned<T> {
    pub timestamped_value: Timestamped<T>,
    #[serde(with = "serde_bytes")]
    pub signature: Vec<u8>,
    pub public_key: RSAPublicKey,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct UserAccessInfo {
    pub username: String,
    pub public_key: RSAPublicKey,
    pub access_key: EncryptedUserAccessKey,
}

// TODO: remove all of this struct
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct FolderAccessInfo {
    pub folder_id: Uuid, // TODO remove this?
    pub access_key: EncryptedFolderAccessKey,
}
