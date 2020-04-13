use core::num::ParseIntError;
use std::option::NoneError;
use std::time::{SystemTime, UNIX_EPOCH};
use std::time::SystemTimeError;

use crate::auth_service::VerificationError::{DecryptionFailure, IncompleteAuth, InvalidTimeStamp, InvalidUsername, TimeStampOutOfBounds, TimeStampParseFailure};
use crate::crypto::{CryptoService, DecryptedValue, EncryptedValue, KeyPair, PublicKey, RsaCryptoService};
use crate::crypto::DecryptionError;
use crate::crypto::EncryptionError;
use crate::error_enum;


pub struct Clock;

impl Clock {
    fn get_time() -> Result<u128, SystemTimeError> {
        Ok(SystemTime::now()
            .duration_since(UNIX_EPOCH)?
            .as_millis())
    }
}

#[derive(Debug)]
pub enum VerificationError {
    TimeStampParseFailure(ParseIntError),
    DecryptionFailure(DecryptionError),
    IncompleteAuth(NoneError),
    InvalidTimeStamp(SystemTimeError),
    InvalidUsername,
    TimeStampOutOfBounds,
}

// impl PartialEq for VerificationError {
//     fn eq(&self, other: &Self) -> bool {
//         discriminant(&self).eq(&discriminant(&other))
//     }
// }

impl From<ParseIntError> for VerificationError {
    fn from(e: ParseIntError) -> Self { TimeStampParseFailure(e) }
}

impl From<DecryptionError> for VerificationError {
    fn from(e: DecryptionError) -> Self { DecryptionFailure(e) }
}

impl From<NoneError> for VerificationError {
    fn from(e: NoneError) -> Self { IncompleteAuth(e) }
}

impl From<SystemTimeError> for VerificationError {
    fn from(e: SystemTimeError) -> Self { InvalidTimeStamp(e) }
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
        let real_time = Clock::get_time()?;

        if &String::from(auth_comp.next()?) != username {
            return Err(InvalidUsername);
        }

        let auth_time = auth_comp.next()?.parse::<u128>()?;
        let range = auth_time..auth_time + 50;

        if !range.contains(&real_time) {
            return Err(TimeStampOutOfBounds);
        }
        Ok(())
    }

    fn generate_auth(
        keys: &KeyPair,
        username: &String,
    ) -> Result<String, AuthGenError> {
        let decrypted = format!("{},{}",
                                username,
                                Clock::get_time()?.to_string());

        Ok(RsaCryptoService::encrypt_private(
            keys,
            &DecryptedValue { secret: decrypted })?.garbage)
    }
}

#[cfg(test)]
mod unit_tests {
    use crate::auth_service::{AuthServiceImpl, AuthService, VerificationError};
    use crate::crypto::{CryptoService, RsaCryptoService, DecryptedValue, KeyPair, PrivateKey, PublicKey};
    use std::mem::discriminant;

