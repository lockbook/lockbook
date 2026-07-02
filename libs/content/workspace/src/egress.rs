//! Guarded outbound fetch for editor content — link-preview metadata and
//! remote images/favicons. Every remote GET goes through here so they share
//! one set of safety checks.
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
//!
//! Rate limits: a 429/503 puts the *host* on cooldown until a next-retry
//! timestamp — `Retry-After` when the server sends one (clamped), else
//! exponential backoff with jitter. While cooling, fetches to that host fail
//! fast as [`FetchError::RateLimited`] without touching the network; callers
//! decide when to re-attempt by comparing against `until`.

use std::collections::HashMap;
use std::fmt;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use web_time::Instant;

const MAX_HTML_BYTES: u64 = 256 * 1024;
/// Generous cap for a remote image body.
pub const MAX_IMAGE_BYTES: u64 = 16 * 1024 * 1024;

/// First backoff step when a rate-limited response carries no `Retry-After`.
const COOLDOWN_BASE: Duration = Duration::from_secs(15);
/// Ceiling for any cooldown — including server-supplied `Retry-After`, which
/// is remote input and shouldn't be able to park a host for hours.
const MAX_COOLDOWN: Duration = Duration::from_secs(10 * 60);

/// Why a fetch failed — split by what a caller should do about it.
#[derive(Clone, Debug)]
pub enum FetchError {
    /// The host answered 429/503 (or is still cooling down from one).
    /// `until` is the next desired retry time.
    RateLimited { until: Instant },
    /// Deterministic failure (blocked URL, 4xx, not HTML) — retrying won't help.
    Permanent(String),
    /// Network-level failure (timeout, reset). Not currently retried.
    Transient(String),
}

impl fmt::Display for FetchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FetchError::RateLimited { .. } => write!(f, "rate limited"),
            FetchError::Permanent(msg) | FetchError::Transient(msg) => write!(f, "{msg}"),
        }
    }
}

/// Per-host cooldown state: the next-retry timestamp plus a strike count that
/// drives the backoff schedule when the server doesn't name its own delay.
/// Strikes persist across expired cooldowns (repeat offenders back off harder)
/// and reset on the first successful response.
pub(crate) struct Cooldowns {
    hosts: Mutex<HashMap<String, (Instant, u32)>>,
}

impl Cooldowns {
    pub(crate) fn new() -> Self {
        Self { hosts: Mutex::new(HashMap::new()) }
    }

    /// `Err(RateLimited { until })` while `now` is before the host's
    /// next-retry timestamp; `Ok` otherwise (including unknown hosts).
    pub(crate) fn check(&self, host: &str, now: Instant) -> Result<(), FetchError> {
        match self.hosts.lock().unwrap().get(host) {
            Some(&(until, _)) if now < until => Err(FetchError::RateLimited { until }),
            _ => Ok(()),
        }
    }

    /// Record a 429/503: the host's next-retry timestamp becomes `now` +
    /// `Retry-After` when given, else an exponentially-backed-off, jittered
    /// delay. Either way the delay is clamped to [`MAX_COOLDOWN`].
    pub(crate) fn record_rate_limited(
        &self, host: &str, retry_after: Option<Duration>, now: Instant,
    ) -> Instant {
        let mut hosts = self.hosts.lock().unwrap();
        let strikes = hosts.get(host).map(|&(_, s)| s).unwrap_or(0);
        let delay = retry_after
            .unwrap_or_else(|| backoff_delay(strikes))
            .min(MAX_COOLDOWN);
        let until = now + delay;
        hosts.insert(host.to_string(), (until, strikes.saturating_add(1)));
        until
    }

    /// Any successful response clears the host's strikes.
    pub(crate) fn record_success(&self, host: &str) {
        self.hosts.lock().unwrap().remove(host);
    }
}

/// `COOLDOWN_BASE << strikes`, jittered into `[d/2, d]` so our own retries
/// don't re-synchronize into bursts, capped at [`MAX_COOLDOWN`].
fn backoff_delay(strikes: u32) -> Duration {
    let d = COOLDOWN_BASE
        .saturating_mul(1u32 << strikes.min(6))
        .min(MAX_COOLDOWN);
    d.div_f64(2.0) + d.div_f64(2.0).mul_f64(rand::random::<f64>())
}

fn cooldowns() -> &'static Cooldowns {
    static COOLDOWNS: OnceLock<Cooldowns> = OnceLock::new();
    COOLDOWNS.get_or_init(Cooldowns::new)
}

/// Parse a `Retry-After` header value. Only the delta-seconds form; the rare
/// HTTP-date form falls back to the backoff schedule (`None`).
fn parse_retry_after(value: &str) -> Option<Duration> {
    let secs: u64 = value.trim().parse().ok()?;
    Some(Duration::from_secs(secs))
}

