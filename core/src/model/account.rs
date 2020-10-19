use rsa::RSAPrivateKey;
use serde::{Deserialize, Serialize};

pub type Username = String;
pub type ApiUrl = String;

#[derive(PartialEq, Debug, Serialize, Deserialize)]
pub struct Account {
    pub username: Username,
    pub api_url: ApiUrl,
    pub private_key: RSAPrivateKey,
}
