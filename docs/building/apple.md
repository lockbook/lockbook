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

## Can't run in Xcode?

1. **Run `lbdev apple ws all` first** – Xcode needs the native libs. From repo root: `cargo install --path utils/lbdev` then `lbdev apple ws all`.
2. **Open the workspace** – Open `clients/apple/Lockbook.xcworkspace` (not `lockbook.xcodeproj`).
3. **Signing** – Xcode → Signing & Capabilities → set Team to your Apple ID. The project uses team `39ZS78S25U`; if you're not on that team, change it to yours or use "Sign to Run Locally" for simulators.
4. **Simulator destination** – For iOS, pick a simulator in the scheme selector (e.g. "iPhone 16" or "iPhone 16e"). If "iPhone 16" fails, try another like "iPhone 16e" or "iPhone 17".
5. **macOS code signing** – If macOS fails with "No signing certificate", either add your "Mac Development" cert in Keychain, or build with: `xcodebuild ... CODE_SIGN_IDENTITY="-" build`. 
