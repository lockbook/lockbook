#[derive(Clone, Copy, PartialEq)]
pub enum RepoSource {
    Local, // files with local edits applied
    Base,  // files at latest known state when client and server matched
}

impl RepoSource {
    pub fn opposite(self: Self) -> RepoSource {
        match self {
            RepoSource::Local => RepoSource::Base,
            RepoSource::Base => RepoSource::Local,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum RepoState<T> {
    New(T),
    Modified { local: T, base: T },
    Unmodified(T),
}

impl<T> RepoState<T> {
    pub fn local(self) -> T {
        match self {
            RepoState::New(f) => f,
            RepoState::Modified { local, base: _ } => local,
            RepoState::Unmodified(f) => f,
        }
    }

    pub fn base(self) -> Option<T> {
        match self {
            RepoState::New(_) => None,
            RepoState::Modified {
                local: _,
                base: remote,
            } => Some(remote),
            RepoState::Unmodified(f) => Some(f),
        }
    }

    pub fn source(self, source: RepoSource) -> Option<T> {
        match source {
            RepoSource::Local => Some(self.local()),
            RepoSource::Base => self.base(),
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
        if let RepoState::Modified { local: _, base: _ } = self {
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

    pub fn from_local_and_base(local: Option<T>, base: Option<T>) -> Option<Self> {
        match (local, base) {
            (None, None) => None,
            (Some(local), None) => Some(RepoState::New(local)), // new files are only stored in the local repo
            (None, Some(base)) => Some(RepoState::Unmodified(base)), // unmodified files are only stored in the base repo
            (Some(local), Some(base)) => Some(RepoState::Modified { local, base }), // modified files are stored in both repos
        }
    }
}
