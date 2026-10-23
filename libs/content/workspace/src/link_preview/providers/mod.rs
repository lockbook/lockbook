//! Sites a preview knows by name, one file each.

mod crates;
mod github;
mod npm;
mod wikipedia;
mod x;

use url::Url;

struct Provider {
    /// The chip's text when the URL alone says what the link is.
    label: fn(&Url) -> Option<String>,
}

const PROVIDERS: &[Provider] =
    &[github::PROVIDER, wikipedia::PROVIDER, x::PROVIDER, crates::PROVIDER, npm::PROVIDER];

/// The chip text a provider reads off `url` (`lockbook#5188`, `@jack`). It
/// needs no fetch, so it shows with previews off too.
pub fn label(url: &str) -> Option<String> {
    let url = Url::parse(url).ok()?;
    PROVIDERS.iter().find_map(|p| (p.label)(&url))
}

/// `url`'s path segments when its host is `host` or a `www.` of it.
fn segments<'a>(url: &'a Url, host: &str) -> Option<Vec<&'a str>> {
    let h = url.host_str()?;
    if h != host && h.strip_prefix("www.") != Some(host) {
        return None;
    }
    Some(url.path_segments()?.filter(|s| !s.is_empty()).collect())
}

#[cfg(test)]
mod tests {
    use super::label;

    #[test]
    fn labels_from_urls() {
        let cases = [
            ("https://github.com/lockbook/lockbook/issues/5188", Some("lockbook#5188")),
            ("https://github.com/lockbook/lockbook/pull/5229/files", Some("lockbook#5229")),
            (
                "https://github.com/lockbook/lockbook/pull/5229#issuecomment-1",
                Some("lockbook#5229"),
            ),
            ("https://github.com/lockbook/lockbook/discussions/12", Some("lockbook#12")),
            (
                "https://github.com/lockbook/lockbook/commit/dd32572ee0a1b2c3",
                Some("lockbook@dd32572"),
            ),
            ("https://github.com/lockbook/lockbook", Some("lockbook/lockbook")),
            ("https://github.com/lockbook/lockbook/", Some("lockbook/lockbook")),
            ("https://www.github.com/lockbook/lockbook/issues/1", Some("lockbook#1")),
            ("https://github.com/lockbook/lockbook/blob/master/README.md", None),
            ("https://github.com/lockbook/lockbook/issues", None),
            ("https://github.com/lockbook/lockbook/issues/new", None),
            ("https://github.com/orgs/lockbook/projects", None),
            ("https://github.com/lockbook", None),
            ("https://gist.github.com/lockbook/lockbook", None),
            ("https://notgithub.com/lockbook/lockbook/issues/1", None),
            (
                "https://en.wikipedia.org/wiki/Conflict-free_replicated_data_type",
                Some("Conflict-free replicated data type"),
            ),
            (
                "https://en.wikipedia.org/wiki/Ramer%E2%80%93Douglas%E2%80%93Peucker_algorithm",
                Some("Ramer–Douglas–Peucker algorithm"),
            ),
            ("https://en.m.wikipedia.org/wiki/Markdown#History", Some("Markdown")),
            ("https://en.wikipedia.org/w/index.php?title=Markdown", None),
            ("https://x.com/jack/status/20", Some("@jack")),
            ("https://twitter.com/rustlang", Some("@rustlang")),
            ("https://x.com/home", None),
            ("https://x.com/i/status/20", None),
            ("https://crates.io/crates/serde", Some("serde")),
            ("https://crates.io/crates/serde/1.0.228", Some("serde")),
            ("https://crates.io/search?q=serde", None),
            ("https://www.npmjs.com/package/react", Some("react")),
            ("https://www.npmjs.com/package/@tanstack/query-core", Some("@tanstack/query-core")),
            ("https://example.com/lockbook/lockbook/issues/1", None),
        ];
        for (url, want) in cases {
            assert_eq!(label(url).as_deref(), want, "{url}");
        }
    }
}
