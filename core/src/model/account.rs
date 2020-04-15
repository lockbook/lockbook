use rsa::RSAPrivateKey;
use serde::{Deserialize, Serialize};

#[derive(PartialEq, Debug, Serialize, Deserialize)]
pub struct Account {
    pub username: String,
    pub keys: RSAPrivateKey,
}