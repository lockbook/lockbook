use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::{
    model::{
        account::Account,
        crypto::AESKey,
        errors::{LbErrKind, LbResult},
    },
    LbServer,
};
use crate::Lb;
use libsecp256k1::PublicKey;
use serde::{ser::SerializeMap, Deserialize, Deserializer, Serialize, Serializer};
use tokio::sync::OnceCell;
use uuid::Uuid;

pub type KeyCache = Arc<RwLock<HashMap<Uuid, AESKey>>>;

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct Keychain {
    #[serde(serialize_with = "serialize_key_cache", deserialize_with = "deserialize_key_cache")]
    key_cache: KeyCache,

    #[serde(serialize_with = "serialize_once_cell", deserialize_with = "deserialize_once_cell")]
    account: Arc<OnceCell<Account>>,

    #[serde(serialize_with = "serialize_once_cell", deserialize_with = "deserialize_once_cell")]
    public_key: Arc<OnceCell<PublicKey>>,
}

fn serialize_key_cache<S>(cache: &KeyCache, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let cache_read = cache.read().unwrap();
    let map = &*cache_read;
    let mut map_serializer = serializer.serialize_map(Some(map.len()))?;

    for (key, value) in map {
        map_serializer.serialize_entry(key, value)?;
    }

    map_serializer.end()
}

fn deserialize_key_cache<'de, D>(deserializer: D) -> Result<KeyCache, D::Error>
where
    D: Deserializer<'de>,
{
    let map: Option<HashMap<Uuid, AESKey>> = Option::deserialize(deserializer)?;

    match map {
        Some(map) => Ok(Arc::new(RwLock::new(map))),
        None => Ok(Arc::new(RwLock::new(HashMap::new()))), // Default to empty HashMap
    }
}

fn serialize_once_cell<S, T>(cell: &Arc<OnceCell<T>>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
    T: Serialize,
{
    if let Some(value) = cell.get() {
        value.serialize(serializer)
    } else {
        serializer.serialize_none()
    }
}

fn deserialize_once_cell<'de, D, T>(deserializer: D) -> Result<Arc<OnceCell<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    let value: Option<T> = Option::deserialize(deserializer)?;
    let cell = OnceCell::new();

    if let Some(v) = value {
        cell.set(v).map_err(serde::de::Error::custom)?;
    }

    Ok(Arc::new(cell))
}

impl From<Option<&Account>> for Keychain {
    fn from(value: Option<&Account>) -> Self {
        match value {
            Some(account) => {
                let account = account.clone();
                let pk = account.public_key();
                let key_cache = Default::default();

                Self {
                    account: Arc::new(OnceCell::from(account)),
                    public_key: Arc::new(OnceCell::from(pk)),
                    key_cache,
                }
            }
            None => Self::default(),
        }
    }
}

impl LbServer {
    pub fn get_account(&self) -> LbResult<&Account> {
        self.keychain.get_account()
    }

    pub fn get_keychain(&self) -> Keychain {
        self.keychain.clone()
    }
}

impl Keychain {
    pub fn get_account(&self) -> LbResult<&Account> {
        self.account
            .get()
            .ok_or_else(|| LbErrKind::AccountNonexistent.into())
    }

    pub fn get_pk(&self) -> LbResult<PublicKey> {
        self.public_key
            .get()
            .copied()
            .ok_or_else(|| LbErrKind::AccountNonexistent.into())
    }

    #[doc(hidden)]
    pub async fn cache_account(&self, account: Account) -> LbResult<()> {
        let pk = account.public_key();
        self.account
            .set(account)
            .map_err(|_| LbErrKind::AccountExists)?;
        self.public_key
            .set(pk)
            .map_err(|_| LbErrKind::AccountExists)?;

        Ok(())
    }

    pub fn contains_aes_key(&self, id: &Uuid) -> LbResult<bool> {
        Ok(self.key_cache.read()?.contains_key(id))
    }

    pub fn insert_aes_key(&self, id: Uuid, key: AESKey) -> LbResult<()> {
        self.key_cache.write()?.insert(id, key);
        Ok(())
    }

    pub fn get_aes_key(&self, id: &Uuid) -> LbResult<Option<AESKey>> {
        Ok(self.key_cache.read()?.get(id).copied())
    }
}
