use crate::crypto::KeyPair;

#[derive(PartialEq, Debug)]
pub struct Account {
    pub username: String,
    pub keys: KeyPair,
}