    #[test]
    fn test_auth_inverse_property() {
        let keys = KeyPair {
            public_key: PublicKey {
                n: "y4K0W3aMqTZTLMSJcdVHQFpotEZZCBkyKeKI4pd/npSVVPzqIz7TvQfVyCvQgWHtg9uzHqP9HhSBFvcsuam7BygxdCyeCJ8a0oIzj6dOq3IBTN9IdF4GHLnYnh2zmAEuJKgIDLrzwJl8uE3R6okMvtvI0Sd+mmZhR9lAaN9ekVbBZvYxpPc1FObHezk+z5FIe6LqxBScZXcC96+scos/j72NsnOPags4kUsAucQZVSqM5VHjpWbKR34IpQOYQxGoJEab6YH8jUnUkDlMGSctUozHc9N3RM0Cm2PA/ZbcOLVDppsHIH+gzgis6GXQotAaWlcP0M4DiyVzydG/Qgh44w==".to_string(),
                e: "AQAB".to_string(),
            },
            private_key: PrivateKey {
                d: "DiNpdkU5JnRQuPZ6ef8QMSdWyNduTgK6GnDTg7J0ukamTT444fP2b9aAgqSQmrx77MIxonpQFmvkP/0yDT/+b1Pag7Cp1f1/too3HM7Jx11nO7jzZqo1kH9Uzj9P/8ptMzy9Om0ui/3dzUwSvlGBIi1QuT8eK4nbTkuIjwCdqEkX4HBi6CVSSj4QrtVEK6mJWdt6Qp0tUrCsWBT+Qo7Xytg0mSl/7CYITi9N7zcozQ0nIANPGNW01aISUXX5jprWZX5ULoKmMMryuejxoyacH67e0KqksyiUauEMJ86uwMu8rOWsA1pWZGpzMU4+Gb95+1EuUBGz9H+Kz0ODGWolOQ==".to_string(),
                p: "9VhvT/qE67WmYxqawRKE1+Px0BDLQSyTdwNEVDpoG7I9xzoyHPFf2VzhZJEcdTIl+KFRssJya9YG/j4UMMpl2xeKl7wTB1GpCLI5ITDYctAmJBdjmr7a6JThlcD6GxowFTDaj9uCRWTZ6tQdnvhBS9LkoC/MdRVgn0gzHJZMLPc=".to_string(),
                q: "1FkuXH27wBdRUg2ho8BwTDEWeW/nBPpdcbXuuiKYvx/nJrWJhmU3sCiu3+H5AA43dFHO375OS0A3OeI5SPbD7BF5EZGGwjXim80mtVUJN4p/dll8xeABToPsjDxlgh5S8c/dFx7ZKVOoHgYIPc/NvVXYkIHpbxMBwzMNjNvodHU=".to_string(),
                dmp1: "jF/Z6F/E13wqQ/+/1YH8Ae4It+wz7wlLIkf7O1njoR0dXbT9YTP1jE8pIrooFyHnOddLAEVi5DIj9Cmesb/MAUv53xEbrg9Z8IDQUR46aY6QlAvR0IMsivBMFbvBHeqg4i7+jlqgsYWfbU2J2R/fdDuo1cIjcEYX72qG2+9ejEc=".to_string(),
                dmq1: "jF/Z6F/E13wqQ/+/1YH8Ae4It+wz7wlLIkf7O1njoR0dXbT9YTP1jE8pIrooFyHnOddLAEVi5DIj9Cmesb/MAUv53xEbrg9Z8IDQUR46aY6QlAvR0IMsivBMFbvBHeqg4i7+jlqgsYWfbU2J2R/fdDuo1cIjcEYX72qG2+9ejEc=".to_string(),
                iqmp: "Kb3QDAaj40FQYBHLKC30dm1lsTuJGDfAnz+y6B5IA9VuC7fVoF8eWPWVNNUkLP3+keY/rgm3bBszgLwdmIhiNmhFv4pEO6ogBiNVt28CKlI4XXCQ2oGkMcWF6bdiSAVsUPq/IAc7918RWCiSJapmanp8e281ZHXuyQTIgBVYjKk=".to_string(),
            },
        };

        let username = String::from("Smail");
        let auth = AuthServiceImpl::generate_auth(&keys, &username).unwrap();

        AuthServiceImpl::verify_auth(&keys.public_key, &username, &auth).unwrap();
    }

