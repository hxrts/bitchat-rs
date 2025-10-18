// swift-tools-version: 5.9
// The swift-tools-version declares the minimum version of Swift required to build this package.

import PackageDescription

let package = Package(
    name: "BitChatSwiftCLI",
    platforms: [
        .macOS(.v12)
    ],
    dependencies: [
        // Add BitChat Swift SDK dependency when available
        // .package(path: "../../swift"),
        
        // For now, we'll use ArgumentParser for CLI
        .package(url: "https://github.com/apple/swift-argument-parser", from: "1.0.0"),
        
        // WebSocket client for Nostr
        .package(url: "https://github.com/vapor/websocket-kit.git", from: "2.0.0"),
        
        // JSON handling
        .package(url: "https://github.com/Flight-School/AnyCodable", from: "0.6.0"),
        
        // Logging
        .package(url: "https://github.com/apple/swift-log.git", from: "1.0.0"),
        
        // Crypto
        .package(url: "https://github.com/apple/swift-crypto.git", from: "2.0.0"),
    ],
    targets: [
        .executableTarget(
            name: "bitchat-swift-cli",
            dependencies: [
                .product(name: "ArgumentParser", package: "swift-argument-parser"),
                .product(name: "WebSocketKit", package: "websocket-kit"),
                .product(name: "AnyCodable", package: "AnyCodable"),
                .product(name: "Logging", package: "swift-log"),
                .product(name: "Crypto", package: "swift-crypto"),
            ],
            path: "Sources"
        ),
    ]
)