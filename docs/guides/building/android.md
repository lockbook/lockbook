# Android Building

## Setting Up Build Environment

- You will need the stable Rust toolchain installed.
- Download and install the Android Software Development Kit (SDK) Manager, along with the Native Development Kit (NDK).
- Ensure that the SDK mentioned in the [this build.gradle](/clients/android/app/build.gradle) is installed.
The SDK version is mentioned at the `targetSdkVersion` field.
- Set the environment variable `ANDROID_NDK_HOME` to the NDK's location.
- Native development support for cargo:
```shell script
cargo install cargo-ndk
```
- Android targets for cargo:
```shell script
rustup target add aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android
```

## Generating Apk
- Go to the [core folder](/core) and `make android`.
- You can either retrieve the lockbook apk and run it locally via commands or Android Studio

### Commands
- Using the terminal, enter Lockbook Android's sourcecode at `clients/android`.
- Run `./gradlew assemble` to generate the APK.
- The apk will be located at `clients/android/app/build/outputs/apk/debug`.

### Android Studio
- Download and install Android Studio
- Using Android Studio, open Lockbook Android at the [android folder](/clients/android).
- Configure Android Studio to use the SDK installed.
- Build the APK using the hammer button on the toolbar. Once built, the bottom information
  bar will give you the option to `locate` the apk.
  The location of the apk is at `clients/android/app/build/outputs/apk/debug/`.
- You can also run the APK directly onto your device. You first enable usb debugging on your Android
  Device, and then connect it to the machine running Android Studio. Then, on the toolbar, you will be given the
  option to directly run the apk on your device.
