use crate::Lb;
use crate::io::docs::LocalDocuments;
use crate::io::server::Server;
use crate::model::api::GetUsageRequest;
use crate::model::errors::LbResult;
use crate::model::file_metadata::Owner;
use crate::model::tree_like::TreeLike;
use crate::model::usage::bytes_to_human;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq)]
pub struct UsageMetrics {
    pub synced: UsageItemMetric,
    pub local: UsageItemMetric,
    pub added: UsageItemMetric,
    pub freed: UsageItemMetric,
    pub data_cap: Option<UsageItemMetric>,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone)]
pub struct UsageItemMetric {
    pub exact: u64,
    pub readable: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub struct UsageEntry {
    pub id: Uuid,
    pub synced: u64,
    pub local: u64,
}

impl From<u64> for UsageItemMetric {
    fn from(exact: u64) -> Self {
        Self { exact, readable: bytes_to_human(exact) }
    }
}

impl Default for UsageItemMetric {
    fn default() -> Self {
        Self::from(0)
    }
}

impl UsageMetrics {
    pub fn new(entries: &[UsageEntry], data_cap: Option<u64>) -> Self {
        let (mut synced, mut added, mut freed) = (0, 0, 0);
        for entry in entries {
            synced += entry.synced;
            added += entry.local.saturating_sub(entry.synced);
            freed += entry.synced.saturating_sub(entry.local);
        }

        Self {
            synced: synced.into(),
            local: (synced + added - freed).into(),
            added: added.into(),
            freed: freed.into(),
            data_cap: data_cap.map(Into::into),
        }
    }

    pub fn over_cap(&self) -> bool {
        self.data_cap
            .as_ref()
            .is_some_and(|cap| self.local.exact > cap.exact)
    }
}

impl<S: Server, D: LocalDocuments> Lb<S, D> {
    #[instrument(level = "debug", skip(self))]
    pub async fn get_usage(&self) -> LbResult<UsageMetrics> {
        let entries = self.get_usage_breakdown().await?;
        let data_cap = self.status.data_cap().await;
        Ok(UsageMetrics::new(&entries, data_cap))
    }

    #[instrument(level = "debug", skip(self))]
    pub async fn get_usage_breakdown(&self) -> LbResult<Vec<UsageEntry>> {
        let owner = Owner(self.keychain.get_pk()?);
        let tx = self.ro_tx().await;
        let db = tx.db();

        let synced = (&db.base_metadata).to_lazy().usage_by_file(owner)?;
        let local = (&db.base_metadata)
            .to_staged(&db.local_metadata)
            .to_lazy()
            .usage_by_file(owner)?;

        let mut entries: Vec<UsageEntry> = local
            .iter()
            .map(|(&id, &local)| UsageEntry {
                id,
                synced: synced.get(&id).copied().unwrap_or_default(),
                local,
            })
            .collect();
        entries.extend(
            synced
                .iter()
                .filter(|(id, _)| !local.contains_key(id))
                .map(|(&id, &synced)| UsageEntry { id, synced, local: 0 }),
        );

        Ok(entries)
    }

    #[instrument(level = "debug", skip(self))]
    pub async fn refresh_usage(&self) -> LbResult<UsageMetrics> {
        let account = self.get_account()?;
        let response = self.client.get_usage(account, GetUsageRequest {}).await?;
        self.status.set_data_cap(response.cap).await;
        let usage = self.get_usage().await?;
        self.set_status_usage(usage.clone()).await;
        Ok(usage)
    }
}
