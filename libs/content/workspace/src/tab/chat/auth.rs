//! SuperGrok / X Premium device-code OAuth. Tokens live in workspace
//! settings (`ws_persistence.json`) — never in a `.chat` file.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::workspace::{GrokPrefs, WsPersistentStore};

const DISCOVERY: &str = "https://auth.x.ai/.well-known/openid-configuration";
const DEVICE_CODE_URL: &str = "https://auth.x.ai/oauth2/device/code";
/// Public client id used by Grok CLI / partner harnesses for subscription OAuth.
const CLIENT_ID: &str = "b1a00492-073a-47ea-816f-4c329264a828";
const SCOPE: &str = "openid profile email offline_access grok-cli:access api:access";
const REFRESH_SKEW_SECS: u64 = 3600;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenSet {
    pub access_token: String,
    pub refresh_token: String,
    #[serde(default)]
    pub id_token: String,
    pub expires_at: u64,
    #[serde(default)]
    pub token_endpoint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct AuthFile {
    #[serde(default)]
    tokens: Option<TokenSet>,
}

pub fn auth_path(writeable_path: &str) -> PathBuf {
    Path::new(writeable_path).join("grok-auth.json")
}

impl TokenSet {
    pub fn from_prefs(p: &GrokPrefs) -> Option<Self> {
        if p.access_token.is_empty() || p.refresh_token.is_empty() {
            return None;
        }
        Some(Self {
            access_token: p.access_token.clone(),
            refresh_token: p.refresh_token.clone(),
            id_token: p.id_token.clone(),
            expires_at: p.expires_at,
            token_endpoint: p.token_endpoint.clone(),
        })
    }

    pub fn write_prefs(&self, p: &mut GrokPrefs) {
        p.access_token = self.access_token.clone();
        p.refresh_token = self.refresh_token.clone();
        p.id_token = self.id_token.clone();
        p.expires_at = self.expires_at;
        p.token_endpoint = self.token_endpoint.clone();
    }
}

/// Prefer workspace settings; copy a leftover `grok-auth.json` in once.
pub fn load_or_migrate(cfg: &WsPersistentStore, writeable_path: &str) -> Option<TokenSet> {
    let mut prefs = cfg.grok();
    if let Some(t) = TokenSet::from_prefs(&prefs) {
        return Some(t);
    }
    let legacy = load_tokens(&auth_path(writeable_path));
    if let Some(t) = &legacy {
        t.write_prefs(&mut prefs);
        cfg.set_grok(prefs);
        info!("chat oauth migrated grok-auth.json into workspace settings");
    }
    legacy
}

pub fn save_prefs(cfg: &WsPersistentStore, tokens: &TokenSet) {
    let mut prefs = cfg.grok();
    tokens.write_prefs(&mut prefs);
    cfg.set_grok(prefs);
}

pub fn is_auth_failure(e: &str) -> bool {
    let e = e.to_ascii_lowercase();
    e.contains("401")
        || e.contains("403")
        || e.contains("entitled")
        || e.contains("invalid_grant")
        || e.contains("sign in again")
}

pub fn sign_in_again(e: &str) -> String {
    if e.to_ascii_lowercase().contains("entitled") {
        e.to_string()
    } else {
        "Sign in again — the saved Grok login expired.".into()
    }
}

pub fn clear_prefs(cfg: &WsPersistentStore) {
    let mut prefs = cfg.grok();
    prefs.access_token.clear();
    prefs.refresh_token.clear();
    prefs.id_token.clear();
    prefs.expires_at = 0;
    prefs.token_endpoint.clear();
    cfg.set_grok(prefs);
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn load_file(path: &Path) -> AuthFile {
    let Ok(bytes) = fs::read(path) else {
        return AuthFile::default();
    };
    serde_json::from_slice(&bytes).unwrap_or_default()
}

fn load_tokens(path: &Path) -> Option<TokenSet> {
    load_file(path).tokens
}

/// Bearer token, refreshing when fewer than an hour remains.
/// Caller persists `tokens` when `expires_at` changes.
pub fn resolve_bearer(tokens: &mut TokenSet) -> Result<String, String> {
    let remaining = tokens.expires_at.saturating_sub(now_secs());
    if remaining < REFRESH_SKEW_SECS {
        info!(remaining_secs = remaining, "chat oauth refresh");
        *tokens = refresh_tokens(tokens)?;
    }
    Ok(tokens.access_token.clone())
}

fn discovery_token_endpoint() -> Result<String, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client
        .get(DISCOVERY)
        .header("Accept", "application/json")
        .send()
        .map_err(|e| format!("OIDC discovery failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("OIDC discovery HTTP {}", resp.status()));
    }
    let v: serde_json::Value = resp
        .json()
        .map_err(|e| format!("OIDC discovery JSON: {e}"))?;
    let ep = v
        .get("token_endpoint")
        .and_then(|x| x.as_str())
        .ok_or_else(|| "OIDC discovery missing token_endpoint".to_string())?;
    if !ep.contains("auth.x.ai") {
        return Err(format!("refusing non-xAI token_endpoint: {ep}"));
    }
    Ok(ep.to_string())
}

fn refresh_tokens(tokens: &TokenSet) -> Result<TokenSet, String> {
    let endpoint = if tokens.token_endpoint.is_empty() {
        discovery_token_endpoint()?
    } else {
        tokens.token_endpoint.clone()
    };
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client
        .post(&endpoint)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .header("Accept", "application/json")
        .form(&[
            ("grant_type", "refresh_token"),
            ("client_id", CLIENT_ID),
            ("refresh_token", tokens.refresh_token.as_str()),
        ])
        .send()
        .map_err(|e| format!("token refresh request failed: {e}"))?;
    let status = resp.status();
    let body = resp.text().unwrap_or_default();
    if !status.is_success() {
        warn!(%status, body = %body.chars().take(200).collect::<String>(), "chat oauth refresh http");
        if status.as_u16() == 403 {
            return Err("this SuperGrok account is not entitled for API access. \
                 Check your plan at https://x.ai/grok."
                .into());
        }
        if body.contains("invalid_grant") || body.contains("refresh token") {
            return Err("Sign in again — the saved Grok login expired.".into());
        }
        return Err(format!(
            "token refresh HTTP {status}: {}",
            body.chars().take(300).collect::<String>()
        ));
    }
    token_set_from_json(&body, endpoint)
}

fn token_set_from_json(body: &str, endpoint: String) -> Result<TokenSet, String> {
    let v: serde_json::Value =
        serde_json::from_str(body).map_err(|e| format!("token JSON: {e}"))?;
    let access = v
        .get("access_token")
        .and_then(|x| x.as_str())
        .ok_or_else(|| "missing access_token".to_string())?
        .to_string();
    let refresh = v
        .get("refresh_token")
        .and_then(|x| x.as_str())
        .ok_or_else(|| "missing refresh_token".to_string())?
        .to_string();
    let expires_in = v
        .get("expires_in")
        .and_then(|x| x.as_u64())
        .unwrap_or(6 * 3600);
    Ok(TokenSet {
        access_token: access,
        refresh_token: refresh,
        id_token: v
            .get("id_token")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string(),
        expires_at: now_secs() + expires_in,
        token_endpoint: endpoint,
    })
}

pub struct DeviceStart {
    pub user_code: String,
    pub verification_url: String,
    device_code: String,
    token_endpoint: String,
    pub interval: u64,
    pub expires_at: u64,
}

pub fn start_device() -> Result<DeviceStart, String> {
    let token_endpoint = discovery_token_endpoint()?;
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())?;
    let device = request_device_code(&client)?;
    let device_code = device
        .get("device_code")
        .and_then(|x| x.as_str())
        .ok_or("missing device_code")?
        .to_string();
    let user_code = device
        .get("user_code")
        .and_then(|x| x.as_str())
        .unwrap_or("?")
        .to_string();
    let verification_url = device
        .get("verification_uri_complete")
        .and_then(|x| x.as_str())
        .or_else(|| device.get("verification_uri").and_then(|x| x.as_str()))
        .ok_or("missing verification_uri")?
        .to_string();
    let expires_in = device
        .get("expires_in")
        .and_then(|x| x.as_u64())
        .unwrap_or(900);
    let interval = device
        .get("interval")
        .and_then(|x| x.as_u64())
        .unwrap_or(5)
        .max(1);
    info!(interval, expires_in, "chat oauth device code");
    Ok(DeviceStart {
        user_code,
        verification_url,
        device_code,
        token_endpoint,
        interval,
        expires_at: now_secs() + expires_in,
    })
}

fn request_device_code(client: &reqwest::blocking::Client) -> Result<serde_json::Value, String> {
    const MAX_ATTEMPTS: u32 = 6;
    for attempt in 1..=MAX_ATTEMPTS {
        let resp = client
            .post(DEVICE_CODE_URL)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .header("Accept", "application/json")
            .form(&[("client_id", CLIENT_ID), ("scope", SCOPE)])
            .send()
            .map_err(|e| format!("device code request failed: {e}"))?;
        if resp.status().is_success() {
            return resp.json().map_err(|e| format!("device code JSON: {e}"));
        }
        let status = resp.status();
        let retry_after = resp
            .headers()
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.trim().parse::<u64>().ok());
        let body = resp.text().unwrap_or_default();
        let slow = status.as_u16() == 429
            || body.contains("slow_down")
            || body.contains("Too many device code");
        if slow && attempt < MAX_ATTEMPTS {
            let delay = retry_after
                .unwrap_or_else(|| 15u64.saturating_mul(1u64 << (attempt - 1).min(3)))
                .clamp(5, 120);
            std::thread::sleep(Duration::from_secs(delay));
            continue;
        }
        return Err(format!(
            "device code HTTP {status}: {}",
            body.chars().take(300).collect::<String>()
        ));
    }
    Err("device code request exhausted retries".into())
}

