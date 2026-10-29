use std::process::Command;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use cli_rs::cli_error::{CliError, CliResult};
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use reqwest::Method;
use reqwest::blocking::Client;
use reqwest::header::CONTENT_TYPE;
use serde::Serialize;
use serde_json::{Value, json};

use crate::releaser::secrets::AppStoreConnect;
use crate::releaser::utils::lb_version;
use crate::utils::CommandRunner;

const API: &str = "https://api.appstoreconnect.apple.com/v1";
const AUDIENCE: &str = "appstoreconnect-v1";
const APP_ID: &str = "1526775001";
const RELEASES: &str = "https://github.com/lockbook/lockbook/releases/tag";

const RELEASE_TYPE: &str = "AFTER_APPROVAL";
const EDITABLE_VERSION_STATES: &str = "PREPARE_FOR_SUBMISSION,READY_FOR_REVIEW,DEVELOPER_REJECTED,REJECTED,METADATA_REJECTED,INVALID_BINARY";
const OPEN_SUBMISSION_STATES: &str = "READY_FOR_REVIEW,UNRESOLVED_ISSUES";

const POLL_INTERVAL: Duration = Duration::from_secs(30);
const PROCESSING_TIMEOUT: Duration = Duration::from_secs(2 * 60 * 60);

#[derive(Serialize)]
struct Claims {
    iss: String,
    iat: u64,
    exp: u64,
    aud: String,
}

pub fn submit_for_review() -> CliResult<()> {
    let asc = AppStoreConnectClient::new();
    for (platform, archive) in [("IOS", "iOS"), ("MAC_OS", "macOS")] {
        let build = asc.processed_build(platform, &archived_build_number(archive)?)?;
        let version = asc.prepare_version(platform, &build)?;
        asc.set_whats_new(&version)?;
        asc.submit(platform, &version)?;
        println!("submitted {platform} {} for app store review", lb_version());
    }
    Ok(())
}