    #[test]
    fn test_auth_time_expired() {
        let keys = KeyPair {
            public_key: PublicKey {
                n: "y4K0W3aMqTZTLMSJcdVHQFpotEZZCBkyKeKI4pd/npSVVPzqIz7TvQfVyCvQgWHtg9uzHqP9HhSBFvcsuam7BygxdCyeCJ8a0oIzj6dOq3IBTN9IdF4GHLnYnh2zmAEuJKgIDLrzwJl8uE3R6okMvtvI0Sd+mmZhR9lAaN9ekVbBZvYxpPc1FObHezk+z5FIe6LqxBScZXcC96+scos/j72NsnOPags4kUsAucQZVSqM5VHjpWbKR34IpQOYQxGoJEab6YH8jUnUkDlMGSctUozHc9N3RM0Cm2PA/ZbcOLVDppsHIH+gzgis6GXQotAaWlcP0M4DiyVzydG/Qgh44w==".to_string(),
                e: "AQAB".to_string(),
            },
            private_key: PrivateKey {
                d: "DiNpdkU5JnRQuPZ6ef8QMSdWyNduTgK6GnDTg7J0ukamTT444fP2b9aAgqSQmrx77MIxonpQFmvkP/0yDT/+b1Pag7Cp1f1/too3HM7Jx11nO7jzZqo1kH9Uzj9P/8ptMzy9Om0ui/3dzUwSvlGBIi1QuT8eK4nbTkuIjwCdqEkX4HBi6CVSSj4QrtVEK6mJWdt6Qp0tUrCsWBT+Qo7Xytg0mSl/7CYITi9N7zcozQ0nIANPGNW01aISUXX5jprWZX5ULoKmMMryuejxoyacH67e0KqksyiUauEMJ86uwMu8rOWsA1pWZGpzMU4+Gb95+1EuUBGz9H+Kz0ODGWolOQ==".to_string(),
                p: "9VhvT/qE67WmYxqawRKE1+Px0BDLQSyTdwNEVDpoG7I9xzoyHPFf2VzhZJEcdTIl+KFRssJya9YG/j4UMMpl2xeKl7wTB1GpCLI5ITDYctAmJBdjmr7a6JThlcD6GxowFTDaj9uCRWTZ6tQdnvhBS9LkoC/MdRVgn0gzHJZMLPc=".to_string(),
                q: "1FkuXH27wBdRUg2ho8BwTDEWeW/nBPpdcbXuuiKYvx/nJrWJhmU3sCiu3+H5AA43dFHO375OS0A3OeI5SPbD7BF5EZGGwjXim80mtVUJN4p/dll8xeABToPsjDxlgh5S8c/dFx7ZKVOoHgYIPc/NvVXYkIHpbxMBwzMNjNvodHU=".to_string(),
                dmp1: "jF/Z6F/E13wqQ/+/1YH8Ae4It+wz7wlLIkf7O1njoR0dXbT9YTP1jE8pIrooFyHnOddLAEVi5DIj9Cmesb/MAUv53xEbrg9Z8IDQUR46aY6QlAvR0IMsivBMFbvBHeqg4i7+jlqgsYWfbU2J2R/fdDuo1cIjcEYX72qG2+9ejEc=".to_string(),
                dmq1: "jF/Z6F/E13wqQ/+/1YH8Ae4It+wz7wlLIkf7O1njoR0dXbT9YTP1jE8pIrooFyHnOddLAEVi5DIj9Cmesb/MAUv53xEbrg9Z8IDQUR46aY6QlAvR0IMsivBMFbvBHeqg4i7+jlqgsYWfbU2J2R/fdDuo1cIjcEYX72qG2+9ejEc=".to_string(),
                iqmp: "Kb3QDAaj40FQYBHLKC30dm1lsTuJGDfAnz+y6B5IA9VuC7fVoF8eWPWVNNUkLP3+keY/rgm3bBszgLwdmIhiNmhFv4pEO6ogBiNVt28CKlI4XXCQ2oGkMcWF6bdiSAVsUPq/IAc7918RWCiSJapmanp8e281ZHXuyQTIgBVYjKk=".to_string(),
            },
        };

        let username = String::from("Smail");
        let decrypt_auth = format!("{},{}", username, 3);
        let auth = RsaCryptoService::encrypt_private(
            &keys,
            &DecryptedValue { secret: decrypt_auth }).unwrap().garbage;

        let result = discriminant(&AuthServiceImpl::verify_auth(&keys.public_key, &username, &auth).unwrap_err());
        let error = discriminant(&VerificationError::TimeStampOutOfBounds);

        assert_eq!(result, error);
    }

