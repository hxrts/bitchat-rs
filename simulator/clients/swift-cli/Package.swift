// swift-tools-version: 5.8
// The swift-tools-version declares the minimum version of Swift required to build this package.

import PackageDescription

let package = Package(
    name: "BitChatSwiftCLI",
    platforms: [
        .macOS(.v11)
    ],
    dependencies: [
        // Essential dependencies only for Nix compatibility
        .package(url: "https://github.com/apple/swift-argument-parser", from: "1.0.0"),
        .package(url: "https://github.com/apple/swift-log.git", from: "1.0.0"),
    ],
    targets: [
        .executableTarget(
            name: "bitchat-swift-cli",
            dependencies: [
                .product(name: "ArgumentParser", package: "swift-argument-parser"),
                .product(name: "Logging", package: "swift-log"),
            ],
            path: "Sources"
        ),
    ]
)