pub enum Poll {
    Pending,
    SlowDown,
    Tokens(TokenSet),
    Failed(String),
}

pub fn poll_device(start: &DeviceStart) -> Poll {
    if now_secs() >= start.expires_at {
        return Poll::Failed("timed out waiting for device authorization".into());
    }
    let client = match reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
    {
        Ok(c) => c,
        Err(e) => return Poll::Failed(e.to_string()),
    };
    let poll = match client
        .post(&start.token_endpoint)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .header("Accept", "application/json")
        .form(&[
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ("client_id", CLIENT_ID),
            ("device_code", start.device_code.as_str()),
        ])
        .send()
    {
        Ok(r) => r,
        Err(e) => return Poll::Failed(format!("token poll failed: {e}")),
    };
    if poll.status().is_success() {
        let body = poll.text().unwrap_or_default();
        return match token_set_from_json(&body, start.token_endpoint.clone()) {
            Ok(t) => Poll::Tokens(t),
            Err(e) => Poll::Failed(e),
        };
    }
    let body = poll.text().unwrap_or_default();
    let err: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
    match err.get("error").and_then(|x| x.as_str()) {
        Some("authorization_pending") => Poll::Pending,
        Some("slow_down") => Poll::SlowDown,
        other => {
            let desc = err
                .get("error_description")
                .and_then(|x| x.as_str())
                .or(other)
                .unwrap_or(body.as_str());
            Poll::Failed(format!("device authorization failed: {desc}"))
        }
    }
}
