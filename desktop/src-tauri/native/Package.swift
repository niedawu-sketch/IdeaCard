// swift-tools-version: 6.2

import PackageDescription

let package = Package(
    name: "IdeaCardNative",
    platforms: [.macOS(.v14)],
    products: [
        .executable(name: "ideacard-native", targets: ["IdeaCardNative"]),
    ],
    targets: [
        .executableTarget(name: "IdeaCardNative"),
        .testTarget(name: "IdeaCardNativeTests", dependencies: ["IdeaCardNative"]),
    ]
)
