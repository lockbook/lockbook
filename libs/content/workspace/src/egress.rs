//! Guarded outbound fetch for editor content — remote images today, link
//! previews to come. Every remote GET goes through here so they share one set
//! of safety checks.
//!
//! The threat model: notes can be *shared*, so these URLs were written by
//! someone else, while the request is made by *this* machine — which can reach
//! things the author can't: localhost services, the user's LAN, a cloud VM's
//! metadata service. An unguarded fetch would let a note aim the editor at any
//! of them (SSRF).
//!
//! Guardrails (native): HTTPS-only; the hostname is resolved *before* the
//! request and every resolved address checked against the block list below —
//! so a legit-looking domain whose DNS points somewhere private is caught —
//! and checked again on each redirect, since "public URL that redirects to a
//! private address" is the classic bypass; ≤5 redirects; a request timeout;
//! and a caller-supplied body-size cap so a hostile server can't feed us
//! gigabytes. (One known gap: reqwest 0.11 can't pin the connection to the
//! address we checked, so a DNS answer that changes between our check and
//! reqwest's own resolve — DNS rebinding — can slip through.) The wasm build
//! leaves the connection to the browser (no DNS access here) and enforces the
//! checks it can.

/// Generous cap for a remote image body.
pub const MAX_IMAGE_BYTES: u64 = 16 * 1024 * 1024;

#[cfg(not(target_arch = "wasm32"))]
mod imp {
    use std::io::Read as _;
    use std::net::{IpAddr, ToSocketAddrs as _};
    use std::sync::OnceLock;
    use std::time::Duration;

    use url::{Host, Url};

    const TIMEOUT: Duration = Duration::from_secs(5);
    const MAX_REDIRECTS: usize = 5;

    /// Allow only public, routable destinations. Every rejected range is a
    /// place that isn't the public web but *is* reachable from this machine —
    /// exactly what a hostile URL in a shared note would aim for.
    fn is_blocked(ip: IpAddr) -> bool {
        match ip {
            IpAddr::V4(v4) => {
                let [a, b, ..] = v4.octets();
                v4.is_loopback() // this machine: unauthenticated dev servers, admin UIs
                    || v4.is_private() // the LAN: routers, NAS, internal sites
                    // 169.254.x.x — incl. 169.254.169.254, where cloud VMs
                    // serve instance credentials to anyone who asks from inside
                    || v4.is_link_local()
                    || v4.is_unspecified()
                    || v4.is_broadcast()
                    || v4.is_documentation()
                    || v4.is_multicast()
                    || a == 0 // 0.0.0.0/8 — behaves like localhost on some systems
                    || (a == 100 && (b & 0xc0) == 64) // 100.64.0.0/10 — ISP-internal (CGNAT), Tailscale
                    || a >= 240 // 240.0.0.0/4 — reserved; nothing legitimate lives here
            }
            IpAddr::V6(v6) => {
                v6.is_loopback()
                    || v6.is_unspecified()
                    || v6.is_multicast()
                    || (v6.segments()[0] & 0xfe00) == 0xfc00 // fc00::/7 — IPv6's private range
                    || (v6.segments()[0] & 0xffc0) == 0xfe80 // fe80::/10 — IPv6 link-local
                    // ::ffff:a.b.c.d is an IPv4 address in IPv6 clothing; re-check
                    // it as IPv4 or ::ffff:127.0.0.1 walks past every rule above
                    || v6.to_ipv4_mapped().is_some_and(|m| is_blocked(IpAddr::V4(m)))
            }
        }
    }

    /// HTTPS-only, and the host must resolve only to public addresses — the
    /// URL itself never has to contain a suspicious IP, so the check is on
    /// what the name *resolves to*. A host that resolves to *any* blocked
    /// address is rejected outright.
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
    /// re-validates each hop up to `MAX_REDIRECTS` — otherwise a public URL
    /// could 302 to a private address and the client would follow it for us.
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

pub use imp::fetch_bytes;
