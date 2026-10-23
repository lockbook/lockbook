//! X (formerly Twitter): profiles and posts read as `@handle`.

use url::Url;

use super::{Provider, segments};

pub(super) const PROVIDER: Provider = Provider { label };

/// First path segments that are X's own pages, not a handle.
const NOT_HANDLES: &[&str] = &[
    "home",
    "explore",
    "search",
    "i",
    "settings",
    "notifications",
    "messages",
    "hashtag",
    "intent",
    "share",
    "login",
    "signup",
    "tos",
    "privacy",
];

fn label(url: &Url) -> Option<String> {
    let path = segments(url, "x.com").or_else(|| segments(url, "twitter.com"))?;
    match path[..] {
        [handle, ..] if !NOT_HANDLES.contains(&handle) => Some(format!("@{handle}")),
        _ => None,
    }
}
