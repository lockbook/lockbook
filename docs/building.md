# Building
Being a *Rust* project, the rust toolchain is the dependency that all clients will require. For many of our pure-rust clients and libraries in our monorepo, this is all that's required! Jump into `clients/cli` and `cargo run -r`.

For our project, you just need `rustup`, our `rust-toolchain.toml` in the root of our project will ensure that you're using the right version of the toolchain. Checkout https://rustup.rs/ for more information.

We generally don't depend on cross-compilation with the rust-toolchain, although in a pinch `cargo-zigbuild` (zig toolchain) and `cargo-cross` (docker) have worked well for us.

Below you'll find the documentation associated with more complicated workflows. 

# CLI
```bash
cd clients/cli && cargo install --path .
```

See [cli](cli.md) for information about setting up completions.

# Windows
```bash
cd clients/desktop && cargo install --path .
```

# Linux
Commonly required dependencies:
* `build-essential`
+ `libssl-devel`
+ `pkg-config`
+ `libxkbcommon-x11-dev`
+ `libgtk-3-dev` (for rfd)

See `clients/linux/default.nix` for the formal specification. Nix-users can just `nix-shell` in this directory to fulfill the dependency requirements.

Once dependencies are fulfilled: 
```bash
cd clients/desktop && cargo install --path .
``` 

# Android
Dependencies:
- Android SDK. The version mentioned in [this build.gradle](/clients/android/app/build.gradle) ([gh](https://github.com/lockbook/lockbook/blob/master/clients/android/app/build.gradle)), at the `targetSdkVersion` field.
- Android NDK. Set the environment variable `ANDROID_NDK_HOME` to the NDK's location.
- Native development support for cargo:
```shell script
cargo install cargo-ndk
```
- Android targets for cargo:
```shell script
rustup target add aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android
```

Steps:
- Run `cargo run -p lbdev -- android ws` which will build workspace for android.
- Choose one:
    1. Command Line
        - In `clients/android` run `./gradlew assemble`.
        - The APK will be located at `clients/android/app/build/outputs/apk/debug`.
    2. Android Studio
        - Download and install Android Studio.
        - In Android Studio open Lockbook Android at the `/clients/android`.
        - Configure Android Studio to use the SDK installed.
        - Build the APK using the hammer button on the toolbar. Once built, the bottom information
            bar will give you the option to locate the APK (`clients/android/app/build/outputs/apk/debug/`).
        - You can also run the APK directly on your device:
            - Enable USB debugging on your Android device.
            - Connect it to the machine running Android Studio.
            - On the toolbar, you will be given the option to directly run the APK on your device.

# Apple
Dependencies:
* Xcode
* `cbindgen` for generating `.h` files for Swift <--> Rust interop
```bash
    cargo install cbindgen
```
* Apple Specific rust toolchains:
```bash
rustup target add aarch64-apple-ios x86_64-apple-ios aarch64-apple-darwin x86_64-apple-darwin aarch64-apple-ios-sim
```

## lbdev
See [lbdev](lbdev.md) for more information about our rust build utility. As an Apple dev you'll want to install `lbdev` for easy access. `lbdev apple ws all` will produce the libraries Xcode expects to perform lockbook operations.
```bash
cd utils/lbdev && cargo install --path . && lbdev apple ws all
```

## Configuring Xcode
After building the native libs:
1. Open `clients/apple` in Xcode
1. In Xcode, open the "Signing & Capabilities" tab and set the team to your personal developer email.
1. Make sure the target is `Lockbook`, then select a device (macOS, iOS device, or iOS simulator).
1. Hit the run button. If you see an App, great. If you can edit a document you have everything wired up correctly. 

## Troubleshooting
1. Restart Xcode. Xcode isn't that robust, and often times if you re-build the Rust libs or do git operations you'll encounter false build errors often resolved by restarting Xcode. 
1. **Signing** – Xcode → Signing & Capabilities → set Team to your Apple ID. The project uses team `39ZS78S25U`; if you're not on that team, change it to yours or use "Sign to Run Locally" for simulators.
1. **macOS code signing** – If macOS fails with "No signing certificate", either add your "Mac Development" cert in Keychain, or build with: `xcodebuild ... CODE_SIGN_IDENTITY="-" build`. 
1. Reach out for help on discord: https://discord.gg/lockbook

## Tips
* If you have your `Swift` hat on, you'll want to execute `lbdev apple ws all` anytime lb-rs changes
* If you have your `Rust` hat on, and just want to quickly check something on Apple you can use `lbdev apple run` variants. `lbdev` has tab completions that will enumerate devices (`lbdev apple run ios <tab>`). Similarly you can just do `lbdev apple run macos` to run on your current device. 

# lb-rs & server
lb-rs & server have no system dependencies.

lb-rs can be pointed to a server via the `API_URL` env var, by default clients are engineered to connect to our production server: `https://api.prod.lockbook.net`. See [self-hosting](self-hosting.md) for more information.

Tests are configured to connect to `https://localhost:8000`.

You can run the server locally by executing `lbdev server` for local dev.