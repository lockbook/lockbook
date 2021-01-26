Our repository contains many projects which facilitate the development of Lockbook. This document describes the various build strategies used to build Lockbook components.

Understanding the [system architecture](system-architecture.md) is a prerequisite for getting the most out of this document.

# Clients

Lockbook is designed to make it easy to create high quality native applications for many platforms. We achieve this by putting all the business logic that [clients are responsible for](system-architecture.md) in a [core library](../core) which is written in Rust.

Writing core in Rust allows us to perform FFI calls with C-like overhead, with the safety and productivity of high level languages. Reusing this code across all of our clients makes the addition of clients inexpensive and the quality of core very high.

As our server is also written in Rust, we can also share code between clients and servers. This allows us to check the contract between these two components at compile time in a really lightweight way (compared to something like gRPC).

Building and running unit tests for core is straightforward. With a stable Rust toolchain you simply `cargo run` or `cargo test`.

However, building clients has a varied list of hardware and software requirements. They are listed below in order of straightforwardness.

More specific instructions for things like installing the stable Rust toolchain can be found [here](#reference-instructions).

## CLI

CLI is the most straightforward client. You can build it on any machine and don't need anything in addition to the stable Rust toolchain.

Simply go into the [CLI Folder](../clients/cli) and `cargo run`.

## Linux

In order to build the Linux client, you need the stable Rust toolchain on a
Linux distro with GTK installed. Then, go into the [Linux
folder](../clients/linux) and `cargo run`.

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


## Windows

You need a Windows computer, and you need to set yourself up for [UWP development](https://docs.microsoft.com/en-us/windows/uwp/get-started/get-set-up).

You'll also need the [Rust toolchain](https://rustup.rs/) to build core.

You can build `core` for Windows by executing the `create_core_for_windows.bat` script from inside `dev_utils`.

At this point you should be able to click the green play button inside Visual Studio.

To create the executable:

Run the `create_windows_app_bundle.bat` in `dev_utils`.

## iOS (iPhone and iPad)

Standard iOS development toolchain (XCode).

The header maker (turns your Rust code into C stubs):
```zsh
cargo install cbindgen
```

The fat-library builder:
```zsh
cargo install cargo-lipo
```

iOS Simulator and Device targets for Rust:
```zsh
rustup target add aarch64-apple-ios x86_64-apple-ios
```

`make lib_c_for_swift_ios` in the `core` folder.

## macOS

### Hardware Requirements
+ An Apple-blessed computer

###  Software Requirements
+ Everything Core requires
+ XCode 11+
+ `cbindgen` for creating C headers `cargo install cbindgen`
+ `cargo-lipo` for creating [`Fat Binaries`](https://en.wikipedia.org/wiki/Fat_binary) `cargo install cargo-lipo`
+ The build targets:

```shell script
rustup target add aarch64-apple-ios armv7-apple-ios armv7s-apple-ios x86_64-apple-ios i386-apple-ios
```

## Reference Instructions

### Installing the Rust Toolchain

Download the `rustup` script:
```
$ curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```
You should elect to do a **custom** install such that you can select **stable** and **complete**.

# Server

Building the `server` simply requires you to have the `stable` Rust toolchain. `cargo run` will fetch dependencies and begin running your server.

Running the server, however, will require you to point your server to a `FileDb` and `IndexDb`. You can take a look at our `CI` to see how we do this using containers at test-time.

