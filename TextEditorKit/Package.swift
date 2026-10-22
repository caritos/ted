// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "TextEditorKit",
    platforms: [.iOS(.v17), .macOS(.v14)],
    products: [
        .library(name: "TextEditorKit", targets: ["TextEditorKit"]),
    ],
    targets: [
        .target(name: "TextEditorKit"),
        .testTarget(name: "TextEditorKitTests", dependencies: ["TextEditorKit"]),
    ]
)
