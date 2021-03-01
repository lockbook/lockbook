// swift-tools-version:5.3
// The swift-tools-version declares the minimum version of Swift required to build this package.

import PackageDescription

let package = Package(
    name: "NotepadSwift",
    products: [
        // Products define the executables and libraries a package produces, and make them visible to other packages.
        .library(
            name: "NotepadSwift",
            targets: ["NotepadSwift"]),
    ],
    dependencies: [
        // Dependencies declare other packages that this package depends on.
        // .package(url: /* package url */, from: "1.0.0"),
    ],
    targets: [
        // Targets are the basic building blocks of a package. A target can define a module or a test suite.
        // Targets can depend on other targets in this package, and on products in packages this package depends on.
        .target(
            name: "NotepadSwift",
            dependencies: [],
            resources: [
                .process("themes/base16-tomorrow-dark.json"),
                .process("themes/base16-tomorrow-light.json"),
                .process("themes/blues-clues.json"),
                .process("themes/one-dark-custom.json"),
                .process("themes/one-dark.json"),
                .process("themes/one-light-custom.json"),
                .process("themes/one-light.json"),
                .process("themes/solarized-dark.json"),
                .process("themes/solarized-light.json"),
                .process("themes/system-minimal.json")
            ]),
    ]
)
