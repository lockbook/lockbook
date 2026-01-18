use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use uuid::Uuid;

/// Represents an LB id, would be destoreyd on re-download or delation of the lb_id.bin file
/// used to identify the Lb instance on debug users to inform crash analytics.
#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Debug, Hash)]
pub struct LbID(Option<Uuid>);

pub const FILE_NAME: &str = "lb_id.bin";

impl LbID {
    pub fn generate() -> Self {
        LbID(Some(Uuid::new_v4()))
    }

    fn save_to_file(&self, path: &PathBuf) -> io::Result<()> {
        let encoded =
            bincode::serialize(self).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        fs::write(path, encoded)
    }

    fn load_from_file(path: &PathBuf) -> io::Result<Self> {
        let bytes = fs::read(path)?;
        bincode::deserialize(&bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    /// Load lb id from file, or generate and persist if loading fails or if it doesn't exist.
    pub fn load_or_generate<P: AsRef<Path>>(base_path: P) -> io::Result<Self> {
        let path = base_path.as_ref().join(FILE_NAME);

        match Self::load_from_file(&path) {
            Ok(id) => Ok(id),
            Err(_) => {
                let id = Self::generate();
                id.save_to_file(&path)?;
                Ok(id)
            }
        }
    }
}
