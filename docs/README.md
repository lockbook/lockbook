# Lockbook

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
| Space Utilized     |   ✅     |    ✅     |     ✅      |     ✅       |      ✅        |    ✅     |
| Billing            |   ✅     |    ✅     |     ✅      |     📆       |      📆        |    📆     |

### File Operations

|                       |  [CLI]  |  [Linux]  |  [Android]  |  [Windows]  |  [iOS/iPadOS]  |  [macOS]  |
|-----------------------|:-------:|:---------:|:-----------:|:-----------:|:--------------:|:---------:|
| Rename                |   ✅     |    ✅     |     ✅      |     ✅       |      ✅        |    ✅     |
| Move                  |   ✅     |    ✅     |     ✅      |     ✅       |      ✅        |    ✅     |
| Delete                |   ✅     |    ✅     |     ✅      |     ✅       |      ✅        |    ✅     |
| Sync                  |   ✅     |    ✅     |     ✅      |     ✅       |      ✅        |    ✅     |
| Export file to host   |   ✅     |    ✅     |     ✅      |     📆       |      📆        |    📆     |
| Import file from host |   ✅     |    ✅     |     📆      |     📆       |      📆        |    📆     |
| Sharing               |   ✅     |    📆     |     📆      |     📆       |      📆        |    📆     |

### Document Types

|                       |  [CLI]  |  [Linux]  |  [Android]  |  [Windows]  |  [iOS/iPadOS]  |  [macOS]  |
|-----------------------|:-------:|:---------:|:-----------:|:-----------:|:--------------:|:---------:|
| Text                  |   ✅     |    ✅     |     ✅      |     ✅       |      ✅        |    ✅     |
| Markdown              |   ✅     |    ✅     |     ✅      |     📆       |      ✅        |    ✅     |
| Drawings              |   ✅     |    🏗     |     ✅      |     🏗       |      ✅        |    ✅     |
| Images                |   ✅     |    ✅     |     ✅      |     📆       |      📆        |    📆     |
| PDFs                  |   📆     |    📆     |     ✅      |     📆       |      📆        |    📆     |
| Todo lists            |   📆     |    📆     |     📆      |     📆       |      📆        |    📆     |
| Document Linking      |   📆     |    ✅     |     📆      |     📆       |      📆        |    📆     |

# Further Reading

+ [System Architecture](design-tech/system-architecture.md)
+ [Data Model and Procedures](design-tech/data_model.md)

[Cli]: guides/install/cli.md
[Linux]: guides/install/linux.md
[Android]: guides/install/android.md
[Windows]: guides/install/windows.md
[macOS]: guides/install/macos.md
[iOS/iPadOS]: guides/install/iOS-iPadOS.md
