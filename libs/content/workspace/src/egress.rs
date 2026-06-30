//! Guarded outbound fetch for editor content — link-preview metadata and remote
//! images/favicons. Every remote GET goes through here so they share one set of
//! safety checks. These URLs arrive via synced, *shared* documents, so an
//! unguarded fetch is an SSRF vector: a shared note could point at
//! `http://169.254.169.254/…` (cloud metadata) or an intranet host.
//!
//! Guardrails (native): HTTPS-only; private/loopback/link-local/CGNAT/etc. IPs
//! blocked — the host is resolved and every address checked before the request,
//! and again on each redirect; ≤5 redirects; a request timeout; and a caller-
//! supplied body-size cap. (One known gap: there's a small resolve-then-connect
//! window — reqwest 0.11 can't pin a custom DNS resolver on a shared client.)
//! The wasm build leaves the connection to the browser (no DNS access here) and
//! enforces the checks it can.

const MAX_HTML_BYTES: u64 = 256 * 1024;
/// Generous cap for a remote image body.
pub const MAX_IMAGE_BYTES: u64 = 16 * 1024 * 1024;

#[cfg(not(target_arch = "wasm32"))]
mod imp {
    use std::io::Read as _;
    use std::net::{IpAddr, ToSocketAddrs as _};
    use std::sync::OnceLock;
    use std::time::Duration;

    use url::{Host, Url};

    use super::MAX_HTML_BYTES;

    const TIMEOUT: Duration = Duration::from_secs(5);
    const MAX_REDIRECTS: usize = 5;

    /// Reject any address that isn't a public, routable host — loopback,
    /// private, link-local (incl. cloud metadata at 169.254.169.254), CGNAT,
    /// unspecified, multicast, reserved, and the IPv4-mapped forms of those.
    fn is_blocked(ip: IpAddr) -> bool {
        match ip {
            IpAddr::V4(v4) => {
                let [a, b, ..] = v4.octets();
                v4.is_loopback()
                    || v4.is_private()
                    || v4.is_link_local()
                    || v4.is_unspecified()
                    || v4.is_broadcast()
                    || v4.is_documentation()
                    || v4.is_multicast()
                    || a == 0 // 0.0.0.0/8
                    || (a == 100 && (b & 0xc0) == 64) // 100.64.0.0/10 CGNAT
                    || a >= 240 // 240.0.0.0/4 reserved
            }
            IpAddr::V6(v6) => {
                v6.is_loopback()
                    || v6.is_unspecified()
                    || v6.is_multicast()
                    || (v6.segments()[0] & 0xfe00) == 0xfc00 // fc00::/7 unique local
                    || (v6.segments()[0] & 0xffc0) == 0xfe80 // fe80::/10 link local
                    || v6.to_ipv4_mapped().is_some_and(|m| is_blocked(IpAddr::V4(m)))
            }
        }
    }

    /// HTTPS-only, and the host must resolve only to public addresses. A host
    /// that resolves to *any* blocked address is rejected outright.
    fn validate(url: &Url) -> Result<(), String> {
        if url.scheme() != "https" {
            return Err("only https URLs are fetched".into());
        }
        let port = url.port_or_known_default().unwrap_or(443);
        let blocked = match url.host() {
            Some(Host::Ipv4(ip)) => is_blocked(IpAddr::V4(ip)),
            Some(Host::Ipv6(ip)) => is_blocked(IpAddr::V6(ip)),
            Some(Host::Domain(d)) => {
                let mut addrs = (d, port)
                    .to_socket_addrs()
                    .map_err(|e| e.to_string())?
                    .peekable();
                if addrs.peek().is_none() {
                    return Err("host did not resolve".into());
                }
                addrs.any(|a| is_blocked(a.ip()))
            }
            None => return Err("missing host".into()),
        };
        if blocked {
            return Err("blocked (non-public) address".into());
        }
        Ok(())
    }

