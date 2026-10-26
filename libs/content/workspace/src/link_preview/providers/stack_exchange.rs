//! Stack Overflow and the other Stack Exchange sites: their pages refuse
//! fetchers, so questions come from the Stack Exchange API (no key; 300
//! requests a day).

use serde_json::Value;
use url::Url;

use super::{Provider, compact, html_text, joined, segments};
use crate::link_preview::{Label, LinkMeta, Status};

pub(super) const PROVIDER: Provider = Provider { label: |_| None, endpoint, parse };

/// The API's name for the site at `host`.
fn site(host: &str) -> Option<String> {
    let host = host.trim_start_matches("www.");
    match host {
        "stackoverflow.com" | "superuser.com" | "serverfault.com" | "askubuntu.com" => {
            Some(host.trim_end_matches(".com").into())
        }
        "mathoverflow.net" => Some(host.into()),
        _ => host.strip_suffix(".stackexchange.com").map(Into::into),
    }
}

fn endpoint(url: &Url) -> Option<String> {
    let host = url.host_str()?;
    let site = site(host)?;
    let id = match segments(url, host.trim_start_matches("www."))?[..] {
        ["questions" | "q", id, ..] => id.parse::<u64>().ok()?,
        _ => return None,
    };
    Some(format!("https://api.stackexchange.com/2.3/questions/{id}?site={site}"))
}

fn parse(v: &Value, url: &Url) -> Option<LinkMeta> {
    let q = &v["items"][0];
    let site = site(url.host_str()?)?;
    let votes = q["score"].as_i64()?;
    let votes = if votes >= 0 { compact(votes as u64) } else { votes.to_string() };
    let answers = match q["answer_count"].as_u64()? {
        1 => "1 answer".to_string(),
        n => format!("{n} answers"),
    };
    let answered = q["is_answered"] == true || !q["accepted_answer_id"].is_null();
    let labels = q["tags"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|tag| Some(Label { name: tag.as_str()?.into(), rgb: None }))
        .collect();
    Some(LinkMeta {
        title: html_text(q["title"].as_str()?),
        status: answered.then_some(Status::Answered),
        byline: joined([Some(format!("{votes} votes")), Some(answers)]),
        labels,
        site_name: Some(if site == "stackoverflow" {
            "Stack Overflow".into()
        } else {
            site.clone()
        }),
        favicon_url: Some(format!("https://cdn.sstatic.net/Sites/{site}/Img/favicon.ico")),
        ..Default::default()
    })
}
