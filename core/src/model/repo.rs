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

pub enum RepoState {
    New,
    Modifed,
    Unmodified,
}
