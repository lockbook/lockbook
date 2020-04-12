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
    use crate::auth_service::{AuthServiceImpl, AuthService};
    use crate::crypto::{CryptoService, RsaCryptoService, DecryptedValue, KeyPair, PrivateKey, PublicKey};

    #[test]
    fn test_auth_inverse_property() {
        let keys = KeyPair {
            public_key: PublicKey {
                n: "AQAB".to_string(),
                e: "tFKn0ZHzUzZYev8GPyKS39h2DbvMCBl6E7YEzNlNY65tIBfw6pP7R3+s2aslMAZJCXFFAGYr6AC3U9vO9Z3B/wNwhP5eD53ip8cVkFYFfuGkvgVTJMv+hTqYH5uZ+dQp/9aq/u4w+qvYxdfy8O8rF6xQfKHNrglbR/fIdIR/tTt/C5DiiRPHVZMnQYdEAQ3Ze0U/WmcXvNoL6tidBEM/DddZnT46yWmwj2vyy8V6sC1znfMUgCJCUEuV8unQdIBve84TloEdx5XQRtxZg/BDcR2HQoVafbY63OeQwuc9BFO0twrGFRTlBhMyXNfj+rXWm3XMm2NAcZQv3vkjto97Cw==".to_string()
            },
            private_key: PrivateKey {
                d: "X4L8YtPUt3msqhFUpLJSa4CDH0kejBe6gqBBsKNVC3yDTqF/uTCCw19MFctCGkrp+rdlXg3AKdXfROrDl3NlIwcWXUtCVTFCsa2QrW/y3z7zfLbjUDVA7h6YHv3TM/H+AQYacoeFp/DIFBsLEjUMdTCPPUSS5iEMmXUUVysrXblvM/EEgzAtA6HeFzLB0GSt8Zy6vjazWggwZY6EDW1LLGpZhZogELf8CZfv12a2vWFz7GaJ4x8I+INCpTW+SFc/ce3cfE128uAZhpX1WowGeDwFufAJGsuRH95WhYLAGiqvDvW3RzO0Hv+qfZpOJGaU4bHzR0QRb341ohBWXYvDiQ==".to_string(),
                p: "2a+z6kfP3umvsO5QTJ1iXkDbxuDbD8In1US8YpiLgKvVmVsMZVq5PT2Nayl78dJZnCLUEGAGz2+yvOD3U8GxXAxytKc08yiHIVM6qM699IFfrXrj4S83EjYHJCsYkzmrSZ4vMiO9QdaZ4h9WGmTe/qQoUk30zr4JCSkRxrmqcq0=".to_string(),
                q: "1A93ZrZqZTBjzL/WsjhpyrnV4mJV2h4c9b7YicYKjsuwzl3pXy3k7llAZ4d0KsVx9E116wvfF9Ewgjiy3mnU7Plsxa+hsJEJnxGbPsOYvHqvkOrgYVcEWPfTcvko0pE5jy82WDy/1ySPkR0xOjrDU/LMQ8liQYDOzmr9vhHvE5c=".to_string(),
                dmp1: "FIcBDEKhU3/t1V1jrRXaRNEQ6HwjrCS+5NmKejGwVf3eMovna1dWyHOZdlV/HpqbYKHYJYMooT8DN9Ru/jLxqqBx4J8z2wojU/0pNunn97qLbyx7eKyfINR/b+Wwd5GkmViVUsEUA7Vc5XnXAL4qWRDZzIkVYLmC2J5K0taHQDE=".to_string(),
                dmq1: "feE8eI06NRz3cRhDowGX0w5jZ4IGAnczq7EBKy+THtbM+oOGv8gniFEUySAAFk+kaGf+4mrmoGW+DN8JVrut+InLRsIOEhjWhEVYSXakWOXfCABU95NG8mUScMJ0uCIa7+MPuGs/Wb/LNVIF4dH2FwQeuvJ1T/rdSGz8ePJ+X7s=".to_string(),
                iqmp: "YxbRnWxOvOTgylpxYqTI2KrQB0Jx69bnjI2U1npM1P2mgOnolFcZtILbxn8o5SuoCUG6jGPc+BP79OX3x96v8pJ2K7Nd3xhVmVCwka40uHqzUR0kJ4JqmggLtQF3QiiRdlOAKWy1KXJhr65do/qIO5DnpqzEIs4LInuMQpu0+b4=".to_string()
            }
        };

        let username = String::from("Smail");
        let auth = AuthServiceImpl::generate_auth(&keys, &username).unwrap();
        AuthServiceImpl::verify_auth(&keys.public_key, &username, &auth).unwrap();
    }

    #[test]
    fn test_auth_time_expired() {
        let keys = KeyPair {
            public_key: PublicKey {
                n: "AQAB".to_string(),
                e: "tFKn0ZHzUzZYev8GPyKS39h2DbvMCBl6E7YEzNlNY65tIBfw6pP7R3+s2aslMAZJCXFFAGYr6AC3U9vO9Z3B/wNwhP5eD53ip8cVkFYFfuGkvgVTJMv+hTqYH5uZ+dQp/9aq/u4w+qvYxdfy8O8rF6xQfKHNrglbR/fIdIR/tTt/C5DiiRPHVZMnQYdEAQ3Ze0U/WmcXvNoL6tidBEM/DddZnT46yWmwj2vyy8V6sC1znfMUgCJCUEuV8unQdIBve84TloEdx5XQRtxZg/BDcR2HQoVafbY63OeQwuc9BFO0twrGFRTlBhMyXNfj+rXWm3XMm2NAcZQv3vkjto97Cw==".to_string()
            },
            private_key: PrivateKey {
                d: "X4L8YtPUt3msqhFUpLJSa4CDH0kejBe6gqBBsKNVC3yDTqF/uTCCw19MFctCGkrp+rdlXg3AKdXfROrDl3NlIwcWXUtCVTFCsa2QrW/y3z7zfLbjUDVA7h6YHv3TM/H+AQYacoeFp/DIFBsLEjUMdTCPPUSS5iEMmXUUVysrXblvM/EEgzAtA6HeFzLB0GSt8Zy6vjazWggwZY6EDW1LLGpZhZogELf8CZfv12a2vWFz7GaJ4x8I+INCpTW+SFc/ce3cfE128uAZhpX1WowGeDwFufAJGsuRH95WhYLAGiqvDvW3RzO0Hv+qfZpOJGaU4bHzR0QRb341ohBWXYvDiQ==".to_string(),
                p: "2a+z6kfP3umvsO5QTJ1iXkDbxuDbD8In1US8YpiLgKvVmVsMZVq5PT2Nayl78dJZnCLUEGAGz2+yvOD3U8GxXAxytKc08yiHIVM6qM699IFfrXrj4S83EjYHJCsYkzmrSZ4vMiO9QdaZ4h9WGmTe/qQoUk30zr4JCSkRxrmqcq0=".to_string(),
                q: "1A93ZrZqZTBjzL/WsjhpyrnV4mJV2h4c9b7YicYKjsuwzl3pXy3k7llAZ4d0KsVx9E116wvfF9Ewgjiy3mnU7Plsxa+hsJEJnxGbPsOYvHqvkOrgYVcEWPfTcvko0pE5jy82WDy/1ySPkR0xOjrDU/LMQ8liQYDOzmr9vhHvE5c=".to_string(),
                dmp1: "FIcBDEKhU3/t1V1jrRXaRNEQ6HwjrCS+5NmKejGwVf3eMovna1dWyHOZdlV/HpqbYKHYJYMooT8DN9Ru/jLxqqBx4J8z2wojU/0pNunn97qLbyx7eKyfINR/b+Wwd5GkmViVUsEUA7Vc5XnXAL4qWRDZzIkVYLmC2J5K0taHQDE=".to_string(),
                dmq1: "feE8eI06NRz3cRhDowGX0w5jZ4IGAnczq7EBKy+THtbM+oOGv8gniFEUySAAFk+kaGf+4mrmoGW+DN8JVrut+InLRsIOEhjWhEVYSXakWOXfCABU95NG8mUScMJ0uCIa7+MPuGs/Wb/LNVIF4dH2FwQeuvJ1T/rdSGz8ePJ+X7s=".to_string(),
                iqmp: "YxbRnWxOvOTgylpxYqTI2KrQB0Jx69bnjI2U1npM1P2mgOnolFcZtILbxn8o5SuoCUG6jGPc+BP79OX3x96v8pJ2K7Nd3xhVmVCwka40uHqzUR0kJ4JqmggLtQF3QiiRdlOAKWy1KXJhr65do/qIO5DnpqzEIs4LInuMQpu0+b4=".to_string()
            }
        };

        let username = String::from("Smail");
        let decrypt_auth = format!("{},{}", username, 3);
        let auth = RsaCryptoService::encrypt_private(
            &keys,
            &DecryptedValue { secret: decrypt_auth }).unwrap().garbage;

        AuthServiceImpl::verify_auth(&keys.public_key, &username, &auth).unwrap_err();
    }
}