use libsecp256k1::{PublicKey, SecretKey};
use serde::{Deserialize, Serialize};

pub type Username = String;
pub type ApiUrl = String;

#[derive(PartialEq, Debug, Serialize, Deserialize)]
pub struct Account {
    pub username: Username,
    pub api_url: ApiUrl,
    #[serde(with = "secret_key_serializer")]
    pub private_key: SecretKey,
}

impl Account {
    pub fn public_key(&self) -> PublicKey {
        PublicKey::from_secret_key(&self.private_key)
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct SecretKeyHolder(pub [u8; 32]);

pub mod secret_key_serializer {
    use libsecp256k1::SecretKey;
    use serde::de::Deserialize;
    use serde::de::Deserializer;
    use serde::ser::Serializer;

    pub fn serialize<S>(sk: &SecretKey, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_bytes(&sk.serialize())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<SecretKey, D::Error>
    where
        D: Deserializer<'de>,
    {
        let key = <[u8; 32]>::deserialize(deserializer)?;
        let sk = SecretKey::parse(&key).map_err(serde::de::Error::custom)?;
        Ok(sk)
    }
}
