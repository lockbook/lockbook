use std::time::{SystemTime, UNIX_EPOCH};
use core::num::ParseIntError;
use std::option::NoneError;
use std::time::SystemTimeError;

use crate::crypto::{PublicKey, RsaCryptoService, CryptoService, DecryptedValue, KeyPair, EncryptedValue};
use crate::error_enum;
use crate::crypto::EncryptionError;
use crate::crypto::DecryptionError;

enum Error {
    InvalidUsername,
    TimeStampOutOfBounds
}

error_enum! {
    enum VerificationError {
        TimeStampParseFailure(ParseIntError),
        DecryptionFailure(DecryptionError),
        IncompleteAuth(NoneError)
    }
}

error_enum! {
    enum AuthGenError {
        AuthEncryptionFailure(EncryptionError),
        InvalidTimeStamp(SystemTimeError)
    }
}

pub trait AuthService {
    fn verify_auth(
        pub_key: &PublicKey,
        username: &String,
        auth: &String,
    ) -> Result<(), VerificationError>;
    fn generate_auth(
        keys: &KeyPair,
        username: &String,
    ) -> Result<String, AuthGenError>;
}

pub struct AuthServiceImpl;

impl AuthService for AuthServiceImpl {
    fn verify_auth(
        pub_key: &PublicKey,
        username: &String,
        auth: &String,
    ) -> Result<(), VerificationError> {
        let decrypt_val = RsaCryptoService::decrypt_public(
            &PublicKey {
                n: pub_key.n.clone(),
                e: pub_key.e.clone(),
            },
            &EncryptedValue {
                garbage: auth.clone(),
            },
        )?;

        let mut auth_comp = decrypt_val.secret.split(",");
        let real_time = SystemTime::now().
            duration_since(UNIX_EPOCH)?.
            as_millis();

        let auth_time = auth_comp.next()?.parse::<u128>()?;

        if String::from(auth_comp.next()?) != username {
            return ;
        }

        let range = auth_time..auth_time + 50;

        if !range.contains(&real_time) {
            return ;
        }
        Ok(())
    }

    fn generate_auth(
        keys: &KeyPair,
        username: &String,
    ) -> Result<String, AuthGenError> {
        let decrypted = format!("{},{}",
                                username,
                                SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis().to_string());

        Ok(RsaCryptoService::encrypt_private(
            keys,
            &DecryptedValue { secret: decrypted })?.garbage)
    }
}