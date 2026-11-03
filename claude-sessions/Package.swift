// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "ClaudeSessions",
    platforms: [.macOS(.v14)],
    targets: [
        .target(name: "ClaudeSessionsKit"),
        .executableTarget(name: "ClaudeSessions", dependencies: ["ClaudeSessionsKit"]),
        .testTarget(name: "ClaudeSessionsKitTests", dependencies: ["ClaudeSessionsKit"]),
    ],
    swiftLanguageModes: [.v5]
)
