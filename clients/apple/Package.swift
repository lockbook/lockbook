// swift-tools-version: 5.9
import PackageDescription

// Test diagnostics without building the app's Rust workspace XCFramework.
let package = Package(
    name: "LockbookDiagnostics",
    platforms: [.macOS(.v12), .iOS(.v15)],
    targets: [
        .target(name: "LockbookDiagnostics", path: "App/Diagnostics"),
        .testTarget(name: "LockbookDiagnosticsTests", dependencies: ["LockbookDiagnostics"], path: "Tests"),
    ]
)
