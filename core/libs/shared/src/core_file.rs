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
//
// #[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
// pub struct OwnedLazyFile<F: FileLike> {
//     pub file: F,
//     name: Option<String>,
//     key: Option<AESKey>,
//     confirmed_deleted: Option<bool>,
// }
//
// impl<F> Display for OwnedLazyFile<F>
// where
//     F: FileLike,
// {
//     fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
//         write!(f, "{}", self.display())
//     }
// }
//
// pub trait LazyFile {
//     fn get_name(&self) -> &Option<String>;
//     fn set_name(&mut self, name: String);
//
//     fn get_key(&self) -> &Option<AESKey>;
//     fn set_key(&mut self, key: AESKey);
//
//     fn get_deleted_status(&self) -> &Option<bool>;
//     fn set_deleted_status(&mut self, deleted: bool);
// }
//
// impl<'a, F: FileLike> LazyFile for BorrowedLazyFile<'a, F> {
//     fn get_name(&self) -> &Option<String> {
//         &self.name
//     }
//
//     fn set_name(&mut self, name: String) {
//         self.name = Some(name);
//     }
//
//     fn get_key(&self) -> &Option<AESKey> {
//         &self.key
//     }
//
//     fn set_key(&mut self, key: AESKey) {
//         self.key = Some(key);
//     }
//
//     fn get_deleted_status(&self) -> &Option<bool> {
//         &self.confirmed_deleted
//     }
//
//     fn set_deleted_status(&mut self, deleted: bool) {
//         self.confirmed_deleted = Some(deleted)
//     }
// }
//
// impl<F: FileLike> LazyFile for OwnedLazyFile<F> {
//     fn get_name(&self) -> &Option<String> {
//         &self.name
//     }
//
//     fn set_name(&mut self, name: String) {
//         self.name = Some(name);
//     }
//
//     fn get_key(&self) -> &Option<AESKey> {
//         &self.key
//     }
//
//     fn set_key(&mut self, key: AESKey) {
//         self.key = Some(key);
//     }
//
//     fn get_deleted_status(&self) -> &Option<bool> {
//         &self.confirmed_deleted
//     }
//
//     fn set_deleted_status(&mut self, deleted: bool) {
//         self.confirmed_deleted = Some(deleted)
//     }
// }
