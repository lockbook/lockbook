// swift-tools-version:5.8
import PackageDescription

let package = Package(
    name: "SwiftWorkspace",
    platforms: [
        .macOS(.v13), .iOS(.v16)
    ],
    products: [
        // Products define the executables and libraries a package produces, and make them visible to other packages.
        .library(
            name: "SwiftWorkspace",
            targets: ["SwiftWorkspace"]),
        .library(
            name: "Bridge",
            targets: ["Bridge"]),
        .library(
            name: "workspace",
            targets: ["workspace"])
    ],
    targets: [
        .target(
            name: "SwiftWorkspace",
            dependencies: ["Bridge"],
            path: "Sources/Workspace"
        ),
        .target(
            name: "Bridge",
            dependencies: ["workspace"],
            path: "Sources/Bridge"
        ),
        .binaryTarget(
            name: "workspace",
            path: "Libs/workspace.xcframework"
        ),
    ]
)