#[cfg(not(target_arch = "wasm32"))]
mod imp {
    use std::io::Read as _;
    use std::net::{IpAddr, ToSocketAddrs as _};
    use std::sync::OnceLock;
    use std::time::Duration;

    use url::{Host, Url};
    use web_time::Instant;

    use super::{FetchError, MAX_HTML_BYTES, cooldowns, parse_retry_after};

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

    fn open(url: &str, user_agent: &str) -> Result<reqwest::blocking::Response, FetchError> {
        let parsed = Url::parse(url).map_err(|e| FetchError::Permanent(e.to_string()))?;
        let host = parsed.host_str().unwrap_or_default().to_string();
        cooldowns().check(&host, Instant::now())?;
        validate(&parsed).map_err(FetchError::Permanent)?;
        let resp = client()
            .get(url)
            .header("User-Agent", user_agent)
            .send()
            .map_err(|e| FetchError::Transient(e.to_string()))?;

        let status = resp.status();
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS
            || status == reqwest::StatusCode::SERVICE_UNAVAILABLE
        {
            let retry_after = resp
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|v| v.to_str().ok())
                .and_then(parse_retry_after);
            let until = cooldowns().record_rate_limited(&host, retry_after, Instant::now());
            return Err(FetchError::RateLimited { until });
        }
        // any other response — even a 4xx — means the host isn't throttling us
        cooldowns().record_success(&host);
        Ok(resp)
    }

    #[tracing::instrument(level = "debug", name = "egress", skip_all, fields(kind = "html", url = %url))]
    pub fn fetch_html(
        _client: &reqwest::blocking::Client, url: &str, user_agent: &str,
    ) -> Result<String, FetchError> {
        let resp = open(url, user_agent)?;
        let is_html = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|ct| ct.contains("html"));
        if !is_html {
            return Err(FetchError::Permanent("response is not HTML".into()));
        }
        let mut buf = Vec::new();
        resp.take(MAX_HTML_BYTES)
            .read_to_end(&mut buf)
            .map_err(|e| FetchError::Transient(e.to_string()))?;
        Ok(String::from_utf8_lossy(&buf).into_owned())
    }

    #[tracing::instrument(level = "debug", name = "egress", skip_all, fields(kind = "bytes", url = %url))]
    pub fn fetch_bytes(
        _client: &reqwest::blocking::Client, url: &str, user_agent: &str, max: u64,
    ) -> Result<Vec<u8>, FetchError> {
        let resp = open(url, user_agent)?;
        let status = resp.status();
        if !status.is_success() {
            return Err(FetchError::Permanent(format!(
                "{} {}",
                status.as_u16(),
                status.canonical_reason().unwrap_or("")
            )));
        }
        let mut buf = Vec::new();
        resp.take(max)
            .read_to_end(&mut buf)
            .map_err(|e| FetchError::Transient(e.to_string()))?;
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
    use web_time::Instant;

    use super::{FetchError, MAX_HTML_BYTES, cooldowns, parse_retry_after};

    /// The browser performs the connection (no DNS access here for IP filtering),
    /// so CORS / Private Network Access policies stand in; we still enforce HTTPS
    /// and a body cap. CORS usually hides `Retry-After` from us, so rate-limited
    /// hosts mostly ride the backoff schedule here.
    async fn open(
        client: &reqwest::Client, url: &str, user_agent: &str,
    ) -> Result<reqwest::Response, FetchError> {
        let parsed = Url::parse(url).map_err(|e| FetchError::Permanent(e.to_string()))?;
        if parsed.scheme() != "https" {
            return Err(FetchError::Permanent("only https URLs are fetched".into()));
        }
        let host = parsed.host_str().unwrap_or_default().to_string();
        cooldowns().check(&host, Instant::now())?;
        let resp = client
            .get(url)
            .header("User-Agent", user_agent)
            .send()
            .await
            .map_err(|e| FetchError::Transient(e.to_string()))?;

        let status = resp.status();
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS
            || status == reqwest::StatusCode::SERVICE_UNAVAILABLE
        {
            let retry_after = resp
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|v| v.to_str().ok())
                .and_then(parse_retry_after);
            let until = cooldowns().record_rate_limited(&host, retry_after, Instant::now());
            return Err(FetchError::RateLimited { until });
        }
        cooldowns().record_success(&host);
        Ok(resp)
    }

    #[tracing::instrument(level = "debug", name = "egress", skip_all, fields(kind = "html", url = %url))]
    pub async fn fetch_html(
        client: &reqwest::Client, url: &str, user_agent: &str,
    ) -> Result<String, FetchError> {
        let resp = open(client, url, user_agent).await?;
        let is_html = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|ct| ct.contains("html"));
        if !is_html {
            return Err(FetchError::Permanent("response is not HTML".into()));
        }
        let text = resp
            .text()
            .await
            .map_err(|e| FetchError::Transient(e.to_string()))?;
        Ok(text.chars().take(MAX_HTML_BYTES as usize).collect())
    }

    #[tracing::instrument(level = "debug", name = "egress", skip_all, fields(kind = "bytes", url = %url))]
    pub async fn fetch_bytes(
        client: &reqwest::Client, url: &str, user_agent: &str, max: u64,
    ) -> Result<Vec<u8>, FetchError> {
        let resp = open(client, url, user_agent).await?;
        let status = resp.status();
        if !status.is_success() {
            return Err(FetchError::Permanent(format!(
                "{} {}",
                status.as_u16(),
                status.canonical_reason().unwrap_or("")
            )));
        }
        let bytes = resp
            .bytes()
            .await
            .map_err(|e| FetchError::Transient(e.to_string()))?;
        Ok(bytes.into_iter().take(max as usize).collect())
    }
}

