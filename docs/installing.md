# Install Lockbook
Lockbook is available on all major platforms — here are the supported ways to install.

Are we missing yours? Reach out on [Discord](https://discord.gg/lockbook), file a [GitHub](https://github.com/lockbook/lockbook/issues) issue, or [build](building.md) Lockbook yourself.

## Windows
*Coming [soon](https://github.com/lockbook/lockbook/issues/4535) to the Windows Store.*

**Desktop app**:
1. Navigate to our [Github Releases](https://github.com/lockbook/lockbook/releases).
1. Download and open `lockbook-windows-setup-x86_64.exe` or `lockbook-windows-setup-arm64.exe`
   * Choose the `x86_64` variant for x64-based devices or the `arm64` variant for ARM-based devices.
   * 99%+ of Windows devices use `x86_64`—if you don't know, your device probably uses this one.
   * You can check in Settings > System > About > Device info > System type.
1. When prompted with "Windows protected your PC," click 'More info' and 'Run Anyway.' These steps are temporary until Lockbook is distributed through the Windows Store.
1. Click the 'Install' button in the installer.
1. Launch Lockbook from the start menu like any other application.

**CLI**:
* Available through our [Github Releases](https://github.com/lockbook/lockbook/releases).

## macOS
**Desktop app**:
* Install through the [App Store](https://apps.apple.com/us/app/lockbook/id1526775001?platform=mac).
* Also available through our [Github Releases](https://github.com/lockbook/lockbook/releases).

**CLI**:
- `brew`: `brew tap lockbook/lockbook && brew install lockbook`
* Also available through our [Github Releases](https://github.com/lockbook/lockbook/releases).

## Linux
**Desktop app**:
* Install through a package manager:
  - [Nix](https://search.nixos.org/packages?channel=25.05&show=lockbook-desktop&from=0&size=50&sort=relevance&type=packages&query=lockbook-desktop): `nix-shell -p lockbook-desktop`
  - [AUR](https://aur.archlinux.org/packages/lockbook-desktop): `yay -S lockbook-desktop`
  - [Snap](https://snapcraft.io/lockbook-desktop): `snap install lockbook-desktop`
  - [Flatpak](https://flathub.org/en/apps/net.lockbook.Lockbook): `flatpak install flathub net.lockbook.Lockbook`
* Also available through our [Github Releases](https://github.com/lockbook/lockbook/releases).

**CLI**:
* Install through a package manager:
  - [Cargo](https://crates.io/crates/lockbook): `cargo install lockbook`
  - [Nix](https://search.nixos.org/packages?channel=25.05&show=lockbook&from=0&size=50&sort=relevance&type=packages&query=lockbook): `nix-shell -p lockbook`
  - [AUR](https://aur.archlinux.org/packages/lockbook): `yay -S lockbook`
  - [Snap](https://snapcraft.io/lockbook): `snap install lockbook`
* Also available through our [Github Releases](https://github.com/lockbook/lockbook/releases).

## iOS
* Install through the [App Store](https://apps.apple.com/us/app/lockbook/id1526775001).

## Android
* Install through the [Play Store](https://play.google.com/store/apps/details?id=app.lockbook).
* Also available through our [Github Releases](https://github.com/lockbook/lockbook/releases).
