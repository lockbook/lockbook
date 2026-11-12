#[cfg(feature = "test-seed")]
use std::sync::{Arc, Mutex};

use libsecp256k1::SecretKey;
use rand::RngCore;
use rand::rngs::OsRng;
#[cfg(feature = "test-seed")]
use rand::{SeedableRng, rngs::StdRng};
use uuid::Uuid;

use crate::model::core_config::Config;
use crate::model::crypto::AESKey;

/// Where an instance draws the ids and keys of the files and accounts it creates: the OS, or a
/// config's seed with the `test-seed` feature, so a test that takes the same steps creates the same
/// ids and keys. Seeded keys are guessable, so only test builds can seed one.
#[derive(Clone, Default)]
pub struct Entropy {
    #[cfg(feature = "test-seed")]
    seeded: Option<Arc<Mutex<StdRng>>>,
}

impl Entropy {
    pub fn new(#[allow(unused_variables)] config: &Config) -> Self {
        Self {
            #[cfg(feature = "test-seed")]
            seeded: config
                .seed
                .map(|seed| Arc::new(Mutex::new(StdRng::seed_from_u64(seed)))),
        }
    }

    pub fn id(&self) -> Uuid {
        let mut bytes = [0u8; 16];
        self.fill(&mut bytes);
        uuid::Builder::from_random_bytes(bytes).into_uuid()
    }

    pub fn key(&self) -> AESKey {
        let mut key = [0u8; 32];
        self.fill(&mut key);
        key
    }

    pub fn secret_key(&self) -> SecretKey {
        #[cfg(feature = "test-seed")]
        if let Some(rng) = &self.seeded {
            return SecretKey::random(&mut *rng.lock().unwrap());
        }
        SecretKey::random(&mut OsRng)
    }

    fn fill(&self, bytes: &mut [u8]) {
        #[cfg(feature = "test-seed")]
        if let Some(rng) = &self.seeded {
            return rng.lock().unwrap().fill_bytes(bytes);
        }
        OsRng.fill_bytes(bytes)
    }
}
