//! crates.io: a crate's page reads as the crate's name.

use url::Url;

use super::{Provider, segments};

pub(super) const PROVIDER: Provider = Provider { label };

fn label(url: &Url) -> Option<String> {
    match segments(url, "crates.io")?[..] {
        ["crates", name, ..] => Some(name.to_string()),
        _ => None,
    }
}
