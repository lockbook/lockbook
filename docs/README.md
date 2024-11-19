# Lockbook: Private note-taking & file storage
Write notes, sketch ideas, and upload documents. Backup and share across platforms. We encrypt your files so even we can’t see them. Don’t take our word for it: Lockbook is 100% open-source.

[<img height= "30" src="https://apple-resources.s3.amazonaws.com/media-badges/download-on-the-app-store/black/en-us.svg">](https://apps.apple.com/us/app/lockbook/id1526775001) [<img height= "30" src="https://upload.wikimedia.org/wikipedia/commons/thumb/7/78/Google_Play_Store_badge_EN.svg/2560px-Google_Play_Store_badge_EN.svg.png">](https://play.google.com/store/apps/details?id=app.lockbook)

[![Discord](https://img.shields.io/discord/1014184997751619664?label=Discord&style=plastic)](https://discord.gg/lockbook)

[![What's Lockbook Video](https://img.youtube.com/vi/doPI9IajzKw/0.jpg)](https://www.youtube.com/watch?v=doPI9IajzKw)

## Community-driven
Lockbook is in open beta. Join our community, share your feedback, and help achieve our vision of open-source privacy without compromises.

## Private & Secure
Keep your thoughts to yourself. Lockbook uses secp256k1 ECDSA keys — just like Bitcoin — to hide your files from prying eyes. Your files never leave your device without being encrypted with your private key. Your private key is generated on your device and transferred directly to your other devices by scanning a QR code or typing a 24-word phrase.

## Transparent
Be the customer, not the product. We sell a note-taking app, not your data.
| Storage    | Price         |
|------------|---------------|
| Up to 1MB  | Free          |
| Up to 30GB | $2.99 / month |
| Above 30GB | Coming soon   |

Lockbook compresses your files before measuring your usage. This makes text files up to 5x smaller: the free tier is about enough to store the entire Harry Potter book series. Larger and less compressible formats like PDFs and images will fill your storage at a closer-to-normal rate.

## Developer-friendly
The Lockbook CLI will fit right into your favorite chain of piped-together Unix commands. Search your notes with `fzf`, edit them with `vim`, and schedule backups with `cron`. Our Rust library `lb-rs` has bindings for C and Java.

## How To Install
### Mobile
iOS/iPadOS:
- [App Store](https://apps.apple.com/us/app/lockbook/id1526775001)
- [Build From Source](./guides/build/apple.md)

Android:
- [Play Store](https://play.google.com/store/apps/details?id=app.lockbook)
- [Github Releases](https://github.com/lockbook/lockbook/releases)
- [Build From Source](./guides/build/android.md)

### Desktop
macOS:
- [App Store](https://apps.apple.com/us/app/lockbook/id1526775001)
- [Github Releases](https://github.com/lockbook/lockbook/releases)
- [Build From Source](./guides/build/apple.md)

Linux:
- [AUR (Arch)](https://aur.archlinux.org/packages/lockbook-desktop): `yay -S lockbook-desktop`
- [Snap](https://snapcraft.io/lockbook-desktop): `snap install lockbook-desktop` (warning: Snap does not verify package integrity)
- [Github Releases](https://github.com/lockbook/lockbook/releases)
- [Build From Source](./guides/build/linux.md)

Windows:
- [Github Releases](https://github.com/lockbook/lockbook/releases)
- [Build From Source](./guides/build/windows.md)

### CLI
macOS:
- `brew`: `brew tap lockbook/lockbook && brew install lockbook`
- [Github Releases](https://github.com/lockbook/lockbook/releases)
- [Build From Source](./guides/build/cli.md)

Linux:
- [AUR (Arch)](https://aur.archlinux.org/packages/lockbook): `yay -S lockbook`
- [Snap](https://snapcraft.io/lockbook): `snap install lockbook` (warning: Snap does not verify package integrity)
- [Github Releases](https://github.com/lockbook/lockbook/releases)
- [Build From Source](./guides/build/cli.md)

Windows:
- [Github Releases](https://github.com/lockbook/lockbook/releases)
- [Build From Source](./guides/build/cli.md)

#### CLI Completions
- [CLI Completions Guide for macos && (bash || zsh)](./guides/cli-completions.md)
