use crate::crypto::{PublicKey, RsaCryptoService, CryptoService, DecryptedValue, KeyPair, EncryptedValue};

use std::time::{SystemTime, UNIX_EPOCH};

error_enum! {
    enum AuthError {
        DecryptionFailure(DecryptionError),

        IncompleteAuth(NoneError),

        AuthGenFailed(EncryptionError),
    }
}

error_enum! {
    enum VerificationError {
        TimeStampParseFailure(ParseIntError),
    }
}

error_enum! {
    enum GenerationError {

    }
}

pub trait AuthService {
    fn verify_auth(
        pub_key: &PublicKey,
        username: &String,
        auth: &String,
    ) -> Result<(), AuthError>;
    fn generate_auth(
        keys: &KeyPair,
        username: &String,
    ) -> Result<String, AuthError>;
}

pub struct AuthServiceImpl;

impl AuthService for AuthServiceImpl {
    fn verify_auth(
        pub_key: &PublicKey,
        username: &String,
        auth: &String,
    ) -> Result<(), AuthError> {
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
        let auth_username = String::from(auth_comp.next()?;
        let auth_time = auth_comp.next()?.parse::<u128>()?;

        if real_username != auth_username {
            return AuthError::IncorrectAuth(IncorrectUsername);
        }

        let range = auth_time..auth_time + 50;

        if !range.contains(&real_time) {
            return AuthError::IncorrectAuth(ExpiredAuth);
        }
        Ok(())
    }

    fn generate_auth(
        keys: &KeyPair,
        username: &String,
    ) -> Result<String, AuthError> {
        let decrypted = format!("{},{}",
                                username,
                                SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis().to_string());

        Ok(RsaCryptoService::encrypt_private(
            keys,
            &DecryptedValue { secret: decrypted })?.garbage)
    }
}