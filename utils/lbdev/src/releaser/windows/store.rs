use std::fs;
use std::io::{Cursor, Write};
use std::path::PathBuf;
use std::process::Command;
use std::thread;
use std::time::Duration;

use cli_rs::cli_error::{CliError, CliResult};
use reqwest::Method;
use reqwest::blocking::Client;
use reqwest::header::CONTENT_TYPE;
use serde_json::{Value, json};
use zip::write::FileOptions;
use zip::{CompressionMethod, ZipWriter};

use crate::releaser::secrets::PartnerCenter;
use crate::releaser::utils::lb_version;
use crate::utils::CommandRunner;

const ASSETS: &str = "utils/lbdev/src/releaser/windows/msix-assets";
const BUNDLE: &str = "windows-build/Lockbook.msixbundle";
const PACKAGES: &str = "windows-build/msix";
const SCREENSHOTS: &str = "fastlane/metadata/windows/en-US/images/desktopScreenshots";

const SUBMISSION_API: &str = "https://manage.devcenter.microsoft.com/v1.0/my/applications";
const TOKEN_RESOURCE: &str = "https://manage.devcenter.microsoft.com";
const LISTING_LANGUAGE: &str = "en-us";
const DESKTOP_SCREENSHOT: &str = "Screenshot";
const COMMIT_POLL_INTERVAL: Duration = Duration::from_secs(15);

/// Fills a Partner Center submission's listing with the screenshots in `SCREENSHOTS`, each
/// `.png` captioned by the `.txt` beside it, with packages and copy as last published. Images
/// are ingested on commit, so `submit` is the only way to see them; without it the draft holds
/// the upload for a later run.
pub fn screenshots(submit: bool) -> CliResult<()> {
    let store = PartnerCenterClient::new()?;

    let app = store.get("")?;
    let mut submission = match app["pendingApplicationSubmission"]["id"].as_str() {
        Some(pending) => {
            let status = store.get(&format!("/submissions/{pending}/status"))?;
            if status["status"] != "PendingCommit" {
                return Err(error(format!(
                    "submission {pending} is {}; wait for it or delete it in Partner Center",
                    status["status"]
                )));
            }
            store.get(&format!("/submissions/{pending}"))?
        }
        None => store.post("/submissions")?,
    };
    let id = submission["id"].as_str().unwrap().to_string();
    let upload_url = submission["fileUploadUrl"].as_str().unwrap().to_string();

    let mut paths: Vec<PathBuf> = fs::read_dir(SCREENSHOTS)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "png"))
        .collect();
    paths.sort();

    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    let mut images = Vec::new();
    for path in &paths {
        let name = path.file_name().unwrap().to_str().unwrap();
        let caption = fs::read_to_string(path.with_extension("txt")).unwrap();
        zip.start_file(name, FileOptions::default().compression_method(CompressionMethod::Stored))
            .unwrap();
        zip.write_all(&fs::read(path).unwrap()).unwrap();
        images.push(json!({
            "fileName": name,
            "fileStatus": "PendingUpload",
            "imageType": DESKTOP_SCREENSHOT,
            "description": caption.trim(),
        }));
    }
    let zip = zip.finish().unwrap().into_inner();

    let listing = &mut submission["listings"][LISTING_LANGUAGE]["baseListing"]["images"];
    let Some(existing) = listing.as_array().cloned() else {
        return Err(error(format!("submission {id} has no {LISTING_LANGUAGE} listing")));
    };
    // published screenshots go, an earlier run's unprocessed uploads are superseded outright
    let mut all: Vec<Value> = existing
        .into_iter()
        .filter(|image| {
            image["imageType"] != DESKTOP_SCREENSHOT || image["fileStatus"] != "PendingUpload"
        })
        .map(|mut image| {
            if image["imageType"] == DESKTOP_SCREENSHOT {
                image["fileStatus"] = json!("PendingDelete");
            }
            image
        })
        .collect();
    all.extend(images);
    *listing = Value::Array(all);

    store.upload(&upload_url, zip)?;
    let updated = store.put(&format!("/submissions/{id}"), submission)?;
    for image in updated["listings"][LISTING_LANGUAGE]["baseListing"]["images"]
        .as_array()
        .unwrap()
    {
        println!("{}: {} {}", image["fileName"], image["imageType"], image["fileStatus"]);
    }
    println!(
        "submission {id}: https://partner.microsoft.com/dashboard/products/{}/submissions/{id}",
        store.key.product_id
    );

    if !submit {
        return Ok(());
    }

    store.post(&format!("/submissions/{id}/commit"))?;
    loop {
        let status = store.get(&format!("/submissions/{id}/status"))?;
        println!("submission {id} is {}", status["status"]);
        match status["status"].as_str() {
            Some("CommitStarted") => thread::sleep(COMMIT_POLL_INTERVAL),
            Some(failed) if failed.ends_with("Failed") => {
                return Err(error(format!("submission {id}: {}", status["statusDetails"])));
            }
            _ => return Ok(()),
        }
    }
}

