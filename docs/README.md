# Lockbook

[<img height= "30" src="https://apple-resources.s3.amazonaws.com/media-badges/download-on-the-app-store/black/en-us.svg">](https://apps.apple.com/us/app/lockbook/id1526775001) [<img height= "30" src="https://upload.wikimedia.org/wikipedia/commons/thumb/7/78/Google_Play_Store_badge_EN.svg/2560px-Google_Play_Store_badge_EN.svg.png">](https://play.google.com/store/apps/details?id=app.lockbook)

[![Discord](https://img.shields.io/discord/1014184997751619664?label=Discord&style=plastic)](https://discord.gg/lockbook)

## About
_The private, polished note-taking platform._

[![What's Lockbook Video](https://img.youtube.com/vi/doPI9IajzKw/0.jpg)](https://www.youtube.com/watch?v=doPI9IajzKw)

Privacy shouldn't be a compromise. That's why we created Lockbook, a secure note-taking app that lets you record, sync, and share your thoughts. We collect no personal information and encrypt your notes so even we can't see them. Don't take our word for it: Lockbook is 100% open-source.

### Polished
We built Lockbook for everyday use because we use Lockbook every day. Our native apps feel at home on every platform, and we've gone the extra mile to ensure they're fast, stable, efficient, and delightful to use. We can't wait for you to try them.

### Secure
Keep your thoughts to yourself. Lockbook encrypts your notes with keys that are generated on your devices and stay on your devices. Only you and the users you share your notes with can see them; no one else, including infrastructure providers, state actors, or Lockbook employees, can access your data.

### Private
Know your customer? We sure don't. We don't collect your email, phone number, or name. We don't need a password. Lockbook is for people with better things to worry about than privacy.

### Honest
Be the customer, not the product. We sell a note-taking app, not your data.

| Payment Option | Monthly Fee    |
|----------------|----------------|
| Monthly        | $2.99 per 30GB |

### Developer Friendly
The Lockbook CLI will fit right into your favorite chain of piped-together Unix commands. Search your notes with `fzf`, edit them with `vim`, and schedule backups with `cron`. When scripting doesn't cut it, use our Rust library for a robust programmatic interface.

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

## Feature Matrix

<details> 
<summary>Legend</summary>

+ ✅ Done
+ 🏗 Planned
+ ⛔️ Not Supported

</details>

|                             | iOS/iPadOS | Android | macOS | Linux | Windows | CLI |
|-----------------------------|------------|---------|-------|-------|---------|-----|
| Register & Login            | ✅          | ✅      | ✅    | ✅    | ✅      | ✅   |
| Upgrade To Premium          | ✅          | ✅      | ✅    | ✅    | ✅      | ✅   |
| Edit & Sync Files           | ✅          | ✅      | ✅    | ✅    | ✅      | ✅   |
| Import & Export To Device   | ✅          | 🏗      | ✅    | 🏗    | 🏗      | ✅   |
| Search                      | ✅          | ✅      | ✅    | ✅    | ✅      | 🏗   |
| Share Files                 | ✅          | ✅      | ✅    | ✅    | ✅      | ✅   |
| Markdown                    | ✅          | ✅      | ✅    | ✅    | ✅      | ✅   |
| Drawings                    | ✅          | ✅      | 🏗    | ✅    | ✅      | ⛔️   |
| Images & PDFs               | ✅          | ✅      | ✅    | ✅    | ✅      | ⛔️   |
