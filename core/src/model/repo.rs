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
pub enum RepoState {
    New,
    Modifed,
    Unmodified,
}
