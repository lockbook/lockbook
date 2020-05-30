# Projects

In this monorepo you will find code for our:
+ [Server](../server) the rust code that runs on our [API Nodes](overview.md).
+ [Core](../core) the rust code that contains all our core business logic. Every client uses this.
+ [Clients](../clients) Native code for various platforms and devices.

# Core

## Requirements

+ rust toolchain

## Setup

### Installing Rust

curl and execute the `rustup` script
```shell script
$ curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```
You should choose a **custom** install and select **complete**.

### Build Core

Navigate to the core folder and use cargo to build
```shell script
$ cd ./core
$ cargo build
``` 

# iOS, iPadOs, macOS

## Requirements
+ Everything Core requires
+ XCode 11+
+ `cbindgen` for creating c headers `cargo install cbindgen`
+ `cargo-lipo` for creating `[Fat Binaries](https://en.wikipedia.org/wiki/Fat_binary) `cargo install cargo-lipo`
