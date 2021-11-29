// swift-tools-version:5.2
// The swift-tools-version declares the minimum version of Swift required to build this package.

import PackageDescription 
let package = Package( 
    name: "CLockbookCore",
    products: [
        .library(name: "CLockbookCore", targets: ["CLockbookCore"]),
    ],
    targets: [
        .target(name: "CLockbookCore")
    ]
)
