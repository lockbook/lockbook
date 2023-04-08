# Releasing

Lockbook automates the releasing process using [releaser](../../utils/releaser). At the project root, run `cargo run -p releaser -- all` to release every platform able to be released on your OS. Each platform requires its own set of considerations to be able to be released.

## Deploy Server

There are no required environment variables. 

The following commands must be present on your system: `ssh`, `curl`. Your ssh keys must be on your system for the server you plan to deploy to. 

To deploy only to server, run `cargo run -p releaser -- deploy-server`.

## Release Apple

This platform can only be released from macOS.

The following environment variables must be present on your system:
- `GITHUB_TOKEN`: a token from github that is authorized to release to tags
- `APPLE_ID_PASSWORD`: the password of your apple developer account

The dependencies needed to release are the same needed to build, please refer to the "Prerequisites" step of the [apple build guide](build/apple.md). `git` must also be installed.

To only release to apple's platforms: run `cargo run -p releaser -- release-apple`.

## Release Android

The following environment variables must be present on your system:
- `GITHUB_TOKEN`: a token from github that is authorized to release to tags
- `GOOGLE_CLOUD_SERVICE_ACCOUNT_KEY`: a google cloud service account private key authorized to make changes to your google play developer console 
- `ANDROID_RELEASE_STORE_FILE`: an absolute path to the key store file used to sign the completed apk and app bundle
- `ANDROID_RELEASE_STORE_PASSWORD`: the password used to access the key store file
- `ANDROID_RELEASE_KEY_ALIAS`: the alias used to access the key store file
- `ANDROID_RELEASE_KEY_PASSWORD`: the password of the alias used to access the key store file

The dependencies needed to release are the same needed to build, please refer to the "Prerequisites" step of the [android build guide](build/android.md). `git` must also be installed.

To only release to android's platforms: run `cargo run -p releaser -- release-android`.

## Release Windows

This platform can only be released from windows.

The following environment variables must be present on your system:
- `GITHUB_TOKEN`: a token from github that is authorized to release to tags

The dependencies needed to release are the same needed to build, please refer to the "Prerequisites" step of the [windows build guide](build/windows.md). `git` must also be installed.

To release only to windows: run `cargo run -p releaser -- release-windows`.

## Release Public Site

The following environment variables must be present on your system:
- `GITHUB_TOKEN`: a token from github that is authorized to edit your github pages

To only release the public site: run `cargo run -p releaser -- release-public-site`.

## Releasing Linux

This platform can only be released from a ubuntu machine.

The following environment variables must be present on your system:
- `GITHUB_TOKEN`: a token from github that is authorized to release to tags

The dependencies needed to release include the same needed to build, please refer to the "Prerequisites" step of the [linux build guide](build/linux.md). Besides `git`, `snapcraft` is needed to release to debian based platforms.

The steps to setup `snapcraft`:
1. Create a ubuntu developer account at the [snapcraft store](https://snapcraft.io).
2. Install the [snap daemon](https://snapcraft.io/docs/installing-snapd).
3. Install `snapcraft` using `snap install --classic snapcraft`.
4. Login to your ubuntu developer account using `snapcraft login`.

In addition, you must clone the [lockbook aur repository](https://github.com/lockbook/aur-lockbook) and the [lockbook desktop aur repository](https://github.com/lockbook/aur-lockbook-desktop). These two repositories must exist in the same directory the lockbook monorepo is located in. You must also set the remote repository aliases, `github` and `aur`, to their respective urls for each repository. 

To release only to linux: run `cargo run -p releaser -- release-linux`.
