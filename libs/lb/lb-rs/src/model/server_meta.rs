use super::signed_meta::SignedMeta;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct ServerMeta {
    pub file: SignedMeta,
    pub version: u64,
}

pub trait IntoServerMeta {
    fn add_time(self, version: u64) -> ServerMeta;
}

impl IntoServerMeta for SignedMeta {
    fn add_time(self, version: u64) -> ServerMeta {
        ServerMeta { file: self, version }
    }
}

// impl Display for ServerMeta {
//     fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
//         write!(f, "{}", self.display())
//     }
// }