fn archived_build_number(archive: &str) -> CliResult<String> {
    let output = Command::new("/usr/libexec/PlistBuddy")
        .args(["-c", "Print :ApplicationProperties:CFBundleVersion"])
        .arg(format!("clients/apple/build/Lockbook-{archive}.xcarchive/Info.plist"))
        .success_output()?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

struct AppStoreConnectClient {
    http: Client,
    key: AppStoreConnect,
}

impl AppStoreConnectClient {
    fn new() -> Self {
        Self { http: Client::new(), key: AppStoreConnect::env() }
    }

    fn processed_build(&self, platform: &str, number: &str) -> CliResult<String> {
        let started = Instant::now();
        loop {
            let builds = self.get(&format!(
                "/builds?filter[app]={APP_ID}&filter[version]={number}&filter[preReleaseVersion.platform]={platform}"
            ))?;
            let state = builds["data"][0]["attributes"]["processingState"].as_str();
            match state {
                Some("VALID") => return Ok(id(&builds["data"][0])),
                Some(failed @ ("FAILED" | "INVALID")) => {
                    return Err(error(format!("{platform} build {number} is {failed}")));
                }
                _ if started.elapsed() > PROCESSING_TIMEOUT => {
                    return Err(error(format!(
                        "{platform} build {number} did not finish processing within {PROCESSING_TIMEOUT:?}"
                    )));
                }
                _ => {
                    println!("waiting for {platform} build {number} to process: {state:?}");
                    thread::sleep(POLL_INTERVAL);
                }
            }
        }
    }

    fn prepare_version(&self, platform: &str, build: &str) -> CliResult<String> {
        let editable = self.get(&format!(
            "/apps/{APP_ID}/appStoreVersions?filter[platform]={platform}&filter[appVersionState]={EDITABLE_VERSION_STATES}"
        ))?;
        let build = json!({ "data": { "type": "builds", "id": build } });

        let version = match editable["data"].get(0) {
            Some(existing) => {
                let existing = id(existing);
                self.patch(
                    &format!("/appStoreVersions/{existing}"),
                    json!({ "data": {
                        "type": "appStoreVersions",
                        "id": existing,
                        "attributes": { "versionString": lb_version(), "releaseType": RELEASE_TYPE },
                        "relationships": { "build": build },
                    }}),
                )?
            }
            None => self.post(
                "/appStoreVersions",
                json!({ "data": {
                    "type": "appStoreVersions",
                    "attributes": {
                        "platform": platform,
                        "versionString": lb_version(),
                        "releaseType": RELEASE_TYPE,
                    },
                    "relationships": {
                        "app": { "data": { "type": "apps", "id": APP_ID } },
                        "build": build,
                    },
                }}),
            )?,
        };

        Ok(id(&version["data"]))
    }

    fn set_whats_new(&self, version: &str) -> CliResult<()> {
        let whats_new = format!(
            "Several bugfixes and refinements. Visit {RELEASES}/{} for more details",
            lb_version()
        );
        let localizations =
            self.get(&format!("/appStoreVersions/{version}/appStoreVersionLocalizations"))?;

        for localization in localizations["data"].as_array().unwrap() {
            let localization = id(localization);
            self.patch(
                &format!("/appStoreVersionLocalizations/{localization}"),
                json!({ "data": {
                    "type": "appStoreVersionLocalizations",
                    "id": localization,
                    "attributes": { "whatsNew": whats_new },
                }}),
            )?;
        }
        Ok(())
    }

    fn submit(&self, platform: &str, version: &str) -> CliResult<()> {
        let open = self.get(&format!(
            "/reviewSubmissions?filter[app]={APP_ID}&filter[platform]={platform}&filter[state]={OPEN_SUBMISSION_STATES}"
        ))?;
        let submission = match open["data"].get(0) {
            Some(existing) => id(existing),
            None => id(&self.post(
                "/reviewSubmissions",
                json!({ "data": {
                    "type": "reviewSubmissions",
                    "attributes": { "platform": platform },
                    "relationships": { "app": { "data": { "type": "apps", "id": APP_ID } } },
                }}),
            )?["data"]),
        };

        let items =
            self.get(&format!("/reviewSubmissions/{submission}/items?include=appStoreVersion"))?;
        let already_added = items["data"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["relationships"]["appStoreVersion"]["data"]["id"] == version);

        if !already_added {
            self.post(
                "/reviewSubmissionItems",
                json!({ "data": {
                    "type": "reviewSubmissionItems",
                    "relationships": {
                        "reviewSubmission": { "data": { "type": "reviewSubmissions", "id": submission } },
                        "appStoreVersion": { "data": { "type": "appStoreVersions", "id": version } },
                    },
                }}),
            )?;
        }

        self.patch(
            &format!("/reviewSubmissions/{submission}"),
            json!({ "data": {
                "type": "reviewSubmissions",
                "id": submission,
                "attributes": { "submitted": true },
            }}),
        )?;
        Ok(())
    }

    fn get(&self, path: &str) -> CliResult<Value> {
        self.send(Method::GET, path, None)
    }

    fn post(&self, path: &str, body: Value) -> CliResult<Value> {
        self.send(Method::POST, path, Some(body))
    }

    fn patch(&self, path: &str, body: Value) -> CliResult<Value> {
        self.send(Method::PATCH, path, Some(body))
    }

    fn send(&self, method: Method, path: &str, body: Option<Value>) -> CliResult<Value> {
        let mut request = self
            .http
            .request(method.clone(), format!("{API}{path}"))
            .bearer_auth(self.token());
        if let Some(body) = body {
            request = request
                .header(CONTENT_TYPE, "application/json")
                .body(body.to_string());
        }

        let response = request.send().unwrap();
        let status = response.status();
        let text = response.text().unwrap();
        if !status.is_success() {
            return Err(error(format!("app store connect {method} {path}: {status}\n{text}")));
        }

        Ok(if text.is_empty() { Value::Null } else { serde_json::from_str(&text).unwrap() })
    }

    fn token(&self) -> String {
        let mut header = Header::new(Algorithm::ES256);
        header.kid = Some(self.key.key_id.clone());

        let iat = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let claims = Claims {
            iss: self.key.issuer_id.clone(),
            iat,
            exp: iat + 20 * 60,
            aud: AUDIENCE.to_string(),
        };

        encode(
            &header,
            &claims,
            &EncodingKey::from_ec_pem(self.key.private_key.as_bytes()).unwrap(),
        )
        .unwrap()
    }
}

fn id(resource: &Value) -> String {
    resource["id"].as_str().unwrap().to_string()
}

fn error(msg: String) -> CliError {
    CliError { msg, status: 1 }
}
