use lb_rs::Uuid;
use nfs3_server::{nfs3_types::nfs3::fileid3, vfs::FileHandle};

/// Represents a file handle based on a UUID.
///
/// It's the same value as [lb_rs::model::file::File::id()]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct UuidFileHandle(Uuid);

impl UuidFileHandle {
    pub(crate) fn fileid(&self) -> fileid3 {
        self.0.as_u64_pair().0
    }
    pub(crate) fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl std::fmt::Display for UuidFileHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl FileHandle for UuidFileHandle {
    fn len(&self) -> usize {
        16
    }

    fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }

    fn from_bytes(bytes: &[u8]) -> Option<Self>
    where
        Self: Sized,
    {
        Uuid::from_slice(bytes).ok().map(Self)
    }
}

impl From<Uuid> for UuidFileHandle {
    fn from(id: Uuid) -> Self {
        Self(id)
    }
}

impl Into<Uuid> for UuidFileHandle {
    fn into(self) -> Uuid {
        self.0
    }
}

impl AsRef<Uuid> for UuidFileHandle {
    fn as_ref(&self) -> &Uuid {
        &self.0
    }
}
