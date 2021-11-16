# Apple development

Prerequisites:

1. Computer with macOS
2. Standard iOS/macOS toolchain (xcode)
3. Stable rust toolchain
4. `cargo install cbindgen cargo-lipo`. `cbindgen` generates `.h` files for us, and `cargo-lipo`
   generates [fat-libraries](https://en.wikipedia.org/wiki/Fat_binary).
5. Install the following toolchain targets for building `core` for `iOS`, `macOS`, and various simulator targets.
```bash
rustup target add aarch64-apple-ios x86_64-apple-ios armv7-apple-ios armv7s-apple-ios x86_64-apple-ios i386-apple-ios aarch64-apple-darwin x86_64-apple-darwin
```
6. You can run `make swift_libs` which will generate `core` libs and place them into the correct location within your xcode project.
