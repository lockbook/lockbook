Our repository contains many projects which facilitate the development of Lockbook. This document descibes the various build strategies used to build lockbook components.

Understanding the [system architecture](system-architecture.md) is a prerequisite for getting the most out of this document.

# Clients

Lockbook is designed to make it easy to create high quality native applications for many platforms. We achieve this by putting all the business logic that [clients are responsible for](system-architecture.md) in a [core library](../core) which is written in rust.

Writing core in rust allows us to perform FFI calls with C-like overhead, with the safety and productivity of high level languages. Reusing this code across all our clients makes the addition of clients inexpensive and the quality of core very high.

As our server is also written in rust, we can also share code between clients and servers. This allows us to check the contract between these two components at compile time in a really lightweight way (compared to something like gRPC).

Building and running unit tests for core is straightforward. With a stable rust toolchain you simply `cargo run` or `cargo test`.

Building clients however has a varied list of hardware, and software requirements. They are listed below in order of straightforwardness.

More specific instructions for things like installing the stable rust toolchain can be found [here](#reference-instructions).

## Cli

Cli is the most straightforward client. You can build it on any machine and don't need anything in addition to the stable rust toolchain.

Simply go into the [Cli Folder](../clients/cli) and `cargo run`.

## Android

Standard Android development toolchain, along with the native development kit.

Native development support for cargo:
```shell script
cargo install cargo-ndk
```

Android targets for cargo:
```shell script
rustup target add aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android
```

Download the android ndk through android studio or directly online from the android developers website. Extract it and set the environment variable `ANDROID_NDK_HOME` to its location.

`make android` in the `core` folder.

## iOS, iPadOS, and macOS

### Hardware Requirements
+ An Apple-blessed computer

###  Software Requirements
+ Everything Core requires
+ XCode 11+
+ `cbindgen` for creating c headers `cargo install cbindgen`
+ `cargo-lipo` for creating `[Fat Binaries](https://en.wikipedia.org/wiki/Fat_binary) `cargo install cargo-lipo`
+ The build targets:

```shell script
rustup target add aarch64-apple-ios armv7-apple-ios armv7s-apple-ios x86_64-apple-ios i386-apple-ios
```

## Reference Instructions

### Installing the rust toolchain

curl down the `rustup` script.
```
$ curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```
You should elect to do a **custom** install such that you can select **stable** and **complete**.

# Server

Building `server` simply requires you to have the `stable` rust toolchain. `cargo run` will fetch dependencies and begin running your server.

Running server however will require you to point your server to a `FileDb` and `IndexDb`. You can take a look at our `CI` to see how we do this using containers at test-time.

