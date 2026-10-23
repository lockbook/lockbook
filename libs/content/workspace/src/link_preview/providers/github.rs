//! GitHub: issues, pull requests and discussions read as `repo#n`, commits as
//! `repo@sha`, repositories as `owner/repo`. Issues and pull requests come
//! from the REST API (no token; 60 requests an hour), so their cards say
//! where they stand.

use serde_json::Value;
use url::Url;

use super::{Provider, joined, segments, text};
use crate::link_preview::{Label, LinkMeta, Status};

pub(super) const PROVIDER: Provider = Provider { label, endpoint, parse };

const FAVICON: &str = "https://github.githubassets.com/favicons/favicon.png";

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

fn repo(url: &Url) -> Option<(Vec<&str>, &str, &str)> {
    let path = segments(url, "github.com")?;
    let (owner, repo) = match path[..] {
        [owner, repo, ..] if !NOT_OWNERS.contains(&owner) => (owner, repo),
        _ => return None,
    };
    Some((path, owner, repo))
}

fn label(url: &Url) -> Option<String> {
    let (path, owner, repo) = repo(url)?;
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

fn endpoint(url: &Url) -> Option<String> {
    let (path, owner, repo) = repo(url)?;
    let api = match path[2..] {
        ["issues", n, ..] if n.parse::<u64>().is_ok() => format!("issues/{n}"),
        ["pull", n, ..] if n.parse::<u64>().is_ok() => format!("pulls/{n}"),
        _ => return None,
    };
    Some(format!("https://api.github.com/repos/{owner}/{repo}/{api}"))
}

fn parse(v: &Value, url: &Url) -> Option<LinkMeta> {
    let (path, owner, repo) = self::repo(url)?;
    let n = path.get(3)?;
    let pull = v.get("merged").is_some() || v.get("pull_request").is_some();
    let merged = v["merged"] == true || !v["pull_request"]["merged_at"].is_null();
    let status = match (v["state"].as_str()?, pull) {
        ("open", true) if v["draft"] == true => Status::PullDraft,
        ("open", true) => Status::PullOpen,
        ("open", false) => Status::IssueOpen,
        (_, true) if merged => Status::PullMerged,
        (_, true) => Status::PullClosed,
        _ if v["state_reason"] == "not_planned" => Status::IssueNotPlanned,
        _ => Status::IssueClosed,
    };
    let count = |n: &Value, one: &str, many: &str| match n.as_u64()? {
        0 => None,
        1 => Some(format!("1 {one}")),
        n => Some(format!("{n} {many}")),
    };
    let changes = match (v["additions"].as_u64(), v["deletions"].as_u64()) {
        (Some(add), Some(del)) => Some((add, del)),
        _ => None,
    };
    let at = match status {
        Status::PullMerged => &v["merged_at"],
        Status::PullClosed | Status::IssueClosed | Status::IssueNotPlanned => &v["closed_at"],
        _ => &v["created_at"],
    };
    let at = chrono::DateTime::parse_from_rfc3339(at.as_str().unwrap_or_default()).ok();
    let byline = joined([text(&v["user"]["login"]), count(&v["comments"], "comment", "comments")]);
    // a 48 px picture, small enough to sit beside a name
    let avatar_url = text(&v["user"]["avatar_url"]).map(|url| {
        let sep = if url.contains('?') { '&' } else { '?' };
        format!("{url}{sep}s=48")
    });
    let labels = v["labels"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|label| {
            let rgb = u32::from_str_radix(label["color"].as_str()?, 16).ok();
            let rgb = rgb.map(|c| [(c >> 16) as u8, (c >> 8) as u8, c as u8]);
            Some(Label { name: text(&label["name"])?, rgb })
        })
        .collect();
    // a pull request's body's first paragraph, its markdown left as written
    let description = v["body"].as_str().filter(|_| pull).and_then(|body| {
        let first = body.split("\n\n").map(str::trim).find(|p| !p.is_empty())?;
        Some(first.split_whitespace().collect::<Vec<_>>().join(" "))
    });
    Some(LinkMeta {
        title: text(&v["title"])?,
        description,
        site_name: Some(format!("{owner}/{repo} #{n}")),
        favicon_url: Some(FAVICON.into()),
        status: Some(status),
        byline,
        avatar_url,
        changes,
        changed_files: v["changed_files"].as_u64(),
        at: at.map(|at| at.timestamp()),
        labels,
        ..Default::default()
    })
}