pub use imp::{fetch_bytes, fetch_html};

#[cfg(test)]
mod cooldown_tests {
    use std::time::Duration;

    use web_time::Instant;

    use super::{COOLDOWN_BASE, Cooldowns, FetchError, MAX_COOLDOWN, parse_retry_after};

    const HOST: &str = "github.com";

    #[test]
    fn unknown_host_is_not_cooling() {
        assert!(Cooldowns::new().check(HOST, Instant::now()).is_ok());
    }

    #[test]
    fn retry_after_sets_the_next_retry_timestamp() {
        let c = Cooldowns::new();
        let t0 = Instant::now();
        let until = c.record_rate_limited(HOST, Some(Duration::from_secs(60)), t0);
        assert_eq!(until, t0 + Duration::from_secs(60));

        // blocked strictly before `until`, with the timestamp surfaced
        match c.check(HOST, t0 + Duration::from_secs(59)) {
            Err(FetchError::RateLimited { until: u }) => assert_eq!(u, until),
            other => panic!("expected RateLimited, got {other:?}"),
        }
        // open again the moment it passes
        assert!(c.check(HOST, until).is_ok());
    }

    #[test]
    fn missing_retry_after_backs_off_exponentially_with_jitter() {
        let c = Cooldowns::new();
        let t0 = Instant::now();
        // strike n's delay is jittered within (base·2ⁿ/2, base·2ⁿ]
        for strikes in 0..3u32 {
            let until = c.record_rate_limited(HOST, None, t0);
            let d = until - t0;
            let full = COOLDOWN_BASE * 2u32.pow(strikes);
            assert!(
                d > full / 2 && d <= full,
                "strike {strikes}: {d:?} not in ({:?}, {full:?}]",
                full / 2
            );
        }
    }

    #[test]
    fn server_supplied_delay_is_clamped() {
        let c = Cooldowns::new();
        let t0 = Instant::now();
        let until = c.record_rate_limited(HOST, Some(Duration::from_secs(86_400)), t0);
        assert_eq!(until, t0 + MAX_COOLDOWN);
    }

    #[test]
    fn backoff_never_exceeds_the_ceiling() {
        let c = Cooldowns::new();
        let t0 = Instant::now();
        let mut until = t0;
        for _ in 0..20 {
            until = c.record_rate_limited(HOST, None, t0);
        }
        assert!(until - t0 <= MAX_COOLDOWN);
    }

    #[test]
    fn success_resets_strikes() {
        let c = Cooldowns::new();
        let t0 = Instant::now();
        for _ in 0..4 {
            c.record_rate_limited(HOST, None, t0);
        }
        c.record_success(HOST);
        assert!(c.check(HOST, t0).is_ok());
        let until = c.record_rate_limited(HOST, None, t0);
        assert!(until - t0 <= COOLDOWN_BASE, "strikes should restart at zero");
    }

    #[test]
    fn hosts_cool_down_independently() {
        let c = Cooldowns::new();
        let t0 = Instant::now();
        c.record_rate_limited(HOST, Some(Duration::from_secs(60)), t0);
        assert!(c.check("example.com", t0).is_ok());
    }

    #[test]
    fn parses_delta_seconds_only() {
        assert_eq!(parse_retry_after("60"), Some(Duration::from_secs(60)));
        assert_eq!(parse_retry_after(" 5 "), Some(Duration::from_secs(5)));
        // HTTP-date form → None → caller falls back to the backoff schedule
        assert_eq!(parse_retry_after("Wed, 21 Oct 2026 07:28:00 GMT"), None);
        assert_eq!(parse_retry_after(""), None);
    }
}
