// swift-tools-version:5.2
// The swift-tools-version declares the minimum version of Swift required to build this package.

import PackageDescription 
let package = Package( 
    name: "CLockbookCore",
    products: [
        .library(name: "CLockbookCore", targets: ["CLockbookCore"]),
    ],
    targets: [
        // Here we export the C static library that we've built with Rust
        // We use a relative path like this because other module (SwiftLockbookCore)
        // End up using the same relative path which I imagine is a bug at the moment
        .target(
            name: "CLockbookCore",
            dependencies: [],
            linkerSettings: [LinkerSetting.unsafeFlags(["-L../CLockbookCore/Sources/CLockbookCore/lib"])]
        )
    ]
)
