use crate::model::clock::{TimeGetter, timestamp};
use crate::model::crypto::*;
use libsecp256k1::{Message, PublicKey, SecretKey, SharedSecret, Signature};
use rand::rngs::OsRng;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::convert::TryInto;

use super::errors::{LbErrKind, LbResult, SignError, Unexpected};

pub fn generate_key() -> SecretKey {
    SecretKey::random(&mut OsRng)
}

pub fn sign<T: Serialize>(
    sk: &SecretKey, pk: &PublicKey, to_sign: T, time_getter: TimeGetter,
) -> LbResult<ECSigned<T>> {
    let timestamped = timestamp(to_sign, time_getter);
    let serialized = bincode::serialize(&timestamped).map_unexpected()?;
    let digest = Sha256::digest(&serialized);
    let message = &Message::parse_slice(&digest)
        .map_err(|err| LbErrKind::Sign(SignError::SignatureParseError(err)))?;
    let (signature, _) = libsecp256k1::sign(message, sk);
    Ok(ECSigned {
        timestamped_value: timestamped,
        signature: signature.serialize().to_vec(),
        public_key: *pk,
    })
}

pub fn verify<T: Serialize>(
    pk: &PublicKey, signed: &ECSigned<T>, max_delay_ms: u64, max_skew_ms: u64,
    time_getter: TimeGetter,
) -> LbResult<()> {
    if &signed.public_key != pk {
        Err(LbErrKind::Sign(SignError::WrongPublicKey))?;
    }

    let auth_time = signed.timestamped_value.timestamp;
    let current_time = time_getter().0;
    let max_skew_ms = max_skew_ms as i64;
    let max_delay_ms = max_delay_ms as i64;

    if current_time < auth_time - max_skew_ms {
        Err(LbErrKind::Sign(SignError::SignatureInTheFuture(
            (current_time - (auth_time - max_delay_ms)) as u64,
        )))?;
    }

    if current_time > auth_time + max_delay_ms {
        Err(LbErrKind::Sign(SignError::SignatureExpired(
            (auth_time + max_delay_ms - current_time) as u64,
        )))?;
    }

    // todo: evaluate potential waste here: didn't we just have this in it's
    // serialized form?
    let serialized = bincode::serialize(&signed.timestamped_value)?;

    let digest = Sha256::digest(&serialized).to_vec();
    let message = &Message::parse_slice(&digest).map_err(|err| {
        LbErrKind::Unexpected(format!(
            "unexpected failure to produce a message after hashing: {err:?}"
        ))
    })?;

    let signature = Signature::parse_standard_slice(&signed.signature)
        .map_err(|err| LbErrKind::Sign(SignError::SignatureParseError(err)))?;

    if libsecp256k1::verify(message, &signature, &signed.public_key) {
        Ok(())
    } else {
        Err(LbErrKind::Sign(SignError::SignatureInvalid))?
    }
}

pub fn get_aes_key(sk: &SecretKey, pk: &PublicKey) -> LbResult<AESKey> {
    SharedSecret::<Sha256>::new(pk, sk)
        .map_unexpected()?
        .as_ref()
        .try_into()
        .map_unexpected()
}

#[cfg(test)]
mod unit_tests {
    use libsecp256k1::PublicKey;

    use crate::model::clock::Timestamp;
    use crate::model::pubkey::*;

    static EARLY_CLOCK: fn() -> Timestamp = || Timestamp(500);
    static LATE_CLOCK: fn() -> Timestamp = || Timestamp(520);

    #[test]
    fn ec_test_sign_verify() {
        let sk = generate_key();
        let pk = PublicKey::from_secret_key(&sk);
        let value = sign(&sk, &pk, "Test", EARLY_CLOCK).unwrap();
        verify(&pk, &value, 20, 20, LATE_CLOCK).unwrap();
    }

    #[test]
    fn ec_test_sign_verify_late() {
        let sk = generate_key();
        let pk = PublicKey::from_secret_key(&sk);
        let value = sign(&sk, &pk, "Test", EARLY_CLOCK).unwrap();
        verify(&pk, &value, 10, 10, LATE_CLOCK).unwrap_err();
    }

    #[test]
    fn ec_test_shared_secret_one_party() {
        // Just sanity checks

        let key = generate_key();
        let shared_secret1 = get_aes_key(&key, &PublicKey::from_secret_key(&key)).unwrap();

        let shared_secret2 = get_aes_key(&key, &PublicKey::from_secret_key(&key)).unwrap();

        assert_eq!(shared_secret1, shared_secret2);
    }

    #[test]
    fn ec_test_shared_secret_two_parties() {
        // Just sanity checks

        let key1 = generate_key();
        let key2 = generate_key();

        let shared_secret1 = get_aes_key(&key1, &PublicKey::from_secret_key(&key2)).unwrap();

        let shared_secret2 = get_aes_key(&key2, &PublicKey::from_secret_key(&key1)).unwrap();

        assert_eq!(shared_secret1, shared_secret2);
    }

    #[test]
    fn same_sk_same_pk_sanity_check() {
        let key1 = generate_key();
        let key2 = SecretKey::parse(&key1.serialize()).unwrap();

        assert_eq!(key1, key2);

        assert_eq!(PublicKey::from_secret_key(&key1), PublicKey::from_secret_key(&key2));
    }
}
