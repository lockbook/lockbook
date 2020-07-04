use rsa::RSAPrivateKey;
use serde::{Deserialize, Serialize};

pub type Username = String;

#[derive(PartialEq, Debug, Serialize, Deserialize)]
pub struct Account {
    pub username: Username,
    pub keys: RSAPrivateKey,
}
