import CoreGraphics
import Foundation

protocol ProcessRunning {
    func run(executableURL: URL, arguments: [String]) throws -> Int32
}

protocol ScreenCapturePermissionProviding {
    func preflight() -> Bool
    func request() -> Bool
}

enum ScreenPermissionStatus: String, Equatable, Sendable {
    case granted
    case notGranted = "not_granted"
}

enum ScreenshotCaptureKind: String, Equatable, Sendable {
    case region
    case scrolling
}

enum ScreenshotResult: Equatable, Sendable {
    case captured(URL, ScreenshotCaptureKind, String)
    case cancelled
}

struct ScreenshotSelection: Equatable, Sendable {
    let captureRect: CGRect
    let note: String
}

enum RegionSelectionResult: Equatable, Sendable {
    case selected(ScreenshotSelection)
    case scrollingCaptured(URL)
    case accessibilityRequired
    case cancelled
}

@MainActor
protocol RegionSelecting {
    func selectRegion(outputURL: URL) throws -> RegionSelectionResult
}

@MainActor
struct ScreenshotService {
    private let process: any ProcessRunning
    private let permission: any ScreenCapturePermissionProviding
    private let regionSelector: any RegionSelecting

    init(
        process: any ProcessRunning = SystemProcessRunner(),
        permission: any ScreenCapturePermissionProviding = SystemScreenCapturePermissionProvider(),
        regionSelector: any RegionSelecting = AppKitRegionSelector()
    ) {
        self.process = process
        self.permission = permission
        self.regionSelector = regionSelector
    }

    func permissionStatus() -> ScreenPermissionStatus {
        permission.preflight() ? .granted : .notGranted
    }

    func captureRegion(to outputURL: URL) throws -> ScreenshotResult {
        guard permission.preflight() || permission.request() else {
            throw NativeServiceError.screenPermissionDenied
        }

        let selection: ScreenshotSelection
        switch try regionSelector.selectRegion(outputURL: outputURL) {
        case let .selected(value):
            selection = value
        case let .scrollingCaptured(url):
            return .captured(url, .scrolling, "")
        case .accessibilityRequired:
            throw NativeServiceError.accessibilityNotTrusted
        case .cancelled:
            return .cancelled
        }

        let exitCode: Int32
        do {
            Thread.sleep(forTimeInterval: 0.08)
            exitCode = try process.run(
                executableURL: URL(fileURLWithPath: "/usr/sbin/screencapture"),
                arguments: ["-x", captureArgument(selection.captureRect), outputURL.path]
            )
        } catch {
            throw NativeServiceError.screenshotFailed
        }

        guard exitCode == 0 else {
            throw NativeServiceError.screenshotFailed
        }
        guard FileManager.default.fileExists(atPath: outputURL.path) else {
            return .cancelled
        }
        return .captured(outputURL, .region, selection.note)
    }
}

private func captureArgument(_ rect: CGRect) -> String {
    let x = Int(rect.minX.rounded())
    let y = Int(rect.minY.rounded())
    let width = Int(rect.width.rounded())
    let height = Int(rect.height.rounded())
    return "-R\(x),\(y),\(width),\(height)"
}

private struct SystemProcessRunner: ProcessRunning {
    func run(executableURL: URL, arguments: [String]) throws -> Int32 {
        let process = Process()
        process.executableURL = executableURL
        process.arguments = arguments
        try process.run()
        process.waitUntilExit()
        return process.terminationStatus
    }
}

private struct SystemScreenCapturePermissionProvider: ScreenCapturePermissionProviding {
    func preflight() -> Bool {
        CGPreflightScreenCaptureAccess()
    }

    func request() -> Bool {
        CGRequestScreenCaptureAccess()
    }
}
