# Project Overview
![Build](https://github.com/lockbook/monorepo/workflows/Build/badge.svg)

Lockbook is a document editor that is:
+ Secure
+ Minimal
+ Private
+ Open Source
+ Cross Platform

## Secure

All user generated content is encrypted on clients with keys that never leave your hands. No Lockbook employee, cloud provider, or state actor can view your content.

## Minimal

Clear, snappy, native user interfaces. Deep support for offline use and background sync. Our clients include a CLI which requires no dependencies, and invokes your favorite text editor. Minimal software is secure software.

## Private

No verification, no emails, no passwords. Our business model is straightforward ($/gb) and doesn't include selling your data.

## Open Source

Secure software cannot be closed source. This is free and unencumbered software released into the public domain. Open source makes it easy for security researchers to inspect our code and provide feedback. Problems are discussed openly and anyone can improve our software.

## Cross Platform

Native support for: Linux, macOS, iOS, iPadOS, Android, and Windows. We capture the essence of each device / platform. This means a scriptable CLI on Linux and Apple Pencil support for our iPad app.


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
| Space Utilized     |   ✅     |    🏗     |     ✅      |     ✅       |      🏗        |    🏗     |
| Billing            |   📆     |    📆     |     📆      |     📆       |      📆        |    📆     |

### File Operations

|                       |  [CLI]  |  [Linux]  |  [Android]  |  [Windows]  |  [iOS/iPadOS]  |  [macOS]  |
|-----------------------|:-------:|:---------:|:-----------:|:-----------:|:--------------:|:---------:|
| Rename                |   ✅     |    ✅     |     ✅      |     ✅       |      🏗        |    🏗     |
| Move                  |   ✅     |    📆     |     ✅      |     ✅       |      🏗        |    🏗     |
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