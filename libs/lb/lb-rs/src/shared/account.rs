use crate::shared::pubkey;
use bip39_dict::Language;
use libsecp256k1::{PublicKey, SecretKey};
use serde::{Deserialize, Serialize};
use sha2::Digest;

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

    pub fn get_phrase(&self) -> [String; 24] {
        let key = self.private_key.serialize();
        let key_bits: String = key.iter().map(|byte| format!("{:08b}", byte)).collect();

        let checksum: String = sha2::Sha256::digest(&key)
            .into_iter()
            .map(|byte| format!("{:08b}", byte))
            .collect();

        let checksum_last_4_bits = &checksum[..4];
        let combined_bits = format!("{}{}", key_bits, checksum_last_4_bits);

        let mut phrase: [String; 24] = std::array::from_fn(|_| String::new());
        let mut i = 0;
        
        for chunk in combined_bits.chars().collect::<Vec<_>>().chunks(11) {
            let index = u16::from_str_radix(&chunk.iter().collect::<String>(), 2).unwrap();
            let word = bip39_dict::ENGLISH
                .lookup_word(bip39_dict::MnemonicIndex(index))
                .to_string();

            phrase[i] = word;
            i += 1;
        }

        phrase
    }

    pub fn phrase_to_private_key(phrases: [String; 24]) -> SecretKey {
        let mut combined_bits: String = phrases
            .iter()
            .map(|word| bip39_dict::ENGLISH.lookup_mnemonic(word).unwrap().0)
            .map(|comp| format!("{:011b}", comp))
            .collect();

        for _ in 0..4 {
            combined_bits.remove(253);
        }

        let key_bits = &combined_bits[..256];
        let checksum_last_4_bits = &combined_bits[256..260];

        let mut key: Vec<u8> = Vec::new();
        for chunk in key_bits.chars().collect::<Vec<_>>().chunks(8) {
            let comp = u8::from_str_radix(&chunk.iter().collect::<String>(), 2).unwrap();

            key.push(comp);
        }

        let gen_checksum: String = sha2::Sha256::digest(&key)
            .into_iter()
            .map(|byte| format!("{:08b}", byte))
            .collect();

        let gen_checksum_last_4 = &gen_checksum[..4];

        assert!(gen_checksum_last_4 == checksum_last_4_bits);

        SecretKey::parse_slice(&key).unwrap()
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

    use crate::Account;

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

#[cfg(test)]
mod test_account_key_and_phrase {
    use libsecp256k1::SecretKey;
    use rand::rngs::OsRng;

    use crate::Account;

    #[test]
    fn account_key_and_phrase_eq() {
        let account1 = Account {
            username: "test".to_string(),
            api_url: "test.com".to_string(),
            private_key: SecretKey::random(&mut OsRng),
        };

        let phrase = account1.get_phrase();
        let reverse = Account::phrase_to_private_key(phrase);

        assert!(account1.private_key == reverse);
    }
}
