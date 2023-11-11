use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Hash, Clone)]
pub enum FeatureFlag {
    NewAccounts,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Hash, Clone)]
pub struct FeatureFlags {
    pub new_accounts: bool,
}

impl Default for FeatureFlags {
    fn default() -> Self {
        FeatureFlags { new_accounts: true }
    }
}