struct PartnerCenterClient {
    http: Client,
    key: PartnerCenter,
    token: String,
}

impl PartnerCenterClient {
    fn new() -> CliResult<Self> {
        let http = Client::new();
        let key = PartnerCenter::env();
        let response = http
            .post(format!("https://login.microsoftonline.com/{}/oauth2/token", key.tenant_id))
            .form(&[
                ("grant_type", "client_credentials"),
                ("client_id", &key.client_id),
                ("client_secret", &key.client_secret),
                ("resource", TOKEN_RESOURCE),
            ])
            .send()
            .unwrap();
        let status = response.status();
        let body: Value = response.json().unwrap();
        let Some(token) = body["access_token"].as_str() else {
            return Err(error(format!("partner center sign-in: {status}\n{body}")));
        };
        Ok(Self { http, key, token: token.to_string() })
    }

    fn get(&self, path: &str) -> CliResult<Value> {
        self.send(Method::GET, path, None)
    }

    fn post(&self, path: &str) -> CliResult<Value> {
        self.send(Method::POST, path, None)
    }

    fn put(&self, path: &str, body: Value) -> CliResult<Value> {
        self.send(Method::PUT, path, Some(body))
    }

    fn send(&self, method: Method, path: &str, body: Option<Value>) -> CliResult<Value> {
        let mut request = self
            .http
            .request(method.clone(), format!("{SUBMISSION_API}/{}{path}", self.key.product_id))
            .bearer_auth(&self.token)
            .body(String::new()); // a bodiless POST still needs Content-Length: 0
        if let Some(body) = body {
            request = request
                .header(CONTENT_TYPE, "application/json")
                .body(body.to_string());
        }

        let response = request.send().unwrap();
        let status = response.status();
        let text = response.text().unwrap();
        if !status.is_success() {
            return Err(error(format!("partner center {method} {path}: {status}\n{text}")));
        }

        Ok(if text.is_empty() { Value::Null } else { serde_json::from_str(&text).unwrap() })
    }

    fn upload(&self, sas_url: &str, zip: Vec<u8>) -> CliResult<()> {
        let response = self
            .http
            .put(sas_url)
            .header("x-ms-blob-type", "BlockBlob")
            .body(zip)
            .send()
            .unwrap();
        let status = response.status();
        if !status.is_success() {
            return Err(error(format!(
                "partner center upload: {status}\n{}",
                response.text().unwrap()
            )));
        }
        Ok(())
    }
}

fn error(msg: String) -> CliError {
    CliError { msg, status: 1 }
}

pub fn pack() -> CliResult<()> {
    pack_for_target("x86_64-pc-windows-msvc", "x64")
}

pub fn pack_arm() -> CliResult<()> {
    pack_for_target("aarch64-pc-windows-msvc", "arm64")
}