    #[test]
    fn test_auth_invalid_username() {
        let keys = KeyPair {
            public_key: PublicKey {
                n: "y4K0W3aMqTZTLMSJcdVHQFpotEZZCBkyKeKI4pd/npSVVPzqIz7TvQfVyCvQgWHtg9uzHqP9HhSBFvcsuam7BygxdCyeCJ8a0oIzj6dOq3IBTN9IdF4GHLnYnh2zmAEuJKgIDLrzwJl8uE3R6okMvtvI0Sd+mmZhR9lAaN9ekVbBZvYxpPc1FObHezk+z5FIe6LqxBScZXcC96+scos/j72NsnOPags4kUsAucQZVSqM5VHjpWbKR34IpQOYQxGoJEab6YH8jUnUkDlMGSctUozHc9N3RM0Cm2PA/ZbcOLVDppsHIH+gzgis6GXQotAaWlcP0M4DiyVzydG/Qgh44w==".to_string(),
                e: "AQAB".to_string(),
            },
            private_key: PrivateKey {
                d: "DiNpdkU5JnRQuPZ6ef8QMSdWyNduTgK6GnDTg7J0ukamTT444fP2b9aAgqSQmrx77MIxonpQFmvkP/0yDT/+b1Pag7Cp1f1/too3HM7Jx11nO7jzZqo1kH9Uzj9P/8ptMzy9Om0ui/3dzUwSvlGBIi1QuT8eK4nbTkuIjwCdqEkX4HBi6CVSSj4QrtVEK6mJWdt6Qp0tUrCsWBT+Qo7Xytg0mSl/7CYITi9N7zcozQ0nIANPGNW01aISUXX5jprWZX5ULoKmMMryuejxoyacH67e0KqksyiUauEMJ86uwMu8rOWsA1pWZGpzMU4+Gb95+1EuUBGz9H+Kz0ODGWolOQ==".to_string(),
                p: "9VhvT/qE67WmYxqawRKE1+Px0BDLQSyTdwNEVDpoG7I9xzoyHPFf2VzhZJEcdTIl+KFRssJya9YG/j4UMMpl2xeKl7wTB1GpCLI5ITDYctAmJBdjmr7a6JThlcD6GxowFTDaj9uCRWTZ6tQdnvhBS9LkoC/MdRVgn0gzHJZMLPc=".to_string(),
                q: "1FkuXH27wBdRUg2ho8BwTDEWeW/nBPpdcbXuuiKYvx/nJrWJhmU3sCiu3+H5AA43dFHO375OS0A3OeI5SPbD7BF5EZGGwjXim80mtVUJN4p/dll8xeABToPsjDxlgh5S8c/dFx7ZKVOoHgYIPc/NvVXYkIHpbxMBwzMNjNvodHU=".to_string(),
                dmp1: "jF/Z6F/E13wqQ/+/1YH8Ae4It+wz7wlLIkf7O1njoR0dXbT9YTP1jE8pIrooFyHnOddLAEVi5DIj9Cmesb/MAUv53xEbrg9Z8IDQUR46aY6QlAvR0IMsivBMFbvBHeqg4i7+jlqgsYWfbU2J2R/fdDuo1cIjcEYX72qG2+9ejEc=".to_string(),
                dmq1: "jF/Z6F/E13wqQ/+/1YH8Ae4It+wz7wlLIkf7O1njoR0dXbT9YTP1jE8pIrooFyHnOddLAEVi5DIj9Cmesb/MAUv53xEbrg9Z8IDQUR46aY6QlAvR0IMsivBMFbvBHeqg4i7+jlqgsYWfbU2J2R/fdDuo1cIjcEYX72qG2+9ejEc=".to_string(),
                iqmp: "Kb3QDAaj40FQYBHLKC30dm1lsTuJGDfAnz+y6B5IA9VuC7fVoF8eWPWVNNUkLP3+keY/rgm3bBszgLwdmIhiNmhFv4pEO6ogBiNVt28CKlI4XXCQ2oGkMcWF6bdiSAVsUPq/IAc7918RWCiSJapmanp8e281ZHXuyQTIgBVYjKk=".to_string(),
            },
        };

        let username = String::from("Smail");
        let decrypt_auth = format!("{},{}", String::from("Hamza"), 3);
        let auth = RsaCryptoService::encrypt_private(
            &keys,
            &DecryptedValue { secret: decrypt_auth }).unwrap().garbage;

        let result = discriminant(&AuthServiceImpl::verify_auth(&keys.public_key, &username, &auth).unwrap_err());
        let error = discriminant(&VerificationError::InvalidUsername);

        assert_eq!(result, error);
    }
}