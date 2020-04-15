use serde::{Deserialize, Serialize};
use std::clone::Clone;

#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct File {
    pub id: String,
    pub content: String,
}
