# Build Apple

### Prerequisites:
1. Computer with macOS
2. Standard iOS/macOS toolchain (Xcode)
3. Stable rust toolchain
4. `cbindgen`, which generates header files
```
cargo install cbindgen
```
5. Toolchain targets for building lb-rs for iOS, macOS, and various simulator targets
```bash
rustup target add aarch64-apple-ios x86_64-apple-ios aarch64-apple-darwin x86_64-apple-darwin aarch64-apple-ios-sim
```

### Build native libraries and bootstrap swift workspace
From our repository root, install `lbdev`, our dev utility:
```
cargo install --path utils/lbdev
```

This will let you execute `lbdev apple ws all`. This will now allow you to build in XCode.

### Build in Xcode
After building the native libs:
1. Open the `clients/apple` folder in Xcode.
2. In Xcode, open the "Signing & Capabilities" tab and set the team to your personal developer email.
3. Change the build target to MacOS or iOS and hit the Run button.

## Launching via `lbdev`
XCode needs to be able to do these things before `lbdev` will start to work.

`lbdev apple run macos` will build all the rust and swift and run the macOS app to the current Macbook.

`lbdev apple run ios <device>`  allows you to select an iOS device and send a build to it. This is mostly a shortcut for people who have setup these devices already in XCode. 