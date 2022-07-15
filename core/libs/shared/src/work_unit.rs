// use crate::core_file::BorrowedLazyFile;
// use serde::{Deserialize, Serialize};
//
// #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
// #[serde(tag = "tag", content = "content")]
// pub enum WorkUnit {
//     LocalChange { metadata: BorrowedLazyFile },
//     ServerChange { metadata: BorrowedLazyFile },
// }
//
// impl WorkUnit {
//     pub fn get_metadata(&self) -> BorrowedLazyFile {
//         match self {
//             WorkUnit::LocalChange { metadata } => metadata,
//             WorkUnit::ServerChange { metadata } => metadata,
//         }
//         .clone()
//     }
// }
//
// #[derive(Debug, Serialize, Clone)]
// pub enum ClientWorkUnit {
//     PullMetadata,
//     PushMetadata,
//     PullDocument(String),
//     PushDocument(String),
// }
