use crate::crypto::AESKey;
use crate::file_like::FileLike;
use std::fmt::{Display, Formatter};

#[derive(PartialEq, Debug, Clone)]
pub struct LazyFile<'a, F: FileLike> {
    pub file: &'a F,
    pub name: Option<String>,
    pub key: Option<AESKey>,
    pub implicitly_deleted: Option<bool>,
}

impl<'a, F> Display for LazyFile<'a, F>
where
    F: FileLike,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display())
    }
}