fn pack_for_target(target: &str, arch: &str) -> CliResult<()> {
    Command::new("cargo")
        .args(["build", "-p", "lockbook-windows", "--release", &format!("--target={target}")])
        .assert_success()?;

    let layout = PathBuf::from(format!("windows-build/msix-{arch}"));
    if layout.exists() {
        fs::remove_dir_all(&layout).unwrap();
    }
    fs::create_dir_all(layout.join("Assets")).unwrap();

    fs::copy(format!("target/{target}/release/lockbook-windows.exe"), layout.join("lockbook.exe"))
        .unwrap();

    for asset in fs::read_dir(ASSETS).unwrap() {
        let asset = asset.unwrap().path();
        let name = asset.file_name().unwrap();
        fs::copy(&asset, layout.join("Assets").join(name)).unwrap();
    }

    fs::write(
        layout.join("AppxManifest.xml"),
        MANIFEST_TEMPLATE
            .replace("{version}", &package_version())
            .replace("{arch}", arch),
    )
    .unwrap();

    let priconfig = PathBuf::from(format!("windows-build/priconfig-{arch}.xml"));

    Command::new(sdk_tool("makepri.exe")?)
        .args(["createconfig", "/o", "/dq", "en-US", "/cf"])
        .arg(&priconfig)
        .assert_success()?;

    Command::new(sdk_tool("makepri.exe")?)
        .args(["new", "/o", "/pr"])
        .arg(&layout)
        .arg("/cf")
        .arg(&priconfig)
        .arg("/of")
        .arg(layout.join("resources.pri"))
        .assert_success()?;

    fs::create_dir_all(PACKAGES).unwrap();

    Command::new(sdk_tool("makeappx.exe")?)
        .args(["pack", "/o", "/d"])
        .arg(&layout)
        .arg("/p")
        .arg(format!("{PACKAGES}/lockbook-{arch}.msix"))
        .assert_success()
}

pub fn bundle() -> CliResult<()> {
    Command::new(sdk_tool("makeappx.exe")?)
        .args(["bundle", "/o", "/bv", &package_version(), "/d", PACKAGES, "/p", BUNDLE])
        .assert_success()
}

fn package_version() -> String {
    format!("{}.0", lb_version())
}

fn sdk_tool(name: &str) -> CliResult<PathBuf> {
    let bin = PathBuf::from(r"C:\Program Files (x86)\Windows Kits\10\bin");

    let mut kits: Vec<PathBuf> = fs::read_dir(&bin)
        .unwrap()
        .filter_map(|kit| kit.ok())
        .map(|kit| kit.path())
        .collect();
    kits.sort();

    let hosts = if std::env::consts::ARCH == "aarch64" { ["arm64", "x64"] } else { ["x64", "x86"] };

    for kit in kits.iter().rev() {
        for host in hosts {
            let tool = kit.join(host).join(name);
            if tool.exists() {
                return Ok(tool);
            }
        }
    }

    Err(CliError { msg: format!("could not find {name} under {}", bin.display()), status: 1 })
}

const MANIFEST_TEMPLATE: &str = r##"<?xml version="1.0" encoding="utf-8"?>
<Package
  xmlns="http://schemas.microsoft.com/appx/manifest/foundation/windows10"
  xmlns:uap="http://schemas.microsoft.com/appx/manifest/uap/windows10"
  xmlns:uap10="http://schemas.microsoft.com/appx/manifest/uap/windows10/10"
  xmlns:rescap="http://schemas.microsoft.com/appx/manifest/foundation/windows10/restrictedcapabilities"
  IgnorableNamespaces="uap uap10 rescap">

  <Identity
    Name="LockbookLLC.Lockbook"
    Version="{version}"
    Publisher="CN=D9AE12F1-1EE4-44A0-9763-F57F719BB9E1"
    ProcessorArchitecture="{arch}" />

  <Properties>
    <DisplayName>Lockbook</DisplayName>
    <PublisherDisplayName>Lockbook LLC</PublisherDisplayName>
    <Description>The private, polished note-taking platform.</Description>
    <Logo>Assets\StoreLogo.png</Logo>
  </Properties>

  <Resources>
    <Resource Language="en-us" />
  </Resources>

  <Dependencies>
    <TargetDeviceFamily Name="Windows.Desktop" MinVersion="10.0.19041.0" MaxVersionTested="10.0.26100.0" />
  </Dependencies>

  <Capabilities>
    <rescap:Capability Name="runFullTrust" />
  </Capabilities>

  <Applications>
    <Application
      Id="Lockbook"
      Executable="lockbook.exe"
      uap10:RuntimeBehavior="packagedClassicApp"
      uap10:TrustLevel="mediumIL">
      <uap:VisualElements
        DisplayName="Lockbook"
        Description="Write notes, sketch ideas, and store files in one secure place. Share seamlessly, keep data synced, and access it on any platform-even offline."
        BackgroundColor="#207FDF"
        Square150x150Logo="Assets\Square150x150Logo.png"
        Square44x44Logo="Assets\Square44x44Logo.png">
        <uap:DefaultTile Square71x71Logo="Assets\Square71x71Logo.png" />
      </uap:VisualElements>
    </Application>
  </Applications>
</Package>
"##;
