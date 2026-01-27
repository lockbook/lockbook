use serde::{Deserialize, Serialize};

use uuid::Uuid;

/// Persistant for a device across sessions, would only be destoreyd on re-download
/// or destruction of db-rs
#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Debug, Hash)]
pub struct LbID(Uuid);

impl LbID {
    pub fn generate() -> Self {
        LbID(Uuid::new_v4())
    }
}
