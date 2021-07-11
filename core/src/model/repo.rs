#[derive(Clone, Copy)]
pub enum RepoSource {
    Local,
    Remote,
}

impl RepoSource {
    pub fn opposite(self: Self) -> RepoSource {
        match self {
            RepoSource::Local => RepoSource::Remote,
            RepoSource::Remote => RepoSource::Local,
        }
    }
}

#[derive(Clone)]
pub enum RepoState<T> {
    New(T),
    Modified { local: T, remote: T },
    Unmodified(T),
}

impl<T> RepoState<T> {
    pub fn local(self) -> T {
        match self {
            RepoState::New(f) => f,
            RepoState::Modified { local, remote: _ } => local,
            RepoState::Unmodified(f) => f,
        }
    }

    pub fn remote(self) -> Option<T> {
        match self {
            RepoState::New(_) => None,
            RepoState::Modified { local: _, remote } => Some(remote),
            RepoState::Unmodified(f) => Some(f),
        }
    }

    pub fn source(self, source: RepoSource) -> Option<T> {
        match source {
            RepoSource::Local => Some(self.local()),
            RepoSource::Remote => self.remote(),
        }
    }

    pub fn is_new(&self) -> bool {
        if let RepoState::New(_) = self {
            true
        } else {
            false
        }
    }

    pub fn is_modified(&self) -> bool {
        if let RepoState::Modified {
            local: _,
            remote: _,
        } = self
        {
            true
        } else {
            false
        }
    }

    pub fn is_unmodified(&self) -> bool {
        if let RepoState::Unmodified(_) = self {
            true
        } else {
            false
        }
    }

    pub fn from_local_and_remote(local: Option<T>, remote: Option<T>) -> Option<Self> {
        match (local, remote) {
            (None, None) => None,
            (Some(local), None) => Some(RepoState::New(local)), // new files are only stored in the local repo
            (None, Some(remote)) => Some(RepoState::Unmodified(remote)), // unmodified files are only stored in the remote repo
            (Some(local), Some(remote)) => Some(RepoState::Modified { local, remote }), // modified files are stored in both repos
        }
    }
}
