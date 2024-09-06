use crate::logic::pubkey;
use libsecp256k1::{PublicKey, SecretKey};
use serde::{Deserialize, Serialize};

pub const MAX_USERNAME_LENGTH: usize = 32;

pub type Username = String;
pub type ApiUrl = String;

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Account {
    pub username: Username,
    pub api_url: ApiUrl,
    #[serde(with = "secret_key_serializer")]
    pub private_key: SecretKey,
}

impl Account {
    pub fn new(username: String, api_url: String) -> Self {
        let private_key = pubkey::generate_key();
        Self { username, api_url, private_key }
    }

    pub fn public_key(&self) -> PublicKey {
        PublicKey::from_secret_key(&self.private_key)
    }
}

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
        let key = <Vec<u8>>::deserialize(deserializer)?;
        let sk = SecretKey::parse_slice(&key).map_err(serde::de::Error::custom)?;
        Ok(sk)
    }
}

#[cfg(test)]
mod test_account_serialization {
    use libsecp256k1::SecretKey;
    use rand::rngs::OsRng;

    use crate::logic::account::Account;

    #[test]
    fn account_serialize_deserialize() {
        let account1 = Account {
            username: "test".to_string(),
            api_url: "test.com".to_string(),
            private_key: SecretKey::random(&mut OsRng),
        };

        let encoded: Vec<u8> = bincode::serialize(&account1).unwrap();
        let account2: Account = bincode::deserialize(&encoded).unwrap();

        assert_eq!(account1, account2);
    }
}