    /// One shared client: timeout, HTTPS-only, and a redirect policy that
    /// re-validates each hop (so a redirect can't escape to http or a private
    /// address) up to `MAX_REDIRECTS`.
    fn client() -> &'static reqwest::blocking::Client {
        static CLIENT: OnceLock<reqwest::blocking::Client> = OnceLock::new();
        CLIENT.get_or_init(|| {
            let redirect = reqwest::redirect::Policy::custom(|attempt| {
                if attempt.previous().len() >= MAX_REDIRECTS {
                    attempt.stop()
                } else if validate(attempt.url()).is_ok() {
                    attempt.follow()
                } else {
                    attempt.stop()
                }
            });
            reqwest::blocking::Client::builder()
                .timeout(TIMEOUT)
                .https_only(true)
                .redirect(redirect)
                .build()
                .unwrap_or_default()
        })
    }

    fn open(url: &str, user_agent: &str) -> Result<reqwest::blocking::Response, String> {
        let parsed = Url::parse(url).map_err(|e| e.to_string())?;
        validate(&parsed)?;
        client()
            .get(url)
            .header("User-Agent", user_agent)
            .send()
            .map_err(|e| e.to_string())
    }

    #[tracing::instrument(level = "debug", name = "egress", skip_all, fields(kind = "html", url = %url))]
    pub fn fetch_html(
        _client: &reqwest::blocking::Client, url: &str, user_agent: &str,
    ) -> Result<String, String> {
        let resp = open(url, user_agent)?;
        let is_html = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|ct| ct.contains("html"));
        if !is_html {
            return Err("response is not HTML".into());
        }
        let mut buf = Vec::new();
        resp.take(MAX_HTML_BYTES)
            .read_to_end(&mut buf)
            .map_err(|e| e.to_string())?;
        Ok(String::from_utf8_lossy(&buf).into_owned())
    }

    #[tracing::instrument(level = "debug", name = "egress", skip_all, fields(kind = "bytes", url = %url))]
    pub fn fetch_bytes(
        _client: &reqwest::blocking::Client, url: &str, user_agent: &str, max: u64,
    ) -> Result<Vec<u8>, String> {
        let resp = open(url, user_agent)?;
        let status = resp.status();
        if !status.is_success() {
            return Err(format!("{} {}", status.as_u16(), status.canonical_reason().unwrap_or("")));
        }
        let mut buf = Vec::new();
        resp.take(max)
            .read_to_end(&mut buf)
            .map_err(|e| e.to_string())?;
        Ok(buf)
    }

    #[cfg(test)]
    mod tests {
        use super::{is_blocked, validate};
        use url::Url;

        fn ip(s: &str) -> std::net::IpAddr {
            s.parse().unwrap()
        }

        #[test]
        fn blocks_non_public_addresses() {
            for s in [
                "127.0.0.1",
                "10.0.0.1",
                "172.16.0.1",
                "192.168.1.1",
                "169.254.169.254", // cloud metadata
                "0.0.0.0",
                "100.64.0.1", // CGNAT
                "224.0.0.1",  // multicast
                "::1",
                "fc00::1",
                "fe80::1",
                "::ffff:192.168.0.1", // v4-mapped private
            ] {
                assert!(is_blocked(ip(s)), "{s} should be blocked");
            }
        }

        #[test]
        fn allows_public_addresses() {
            for s in ["1.1.1.1", "8.8.8.8", "140.82.112.3", "2606:4700:4700::1111"] {
                assert!(!is_blocked(ip(s)), "{s} should be allowed");
            }
        }

        #[test]
        fn validate_rejects_http_and_private_literals() {
            assert!(validate(&Url::parse("http://example.com").unwrap()).is_err());
            assert!(validate(&Url::parse("https://127.0.0.1").unwrap()).is_err());
            assert!(validate(&Url::parse("https://10.0.0.1").unwrap()).is_err());
            assert!(validate(&Url::parse("https://[::1]").unwrap()).is_err());
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod imp {
    use url::Url;

    use super::MAX_HTML_BYTES;

    /// The browser performs the connection (no DNS access here for IP filtering),
    /// so CORS / Private Network Access policies stand in; we still enforce HTTPS
    /// and a body cap.
    async fn open(
        client: &reqwest::Client, url: &str, user_agent: &str,
    ) -> Result<reqwest::Response, String> {
        let parsed = Url::parse(url).map_err(|e| e.to_string())?;
        if parsed.scheme() != "https" {
            return Err("only https URLs are fetched".into());
        }
        client
            .get(url)
            .header("User-Agent", user_agent)
            .send()
            .await
            .map_err(|e| e.to_string())
    }

    #[tracing::instrument(level = "debug", name = "egress", skip_all, fields(kind = "html", url = %url))]
    pub async fn fetch_html(
        client: &reqwest::Client, url: &str, user_agent: &str,
    ) -> Result<String, String> {
        let resp = open(client, url, user_agent).await?;
        let is_html = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|ct| ct.contains("html"));
        if !is_html {
            return Err("response is not HTML".into());
        }
        let text = resp.text().await.map_err(|e| e.to_string())?;
        Ok(text.chars().take(MAX_HTML_BYTES as usize).collect())
    }

    #[tracing::instrument(level = "debug", name = "egress", skip_all, fields(kind = "bytes", url = %url))]
    pub async fn fetch_bytes(
        client: &reqwest::Client, url: &str, user_agent: &str, max: u64,
    ) -> Result<Vec<u8>, String> {
        let resp = open(client, url, user_agent).await?;
        let status = resp.status();
        if !status.is_success() {
            return Err(format!("{} {}", status.as_u16(), status.canonical_reason().unwrap_or("")));
        }
        let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
        Ok(bytes.into_iter().take(max as usize).collect())
    }
}

pub use imp::{fetch_bytes, fetch_html};
