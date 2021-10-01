use libsecp256k1::PublicKey;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::marker::PhantomData;
use base64;
use base64_serde::base64_serde_type;

base64_serde_type!(Base64, base64::STANDARD);

pub type AESKey = [u8; 32];
pub type DecryptedDocument = Vec<u8>;
pub type EncryptedDocument = AESEncrypted<DecryptedDocument>;
pub type EncryptedUserAccessKey = AESEncrypted<AESKey>;
pub type EncryptedFolderAccessKey = AESEncrypted<AESKey>;

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct AESEncrypted<T: DeserializeOwned> {
    #[serde(with = "Base64")]
    pub value: Vec<u8>,
    #[serde(with = "Base64")]
    pub nonce: Vec<u8>,
    #[serde(skip_serializing, default = "PhantomData::default")]
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
pub struct Timestamped<T> {
    pub value: T,
    pub timestamp: i64,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct ECSigned<T> {
    pub timestamped_value: Timestamped<T>,
    #[serde(with = "Base64")]
    pub signature: Vec<u8>,
    pub public_key: PublicKey,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct UserAccessInfo {
    pub username: String,
    pub encrypted_by: PublicKey,
    pub access_key: EncryptedUserAccessKey,
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
