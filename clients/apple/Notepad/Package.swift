// swift-tools-version:5.3

import PackageDescription

let package = Package(
    name: "NotepadSwift",
    platforms: [
        .macOS("10.15"),
        .iOS("13.0"),
    ],
    products: [
        .library(
            name: "NotepadSwift",
            targets: ["NotepadSwift"]),
    ],
    dependencies: [
        .package(
            url: "https://github.com/johnxnguyen/Down.git",
            from: "0.10.0"
        ),
    ],
    targets: [
        .target(
            name: "NotepadSwift",
            dependencies: ["Down"]),
        .testTarget(
            name: "NotepadSwiftTests",
            dependencies: ["NotepadSwift"]),
    ]
)
