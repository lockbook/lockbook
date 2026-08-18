use std::fs;
use std::path::PathBuf;
use std::process::Command;

use cli_rs::cli_error::{CliError, CliResult};

use crate::releaser::utils::lb_version;
use crate::utils::CommandRunner;

const ASSETS: &str = "utils/lbdev/src/releaser/windows/msix-assets";
const BUNDLE: &str = "windows-build/Lockbook.msixbundle";
const PACKAGES: &str = "windows-build/msix";

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
        BackgroundColor="transparent"
        Square150x150Logo="Assets\Square150x150Logo.png"
        Square44x44Logo="Assets\Square44x44Logo.png">
        <uap:DefaultTile Square71x71Logo="Assets\Square71x71Logo.png" />
      </uap:VisualElements>
    </Application>
  </Applications>
</Package>
"##;
