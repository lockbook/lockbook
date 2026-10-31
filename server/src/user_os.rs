//! Analytics-only OS membership. An account can belong to several OS cohorts.
use crate::ServerState;
use crate::billing::app_store_client::AppStoreClient;
use crate::billing::google_play_client::GooglePlayClient;
use crate::billing::stripe_client::StripeClient;
use crate::document_service::DocumentService;
use crate::guard::ServerTx;
use crate::schema::ServerV6;
use lb_rs::model::file_metadata::Owner;
use serde::{Deserialize, Serialize};

// Append variants only: their indices are part of the database encoding.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum OS {
    Windows,
    Linux,
    MacOS,
    IOS,
    Android,
    Unknown,
}

impl OS {
    pub const ALL: [Self; 6] =
        [Self::Windows, Self::Linux, Self::MacOS, Self::IOS, Self::Android, Self::Unknown];

    pub fn from_header(header: Option<&str>) -> Self {
        match header {
            Some("windows") => Self::Windows,
            Some("linux") => Self::Linux,
            Some("macOS") => Self::MacOS,
            Some("iOS") => Self::IOS,
            Some("android") => Self::Android,
            _ => Self::Unknown,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Windows => "windows",
            Self::Linux => "linux",
            Self::MacOS => "macOS",
            Self::IOS => "iOS",
            Self::Android => "android",
            Self::Unknown => "unknown",
        }
    }
}

impl ServerV6 {
    /// Unknown headers aren't memberships. Accounts with no known OS are reported
    /// in the unknown cohort until a known OS is observed.
    pub fn record_user_os(&mut self, owner: Owner, os: OS) -> db_rs::errors::Result<bool> {
        if os == OS::Unknown
            || !self.accounts.contains_key(&owner)
            || self
                .owned_files
                .get(&owner)
                .is_none_or(|files| files.is_empty())
            || self
                .user_os
                .get(&owner)
                .is_some_and(|oses| oses.contains(&os))
        {
            return Ok(false);
        }
        self.user_os.insert(owner, os)
    }

    pub fn user_operating_systems(&self, owner: &Owner) -> Vec<OS> {
        let known: Vec<_> = self
            .user_os
            .get(owner)
            .into_iter()
            .flatten()
            .copied()
            .filter(|os| *os != OS::Unknown)
            .collect();
        if known.is_empty() { vec![OS::Unknown] } else { known }
    }
}

impl<S: StripeClient, A: AppStoreClient, G: GooglePlayClient, D: DocumentService>
    ServerState<S, A, G, D>
{
    pub async fn record_user_os(&self, owner: Owner, os: OS) {
        if os == OS::Unknown {
            return;
        }
        let mut db = self.index_db.lock().await;
        if db
            .schema
            .user_os
            .get(&owner)
            .is_some_and(|oses| oses.contains(&os))
        {
            return;
        }
        // Attribution must not turn an otherwise valid API request into an error.
        let result = (|| -> db_rs::errors::Result<()> {
            let mut tx = ServerTx::begin(&mut db)?;
            tx.record_user_os(owner, os)?;
            tx.end()
        })();
        if let Err(error) = result {
            tracing::error!(?error, "failed to record user OS");
        }
    }
}
