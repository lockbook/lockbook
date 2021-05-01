# Lockbook
![Build](https://github.com/lockbook/monorepo/workflows/Build/badge.svg)

## About
_The best place to store and share thoughts_

Privacy shouldn’t be a compromise. That’s why we made Lockbook, a companion for recording thoughts on all your devices. Record, sync, and share your notes with apps engineered to feel like home on every platform. We collect no personal information and encrypt your notes so even _we_ can’t see them. Don’t take our word for it: Lockbook is 100% open-source.

### Polished
We built Lockbook for everyday use because we use Lockbook everyday. We need a note-taking app that doesn't make trade-offs with respect to speed, stability, efficiency, device integration, or delightfulness. The only way to have that is put in the effort, including writing native apps for every platform, and we can't wait for you to try it.

### Secure
Keep your thoughts to yourself. Your notes are encrypted with keys that are generated on your devices and stay on your devices. The people that can see your notes are you and the users you share them with. The people that can't are everyone else, including infrastructure providers, state actors, and Lockbook employees.

### Private
Know your customer? We sure don't. We don't collect your email, phone number, or name. We don't need a password. We don't even want your credit card number: pay anonymously with bitcoin and get a discount. Lockbook is for people with better things to worry about than privacy.

### Honest
Be the customer, not the product. We make money by selling a note-taking app, not your data.

| Payment Option | Monthly Fee   |
|----------------|---------------|
| USD            | $5 + $0.03/GB |
| BTC            | $2 + $0.03/GB |

### Developer Friendly
The Lockbook CLI will right into your favorite chain of piped-together unix commands. Search your notes with `fzf`, edit them with `vim`, and schedule backups with `cron`. When scripting doesn't cut it, use our Rust library for a robust programmatic interface with Lockbook.

## Feature Matrix

<details> 
<summary>Legend</summary>

+ ✅ Done
+ 🏗 In Progress
+ 📆 Planned
+ ⛔️ Not Planned

</details>

### Account Management

|                    |  [CLI]  |  [Linux]  |  [Android]  |  [Windows]  |  [iOS/iPadOS]  |  [macOS]  |
|--------------------|:-------:|:---------:|:-----------:|:-----------:|:--------------:|:---------:|
| New Account        |   ✅     |    ✅     |     ✅      |     ✅       |      ✅        |    ✅     |
| QR Import          |   ⛔️     |    📆     |     ✅      |     📆       |      ✅        |    📆     |
| Import Account     |   ✅     |    ✅     |     ✅      |     ✅       |      ✅        |    ✅     |
| Space Utilized     |   ✅     |    🏗     |     ✅      |     ✅       |      ✅        |    ✅     |
| Billing            |   📆     |    📆     |     📆      |     📆       |      📆        |    📆     |

### File Operations

|                       |  [CLI]  |  [Linux]  |  [Android]  |  [Windows]  |  [iOS/iPadOS]  |  [macOS]  |
|-----------------------|:-------:|:---------:|:-----------:|:-----------:|:--------------:|:---------:|
| Rename                |   ✅     |    ✅     |     ✅      |     ✅       |      ✅        |    ✅     |
| Move                  |   ✅     |    📆     |     ✅      |     ✅       |      ✅        |    ✅     |
| Delete                |   ✅     |    ✅     |     ✅      |     ✅       |      ✅        |    ✅     |
| Sync                  |   ✅     |    ✅     |     ✅      |     ✅       |      ✅        |    ✅     |
| Export file to host   |   ✅     |    📆     |     📆      |     📆       |      📆        |    📆     |
| Import file from host |   ✅     |    📆     |     📆      |     📆       |      📆        |    📆     |
| Sharing               |   📆     |    📆     |     📆      |     📆       |      📆        |    📆     |

### Document Types

|                       |  [CLI]  |  [Linux]  |  [Android]  |  [Windows]  |  [iOS/iPadOS]  |  [macOS]  |
|-----------------------|:-------:|:---------:|:-----------:|:-----------:|:--------------:|:---------:|
| Text                  |   ✅     |    ✅     |     ✅      |     ✅       |      ✅        |    ✅     |
| Markdown              |   ✅     |    📆     |     ✅      |     📆       |      🏗        |    🏗     |
| Drawings              |   ✅     |    🏗     |     ✅      |     🏗       |      ✅        |    🏗     |
| Images                |   ✅     |    🏗     |     ✅      |     📆       |      📆        |    📆     |
| PDFs                  |   📆     |    📆     |     📆      |     📆       |      📆        |    📆     |
| Todo lists            |   📆     |    📆     |     📆      |     📆       |      📆        |    📆     |
| Document Linking      |   📆     |    📆     |     📆      |     📆       |      📆        |    📆     |

# Further Reading

+ [System Architecture](system-architecture.md)
+ [Data Model and Procedures](data_model.md)
+ [Building](building.md)

[Cli]: installing-cli.md
[Linux]: installing-linux.md
[Android]: installing-android.md
[Windows]: installing-windows.md
[MacOS]: installing-macos.md
[iOS/iPadOS]: installing-iOS-iPadOS.md
