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
2. Open Xcode, import the project and hit the Run button.
