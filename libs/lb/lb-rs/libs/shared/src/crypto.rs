use std::fmt::Debug;
use std::marker::PhantomData;

use libsecp256k1::PublicKey;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

pub type AESKey = [u8; 32];
pub type DecryptedDocument = Vec<u8>;
pub type EncryptedDocument = AESEncrypted<DecryptedDocument>;

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct AESEncrypted<T: DeserializeOwned> {
    #[serde(with = "serde_bytes")]
    pub value: Vec<u8>,
    #[serde(with = "serde_bytes")]
    pub nonce: Vec<u8>,
    #[serde(skip_serializing, default = "PhantomData::default")]
    pub _t: PhantomData<T>,
}

impl<T: DeserializeOwned> AESEncrypted<T> {
    /// creates an AESEncrypted from a source of already-encrypted bytes
    pub fn new<V: Into<Vec<u8>>, N: Into<Vec<u8>>>(value: V, nonce: N) -> Self {
        AESEncrypted { value: value.into(), nonce: nonce.into(), _t: PhantomData }
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct Timestamped<T> {
    pub value: T,
    pub timestamp: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ECSigned<T> {
    pub timestamped_value: Timestamped<T>,
    #[serde(with = "serde_bytes")]
    pub signature: Vec<u8>,
    pub public_key: PublicKey,
}
