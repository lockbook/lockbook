//! Wikipedia: an article link reads as its title.

use url::Url;

use super::Provider;

pub(super) const PROVIDER: Provider = Provider { label };

fn label(url: &Url) -> Option<String> {
    if !url.host_str()?.ends_with(".wikipedia.org") {
        return None;
    }
    let title = url.path().strip_prefix("/wiki/")?;
    let title = urlencoding::decode(title).ok()?.replace('_', " ");
    Some(title).filter(|t| !t.is_empty())
}
