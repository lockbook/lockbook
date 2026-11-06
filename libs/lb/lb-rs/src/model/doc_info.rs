use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::file_metadata::{DocumentHmac, Owner};

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LocalDocInfo {
    pub remote: Option<DocHead>,
    pub base: Option<DocHead>,
    pub local: Option<LocalVersion>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default)]
pub struct DocInfo {
    pub versions: Vec<DocVersion>,
    pub retired: Vec<DocVersion>,
    pub updated_at: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub struct DocHead {
    pub hmac: DocumentHmac,
    pub stored_size: u64,
    pub content_size: Option<u64>,
    pub author: Owner,
    pub timestamp: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalVersion {
    pub hmac: DocumentHmac,
    pub compressed_size: u64,
    pub timestamp: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct DocVersion {
    pub hmac: DocumentHmac,
    pub kind: VersionKind,
    pub storage: Storage,
    pub compressed_size: u64,
    pub uncompressed_size: Option<u64>,
    pub content_size: Option<u64>,
    pub author: Owner,
    pub timestamp: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VersionKind {
    Snapshot,
    Diff { base: DocumentHmac },
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Storage {
    Legacy,
    Version(Uuid),
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BlobKey {
    Legacy { id: Uuid, hmac: DocumentHmac },
    Version { id: Uuid, version: Uuid },
}

impl LocalDocInfo {
    pub fn current_hmac(&self) -> Option<DocumentHmac> {
        self.local
            .map(|local| local.hmac)
            .or(self.base_hmac())
            .or(self.remote_hmac())
    }

    pub fn base_hmac(&self) -> Option<DocumentHmac> {
        self.base.map(|base| base.hmac)
    }

    pub fn remote_hmac(&self) -> Option<DocumentHmac> {
        self.remote.map(|remote| remote.hmac)
    }

    pub fn is_behind(&self) -> bool {
        self.remote.is_some() && self.remote_hmac() != self.base_hmac()
    }

    pub fn hmacs(&self) -> impl Iterator<Item = DocumentHmac> {
        self.remote_hmac()
            .into_iter()
            .chain(self.base_hmac())
            .chain(self.local.map(|local| local.hmac))
    }

    pub fn is_empty(&self) -> bool {
        self.remote.is_none() && self.base.is_none() && self.local.is_none()
    }
}

impl DocInfo {
    pub fn head(&self) -> Option<&DocVersion> {
        self.versions.last()
    }

    pub fn head_hmac(&self) -> Option<DocumentHmac> {
        self.head().map(|v| v.hmac)
    }

    pub fn summary(&self) -> Option<DocHead> {
        let head = self.head()?;
        Some(DocHead {
            hmac: head.hmac,
            stored_size: self.stored_size(),
            content_size: head.content_size,
            author: head.author,
            timestamp: head.timestamp,
        })
    }

    pub fn stored_size(&self) -> u64 {
        self.versions.iter().map(|v| v.compressed_size).sum()
    }

    pub fn uncompressed_size(&self) -> Option<u64> {
        self.versions.iter().map(|v| v.uncompressed_size).sum()
    }

    pub fn content_size(&self) -> Option<u64> {
        self.head().and_then(|v| v.content_size)
    }

    pub fn snapshot_size(&self) -> u64 {
        self.versions
            .iter()
            .rev()
            .find(|v| v.is_snapshot())
            .map(|v| v.compressed_size)
            .unwrap_or_default()
    }

    pub fn diff_size(&self) -> u64 {
        self.versions
            .iter()
            .rev()
            .take_while(|v| !v.is_snapshot())
            .map(|v| v.compressed_size)
            .sum()
    }

    pub fn path(
        &self, have: Option<&DocumentHmac>, want: Option<&DocumentHmac>,
    ) -> Option<&[DocVersion]> {
        let target = match want {
            Some(want) => match self.versions.iter().rposition(|v| &v.hmac == want) {
                Some(target) => target,
                None => return self.retired_path(want),
            },
            None => self.versions.len().checked_sub(1)?,
        };
        let start = have
            .and_then(|have| {
                self.versions[..target + 1]
                    .iter()
                    .rposition(|v| &v.hmac == have)
            })
            .map(|found| found + 1)
            .unwrap_or_else(|| {
                self.versions[..target + 1]
                    .iter()
                    .rposition(|v| v.is_snapshot())
                    .unwrap_or(0)
            });
        Some(&self.versions[start..target + 1])
    }

    fn retired_path(&self, want: &DocumentHmac) -> Option<&[DocVersion]> {
        let target = self.retired.iter().rposition(|v| &v.hmac == want)?;
        let start = self.retired[..target + 1]
            .iter()
            .rposition(|v| v.is_snapshot())?;
        Some(&self.retired[start..target + 1])
    }

    pub fn retire(&mut self, id: Uuid) -> Vec<BlobKey> {
        let retired = std::mem::take(&mut self.versions);
        let keys = retired.iter().map(|v| v.blob(id)).collect();
        self.retired.extend(retired);
        keys
    }

    pub fn forget_retired(&mut self, key: &BlobKey) {
        let id = key.id();
        self.retired.retain(|v| &v.blob(id) != key);
    }

    pub fn blobs(&self, id: Uuid) -> impl Iterator<Item = BlobKey> + '_ {
        self.versions.iter().map(move |v| v.blob(id))
    }

    pub fn references(&self, key: &BlobKey) -> bool {
        self.blobs(key.id()).any(|blob| &blob == key)
    }
}

impl DocVersion {
    pub fn blob(&self, id: Uuid) -> BlobKey {
        match self.storage {
            Storage::Legacy => BlobKey::Legacy { id, hmac: self.hmac },
            Storage::Version(version) => BlobKey::Version { id, version },
        }
    }

    pub fn is_snapshot(&self) -> bool {
        self.kind == VersionKind::Snapshot
    }
}

impl BlobKey {
    pub fn id(&self) -> Uuid {
        match self {
            BlobKey::Legacy { id, .. } | BlobKey::Version { id, .. } => *id,
        }
    }
}
