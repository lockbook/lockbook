# Build Android

Prerequisites:
- Stable rust toolchain.
- Android SDK. The version mentioned in [this build.gradle](/clients/android/app/build.gradle), at the `targetSdkVersion` field.
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
