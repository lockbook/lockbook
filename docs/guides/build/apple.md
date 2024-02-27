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
In `lockbook` root directory run:
```
LBROOT=$(pwd) && cd $LBROOT/libs/lb/lb_external_interface && make swift_libs && cd $LBROOT/libs/content/workspace-ffi/SwiftWorkspace && ./create_libs.sh && cd $LBROOT
```
This runs
- `make swift_libs` in [`/libs/lb/lb_external_interface`](/libs/lb/lb_external_interface) which will generate lb-rs libs and place them into the correct location within your Xcode project.
- `create_libs.sh` in [`/libs/content/workspace-ffi/SwiftWorkspace`](/libs/content/workspace-ffi/SwiftWorkspace)

In general, when pulling new changes, you should only need to `make swift_libs` after the swift workspace has been set up once, so the command will simplify to:
```
LBROOT=$(pwd) && cd $LBROOT/libs/lb/lb_external_interface && make swift_libs && cd $LBROOT
```

### Build in Xcode
After building the native libs:
1. Open the `clients/apple` folder in Xcode.
2. In Xcode, open the "Signing & Capabilities" tab and set the team to your personal developer email.
3. Change the build target to MacOS or iOS and hit the Run button.
