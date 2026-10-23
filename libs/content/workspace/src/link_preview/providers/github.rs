//! GitHub: issues, pull requests and discussions read as `repo#n`, commits as
//! `repo@sha`, repositories as `owner/repo`.

use url::Url;

use super::{Provider, segments};

pub(super) const PROVIDER: Provider = Provider { label };

/// First path segments that are GitHub's own pages, not an owner.
const NOT_OWNERS: &[&str] = &[
    "orgs",
    "settings",
    "features",
    "topics",
    "sponsors",
    "marketplace",
    "apps",
    "login",
    "notifications",
    "explore",
    "collections",
    "search",
    "about",
    "pricing",
    "enterprise",
];

fn label(url: &Url) -> Option<String> {
    let path = segments(url, "github.com")?;
    let (owner, repo) = match path[..] {
        [owner, repo, ..] if !NOT_OWNERS.contains(&owner) => (owner, repo),
        _ => return None,
    };
    match path[2..] {
        [] => Some(format!("{owner}/{repo}")),
        ["issues" | "pull" | "discussions", n, ..] if n.parse::<u64>().is_ok() => {
            Some(format!("{repo}#{n}"))
        }
        ["commit", sha, ..] if sha.len() >= 7 && sha.chars().all(|c| c.is_ascii_hexdigit()) => {
            Some(format!("{repo}@{}", &sha[..7]))
        }
        _ => None,
    }
}
