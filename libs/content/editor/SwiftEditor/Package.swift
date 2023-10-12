// swift-tools-version:5.8
import PackageDescription

let package = Package(
    name: "SwiftEditor",
    platforms: [
        .macOS(.v12), .iOS(.v16)
    ],
    products: [
        // Products define the executables and libraries a package produces, and make them visible to other packages.
        .library(
            name: "SwiftEditor",
            targets: ["SwiftEditor"]),
        .library(
            name: "Bridge",
            targets: ["Bridge"]),
        .library(
            name: "egui_editor",
            targets: ["egui_editor"])
    ],
    targets: [
        .target(
            name: "SwiftEditor",
            dependencies: ["Bridge"],
            path: "Sources/Editor"
        ),
        .target(
            name: "Bridge",
            dependencies: ["egui_editor"],
            path: "Sources/Bridge"
        ),
        .binaryTarget(
            name: "egui_editor",
            path: "Libs/egui_editor.xcframework"
        ),
    ]
)

