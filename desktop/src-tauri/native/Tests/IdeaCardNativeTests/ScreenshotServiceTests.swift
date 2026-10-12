import Foundation
import Testing
@testable import IdeaCardNative

@Suite("ScreenshotServiceTests")
@MainActor
struct ScreenshotServiceTests {
    @Test("Keeps the unstable scrolling capture entry disabled")
    func scrollingCaptureEntryIsDisabled() {
        #expect(ScreenshotFeatureFlags.scrollingCaptureEnabled == false)
    }

    @Test("Uses the confirmed region without a shell")
    func passesFixedArgumentsToScreenCapture() throws {
        let outputURL = temporaryOutputURL()
        defer { try? FileManager.default.removeItem(at: outputURL) }
        let process = FakeProcessRunner(exitCode: 0, createOutput: true)
        let permission = FakeScreenPermissionProvider(preflightGranted: true)
        let service = ScreenshotService(
            process: process,
            permission: permission,
            regionSelector: FakeRegionSelector(.selected(ScreenshotSelection(
                captureRect: CGRect(x: 12, y: 34, width: 400, height: 220),
                note: "截图时想到的事"
            )))
        )

        let result = try service.captureRegion(to: outputURL)

        #expect(result == .captured(outputURL, .region, "截图时想到的事"))
        #expect(process.executableURL?.path == "/usr/sbin/screencapture")
        #expect(process.arguments == ["-x", "-R12,34,400,220", outputURL.path])
        #expect(permission.requestCount == 0)
    }

    @Test("Cancels before starting capture when the confirmation is dismissed")
    func dismissedSelectionMeansCancelled() throws {
        let process = FakeProcessRunner(exitCode: 0, createOutput: true)
        let service = ScreenshotService(
            process: process,
            permission: FakeScreenPermissionProvider(preflightGranted: true),
            regionSelector: FakeRegionSelector(.cancelled)
        )

        #expect(try service.captureRegion(to: temporaryOutputURL()) == .cancelled)
        #expect(process.executableURL == nil)
    }

    @Test("Maps a nonzero command exit to a stable error")
    func nonzeroExitFails() {
        let process = FakeProcessRunner(exitCode: 1, createOutput: false)
        let service = ScreenshotService(
            process: process,
            permission: FakeScreenPermissionProvider(preflightGranted: true),
            regionSelector: FakeRegionSelector(.selected(ScreenshotSelection(
                captureRect: CGRect(x: 12, y: 34, width: 400, height: 220),
                note: ""
            )))
        )

        expectServiceError(code: "screenshot_failed") {
            _ = try service.captureRegion(to: temporaryOutputURL())
        }
    }

    @Test("Only requests screen access when a capture starts")
    func permissionRequestIsCaptureOnly() {
        let permission = FakeScreenPermissionProvider(
            preflightGranted: false,
            requestGranted: false
        )
        let service = ScreenshotService(
            process: FakeProcessRunner(exitCode: 0, createOutput: false),
            permission: permission,
            regionSelector: FakeRegionSelector(.cancelled)
        )

        #expect(service.permissionStatus() == .notGranted)
        #expect(permission.requestCount == 0)
        expectServiceError(code: "screen_permission_denied") {
            _ = try service.captureRegion(to: temporaryOutputURL())
        }
        #expect(permission.requestCount == 1)
    }

    @Test("Rejects a relative output path before starting a process")
    func relativePathIsRejected() {
        let process = FakeProcessRunner(exitCode: 0, createOutput: true)
        let service = ScreenshotService(
            process: process,
            permission: FakeScreenPermissionProvider(preflightGranted: true),
            regionSelector: FakeRegionSelector(.cancelled)
        )

        let response = dispatchNativeRequest(
            NativeRequest(
                command: .screenshot,
                path: "relative/capture.png",
                service: nil,
                account: nil,
                secret: nil
            ),
            screenshotService: service
        )

        #expect(response.ok == false)
        #expect(response.errorCode == "invalid_request")
        #expect(process.executableURL == nil)
    }

    @Test("Returns cancellation through the native protocol")
    func protocolReturnsCancellation() {
        let service = ScreenshotService(
            process: FakeProcessRunner(exitCode: 0, createOutput: false),
            permission: FakeScreenPermissionProvider(preflightGranted: true),
            regionSelector: FakeRegionSelector(.cancelled)
        )

        let response = dispatchNativeRequest(
            NativeRequest(
                command: .screenshot,
                path: temporaryOutputURL().path,
                service: nil,
                account: nil,
                secret: nil
            ),
            screenshotService: service
        )

        #expect(response.ok)
        #expect(response.cancelled == true)
        #expect(response.path == nil)
    }

