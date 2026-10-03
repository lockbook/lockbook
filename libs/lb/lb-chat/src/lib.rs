//! The chat driver. Given a store holding a `.chat` document, a provider, and
//! a toolset, it runs turns: each finished completion and each returned tool
//! call is appended to the document as a settled line; nothing in flight is
//! ever written. The driver has no UI; a tab, a CLI, or a daemon drives it
//! through commands and watches it through events.

pub mod context;
pub mod driver;
pub mod models;
pub mod provider;
pub mod store;
pub mod territory;
pub mod tools;
pub mod transcribe;
pub mod vault;
mod voice;
pub mod web;
pub mod wire;

#[cfg(test)]
pub(crate) mod mock;

pub use driver::{Cmd, Driver, Event, Handle};
pub use models::{ModelInfo, list_models, list_models_blocking, prettify, window};
pub use provider::{FAVORITES, Kind, Place, Provider, friendly_name, host, place};
pub use store::{LbStore, MemStore, SharedStore, Store};
pub use territory::Territory;
pub use tools::{NoTools, ToolOutcome, Tools};
pub use vault::VaultTools;
pub use wire::{Call, ToolSchema};
