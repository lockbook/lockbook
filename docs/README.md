# Lockbook

[<img height= "30" src="https://apple-resources.s3.amazonaws.com/media-badges/download-on-the-app-store/black/en-us.svg">](https://apps.apple.com/us/app/lockbook/id1526775001) [<img height= "30" src="https://upload.wikimedia.org/wikipedia/commons/thumb/7/78/Google_Play_Store_badge_EN.svg/2560px-Google_Play_Store_badge_EN.svg.png">](https://play.google.com/store/apps/details?id=app.lockbook)

![Discord](https://img.shields.io/discord/1014184997751619664?label=Discord&style=plastic)

## About
_The private, polished note-taking platform._

Privacy shouldn't be a compromise. That's why we made Lockbook, a companion for recording thoughts on all your devices. Record, sync, and share your notes with apps engineered to feel like home on every platform. We collect no personal information and encrypt your notes so even _we_ can’t see them. Don’t take our word for it: Lockbook is 100% open-source.

### Polished
We built Lockbook for everyday use because we use Lockbook every day. We need a note-taking app that doesn't make trade-offs concerning speed, stability, efficiency, device integration, or delightfulness. The only way to have that is to put in the effort, including writing native apps for every platform, and we can't wait for you to try them.

### Secure
Keep your thoughts to yourself. Your notes are encrypted with keys that are generated on your devices and stay on your devices. The only people that can see your notes are you and the users you share them with. No one else, including infrastructure providers, state actors, and Lockbook employees, can see your notes.

### Private
Know your customer? We sure don't. We don't collect your email, phone number, or name. We don't even need a password. Lockbook is for people with better things to worry about than privacy.

### Honest
Be the customer, not the product. We make money by selling a note-taking app, not your data.

| Payment Option | Monthly Fee    |
|----------------|----------------|
| Monthly        | $2.99 per 30GB |

### Developer Friendly
We also provide a CLI tool that will fit right into your favorite chain of piped-together Unix commands. Search your notes with `fzf`, edit them with `vim`, and schedule backups with `cron`. When scripting doesn't cut it, use our Rust library for a robust programmatic interface with Lockbook.

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
- `yay` (Arch): `yay -S lockbook-desktop`
- `snap`: `snap install lockbook-desktop` (warning: Snap does not verify package integrity)
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
- `yay` (Arch): `yay -S lockbook`
- `snap`: `snap install lockbook` (warning: Snap does not verify package integrity)
- [Github Releases](https://github.com/lockbook/lockbook/releases)
- [Build From Source](./guides/build/cli.md)

Windows:
- [Github Releases](https://github.com/lockbook/lockbook/releases)
- [Build From Source](./guides/build/cli.md)

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
| Import & Export To Device   | 🏗          | 🏗      | 🏗    | 🏗    | 🏗      | ✅   |
| Search                      | 🏗          | ✅      | 🏗    | 🏗    | 🏗      | 🏗   |
| Share Files                 | ✅          | ✅      | ✅    | 🏗    | 🏗      | ✅   |
| Markdown                    | ✅          | ✅      | ✅    | ✅    | ✅      | ✅   |
| Drawings                    | ✅          | ✅      | 🏗    | ✅    | ✅      | ⛔️   |
| Images & PDFs               | 🏗          | ✅      | 🏗    | 🏗    | 🏗      | ⛔️   |
