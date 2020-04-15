use crate::crypto::KeyPair;
use serde::{Deserialize, Serialize};

#[derive(PartialEq, Debug, Serialize, Deserialize)]
pub struct Account {
    pub username: String,
    pub keys: KeyPair,
}
