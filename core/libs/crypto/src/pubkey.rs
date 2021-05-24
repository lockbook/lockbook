use libsecp256k1::Message;
use libsecp256k1::{PublicKey, SecretKey, SharedSecret, Signature};
use rand::rngs::OsRng;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::convert::TryInto;

use crate::clock_service::Clock;
use lockbook_models::crypto::*;

pub trait PubKeyCryptoService {
    fn generate_key() -> SecretKey;
    fn sign<T: Serialize>(secret_key: &SecretKey, to_sign: T) -> Result<ECSigned<T>, ECSignError>;
    fn verify<T: Serialize>(
        public_key: &PublicKey,
        to_verify: &ECSigned<T>,
        max_delay_ms: u64,
        max_skew_ms: u64,
    ) -> Result<(), ECVerifyError>;
    fn get_aes_key(sk: &SecretKey, pk: &PublicKey) -> Result<AESKey, GetAesKeyError>;
}

impl<Time: Clock> PubKeyCryptoService for ElipticCurve<Time> {
    fn generate_key() -> SecretKey {
        SecretKey::random(&mut OsRng)
    }

    fn sign<T: Serialize>(sk: &SecretKey, to_sign: T) -> Result<ECSigned<T>, ECSignError> {
        let timestamped = Time::timestamp(to_sign);
        let serialized = bincode::serialize(&timestamped).map_err(ECSignError::Serialization)?;
        let digest = Sha256::digest(&serialized);
        let message = &Message::parse_slice(&digest).map_err(ECSignError::ParseError)?;
        let (signature, _) = libsecp256k1::sign(&message, &sk);
        Ok(ECSigned {
            timestamped_value: timestamped,
            signature: signature.serialize().to_vec(),
            public_key: PublicKey::from_secret_key(&sk),
        })
    }

    fn verify<T: Serialize>(
        pk: &PublicKey,
        signed: &ECSigned<T>,
        max_delay_ms: u64,
        max_skew_ms: u64,
    ) -> Result<(), ECVerifyError> {
        if &signed.public_key != pk {
            return Err(ECVerifyError::WrongPublicKey);
        }

        let auth_time = signed.timestamped_value.timestamp;
        let current_time = Time::get_time();
        let max_skew_ms = max_skew_ms as i64;
        let max_delay_ms = max_delay_ms as i64;

        if current_time < auth_time - max_skew_ms {
            return Err(ECVerifyError::SignatureInTheFuture(
                (current_time - (auth_time - max_delay_ms)) as u64,
            ));
        }

        if current_time > auth_time + max_delay_ms {
            return Err(ECVerifyError::SignatureExpired(
                (auth_time + max_delay_ms - current_time) as u64,
            ));
        }

        let serialized =
            bincode::serialize(&signed.timestamped_value).map_err(ECVerifyError::Serialization)?;

        let digest = Sha256::digest(&serialized).to_vec();
        let message = &Message::parse_slice(&digest).map_err(ECVerifyError::ParseError)?;
        let signature = Signature::parse_standard_slice(&signed.signature)
            .map_err(ECVerifyError::ParseError)?;

        if libsecp256k1::verify(&message, &signature, &signed.public_key) {
            Ok(())
        } else {
            Err(ECVerifyError::SignatureInvalid)
        }
    }

    fn get_aes_key(sk: &SecretKey, pk: &PublicKey) -> Result<AESKey, GetAesKeyError> {
        SharedSecret::<Sha256>::new(&pk, &sk)
            .map_err(GetAesKeyError::SharedSecretError)?
            .as_ref()
            .try_into()
            .map_err(|_| GetAesKeyError::SharedSecretUnexpectedSize)
    }
}

pub struct ElipticCurve<Time: Clock> {
    _clock: Time,
}

#[derive(Debug)]
pub enum ECSignError {
    ParseError(libsecp256k1::Error),
    Serialization(bincode::Error),
}

#[derive(Debug)]
pub enum ECVerifyError {
    SignatureInvalid,
    WrongPublicKey,
    SignatureInTheFuture(u64),
    SignatureExpired(u64),
    ParseError(libsecp256k1::Error),
    Serialization(bincode::Error),
}

#[derive(Debug)]
pub enum GetAesKeyError {
    SharedSecretUnexpectedSize,
    SharedSecretError(libsecp256k1::Error),
}

#[cfg(test)]
mod unit_test_pubkey {
    use crate::clock_service::Clock;
    use crate::pubkey::*;
    use libsecp256k1::PublicKey;

    struct EarlyClock;
    impl Clock for EarlyClock {
        fn get_time() -> i64 {
            500
        }
    }

    struct LateClock;
    impl Clock for LateClock {
        fn get_time() -> i64 {
            520
        }
    }

    #[test]
    fn ec_test_sign_verify() {
        let key = ElipticCurve::<EarlyClock>::generate_key();
        let value = ElipticCurve::<EarlyClock>::sign(&key, "Test").unwrap();
        ElipticCurve::<LateClock>::verify(&PublicKey::from_secret_key(&key), &value, 20, 20)
            .unwrap();
    }

    #[test]
    fn ec_test_sign_verify_late() {
        let key = ElipticCurve::<EarlyClock>::generate_key();
        let value = ElipticCurve::<EarlyClock>::sign(&key, "Test").unwrap();
        ElipticCurve::<LateClock>::verify(&PublicKey::from_secret_key(&key), &value, 10, 10)
            .unwrap_err();
    }

    #[test]
    fn ec_test_shared_secret_one_party() {
        // Just sanity checks

        let key = ElipticCurve::<EarlyClock>::generate_key();
        let shared_secret1 =
            ElipticCurve::<EarlyClock>::get_aes_key(&key, &PublicKey::from_secret_key(&key))
                .unwrap();

        let shared_secret2 =
            ElipticCurve::<EarlyClock>::get_aes_key(&key, &PublicKey::from_secret_key(&key))
                .unwrap();

        assert_eq!(shared_secret1, shared_secret2);
    }

    #[test]
    fn ec_test_shared_secret_two_parties() {
        // Just sanity checks

        let key1 = ElipticCurve::<EarlyClock>::generate_key();
        let key2 = ElipticCurve::<EarlyClock>::generate_key();

        let shared_secret1 =
            ElipticCurve::<EarlyClock>::get_aes_key(&key1, &PublicKey::from_secret_key(&key2))
                .unwrap();

        let shared_secret2 =
            ElipticCurve::<EarlyClock>::get_aes_key(&key2, &PublicKey::from_secret_key(&key1))
                .unwrap();

        assert_eq!(shared_secret1, shared_secret2);
    }
}
