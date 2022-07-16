use std::fmt::Debug;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;

use libsecp256k1::PublicKey;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

pub type AESKey = [u8; 32];
pub type DecryptedDocument = Vec<u8>;
pub type EncryptedDocument = AESEncrypted<DecryptedDocument>;
pub type EncryptedUserAccessKey = AESEncrypted<AESKey>;
pub type EncryptedFolderAccessKey = AESEncrypted<AESKey>;

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone, Hash)]
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

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Timestamped<T> {
    pub value: T,
    pub timestamp: i64,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct ECSigned<T> {
    pub timestamped_value: Timestamped<T>,
    #[serde(with = "serde_bytes")]
    pub signature: Vec<u8>,
    pub public_key: PublicKey,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone, Copy, Hash, PartialOrd, Ord)]
pub enum UserAccessMode {
    Read,
    Write,
    Owner,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct UserAccessInfo {
    pub mode: UserAccessMode,
    pub encrypted_by_username: String,
    pub encrypted_by_public_key: PublicKey,
    pub encrypted_for_username: String,
    pub encrypted_for_public_key: PublicKey,
    pub access_key: EncryptedUserAccessKey,
    pub file_name: SecretFileName,
    pub deleted: bool,
}

// todo(sharing): implement Hash for PublicKey or omit public keys from PartialEq and Eq impl's for Share
impl Hash for UserAccessInfo {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.encrypted_by_username.hash(state);
        // self.sharer_public_key.hash(state);
        self.encrypted_for_username.hash(state);
        // self.sharee_public_key.hash(state);
        self.mode.hash(state);
    }
}

/// A secret value that can impl an equality check by hmac'ing the
/// inner secret.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SecretFileName {
    pub encrypted_value: AESEncrypted<String>,
    pub hmac: [u8; 32],
}

impl PartialEq for SecretFileName {
    fn eq(&self, other: &Self) -> bool {
        self.hmac == other.hmac
    }
}

impl Eq for SecretFileName {}

impl Hash for SecretFileName {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.hmac.hash(state);
    }
}
