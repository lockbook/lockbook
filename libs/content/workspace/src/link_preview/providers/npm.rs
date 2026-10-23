//! npm: a package's page reads as the package's name, scope included.

use url::Url;

use super::{Provider, segments};

pub(super) const PROVIDER: Provider = Provider { label };

fn label(url: &Url) -> Option<String> {
    match segments(url, "npmjs.com")?[..] {
        ["package", scope, name, ..] if scope.starts_with('@') => Some(format!("{scope}/{name}")),
        ["package", name, ..] => Some(name.to_string()),
        _ => None,
    }
}
