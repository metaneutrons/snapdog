// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "SnapDogHelper",
    platforms: [
        .macOS(.v15),
    ],
    targets: [
        .executableTarget(
            name: "SnapDogHelper",
            swiftSettings: [
                .enableExperimentalFeature("StrictConcurrency"),
            ]
        ),
    ]
)