    @Test("Returns the optional note through the screenshot protocol")
    func protocolReturnsTheScreenshotNote() {
        let outputURL = temporaryOutputURL()
        defer { try? FileManager.default.removeItem(at: outputURL) }
        let service = ScreenshotService(
            process: FakeProcessRunner(exitCode: 0, createOutput: true),
            permission: FakeScreenPermissionProvider(preflightGranted: true),
            regionSelector: FakeRegionSelector(.selected(ScreenshotSelection(
                captureRect: CGRect(x: 12, y: 34, width: 400, height: 220),
                note: "截图时想到的事"
            )))
        )

        let response = dispatchNativeRequest(
            NativeRequest(
                command: .screenshot,
                path: outputURL.path,
                service: nil,
                account: nil,
                secret: nil
            ),
            screenshotService: service
        )

        #expect(response.ok)
        #expect(response.path == outputURL.path)
        #expect(response.captureKind == "region")
        #expect(response.text == "截图时想到的事")
    }

    @Test("Returns a completed scrolling capture without starting screencapture")
    func completedScrollingCaptureUsesItsExistingFile() throws {
        let outputURL = temporaryOutputURL()
        let process = FakeProcessRunner(exitCode: 0, createOutput: false)
        let service = ScreenshotService(
            process: process,
            permission: FakeScreenPermissionProvider(preflightGranted: true),
            regionSelector: FakeRegionSelector(.scrollingCaptured(outputURL))
        )

        #expect(try service.captureRegion(to: outputURL) == .captured(outputURL, .scrolling, ""))
        #expect(process.executableURL == nil)

        let response = dispatchNativeRequest(
            NativeRequest(
                command: .screenshot,
                path: outputURL.path,
                service: nil,
                account: nil,
                secret: nil
            ),
            screenshotService: service
        )
        #expect(response.ok)
        #expect(response.path == outputURL.path)
        #expect(response.captureKind == "scrolling")
        #expect(response.text == "")
    }

    @Test("Maps an accessibility requirement to a stable error")
    func accessibilityRequirementFails() {
        let process = FakeProcessRunner(exitCode: 0, createOutput: false)
        let service = ScreenshotService(
            process: process,
            permission: FakeScreenPermissionProvider(preflightGranted: true),
            regionSelector: FakeRegionSelector(.accessibilityRequired)
        )

        expectServiceError(code: "accessibility_not_trusted") {
            _ = try service.captureRegion(to: temporaryOutputURL())
        }
        #expect(process.executableURL == nil)
    }

    private func temporaryOutputURL() -> URL {
        FileManager.default.temporaryDirectory
            .appending(path: "ideacard-capture-\(UUID().uuidString).png")
    }

    private func expectServiceError(code: String, operation: () throws -> Void) {
        do {
            try operation()
            Issue.record("Expected NativeServiceError with code \(code)")
        } catch let error as NativeServiceError {
            #expect(error.code == code)
        } catch {
            Issue.record("Expected NativeServiceError, got \(error)")
        }
    }
}

@MainActor
private struct FakeRegionSelector: RegionSelecting {
    let result: RegionSelectionResult

    init(_ result: RegionSelectionResult) {
        self.result = result
    }

    func selectRegion(outputURL: URL) throws -> RegionSelectionResult {
        result
    }
}

private final class FakeProcessRunner: ProcessRunning {
    private let exitCode: Int32
    private let createOutput: Bool
    private(set) var executableURL: URL?
    private(set) var arguments: [String] = []

    init(exitCode: Int32, createOutput: Bool) {
        self.exitCode = exitCode
        self.createOutput = createOutput
    }

    func run(executableURL: URL, arguments: [String]) throws -> Int32 {
        self.executableURL = executableURL
        self.arguments = arguments
        if createOutput, let path = arguments.last {
            try Data("png".utf8).write(to: URL(fileURLWithPath: path))
        }
        return exitCode
    }
}

private final class FakeScreenPermissionProvider: ScreenCapturePermissionProviding {
    private let preflightGranted: Bool
    private let requestGranted: Bool
    private(set) var requestCount = 0

    init(preflightGranted: Bool, requestGranted: Bool = true) {
        self.preflightGranted = preflightGranted
        self.requestGranted = requestGranted
    }

    func preflight() -> Bool {
        preflightGranted
    }

    func request() -> Bool {
        requestCount += 1
        return requestGranted
    }
}
