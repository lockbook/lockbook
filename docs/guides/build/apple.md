# Build Apple

Prerequisites:
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

Steps:
1. In `/libs/lb/lb_external_interface` run `make swift_libs` which will generate lb-rs libs and place them into the correct location within your Xcode project.
2. In `/libs/content/workspace-ffi/SwiftWorkspace` run `./create_libs.sh` to create the ffi binary artifact in `/libs/content/workspace-ffi/SwiftWorkspace/Libs/workspace.xcframework`.
3. Open the `clients/apple` folder in Xcode.
4. In Xcode, open the "Signing & Capabilities" tab and set the team to your personal developer email.
5. Change the build target to macos or ios and hit the Run button.
