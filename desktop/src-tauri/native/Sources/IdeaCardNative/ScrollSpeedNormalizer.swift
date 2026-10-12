import AppKit
import ApplicationServices
import CoreGraphics
import Foundation

enum AutoFinishReason: Equatable, Sendable {
    case contentExhausted
    case heightLimitReached
}

/// Scroll pacing and frame capture share this rate so the page can advance by
/// no more than the amount already proven to keep adjacent frames stitchable.
enum ScrollingCaptureRate {
    static let framesPerSecond: Int32 = 30
    static let maximumPointsPerFrame = 15
    static let fixedSpeedHoldFrames = 8

    static var frameInterval: TimeInterval {
        1.0 / TimeInterval(framesPerSecond)
    }

    static var maximumPointsPerSecond: Int {
        Int(framesPerSecond) * maximumPointsPerFrame
    }
}

/// Posting synthetic scroll events to another process requires Accessibility
/// trust. The helper only checks it; the main app owns prompting, because a
/// prompt from the main bundle registers a proper "IdeaCard" entry in the
/// Accessibility list that this helper then inherits as a child process.
enum SyntheticScrollPermission {
    static var isTrusted: Bool {
        AXIsProcessTrusted()
    }
}

/// Pure pacing logic behind scroll normalization. Input below the stable
/// capture limit passes through unchanged. Once one tick receives enough
/// intent to reach the limit, output is latched to the exact stable speed for
/// several ticks. New intent refreshes that short window instead of increasing
/// its speed, smoothing bursty mouse-wheel delivery without a long runoff.
struct ScrollPacing: Equatable, Sendable {
    let maximumPointsPerTick: Int
    let fixedSpeedHoldTicks: Int
    private var pendingPoints: CGFloat = 0
    private var fixedSpeedTicksRemaining = 0

    init(
        maximumPointsPerTick: Int = ScrollingCaptureRate.maximumPointsPerFrame,
        fixedSpeedHoldTicks: Int = ScrollingCaptureRate.fixedSpeedHoldFrames
    ) {
        self.maximumPointsPerTick = maximumPointsPerTick
        self.fixedSpeedHoldTicks = fixedSpeedHoldTicks
    }

    mutating func ingest(points: CGFloat) {
        guard points.isFinite, points > 0 else { return }
        if fixedSpeedTicksRemaining > 0 {
            fixedSpeedTicksRemaining = fixedSpeedHoldTicks
            return
        }
        pendingPoints += points
        guard pendingPoints >= CGFloat(maximumPointsPerTick) else { return }
        pendingPoints = 0
        fixedSpeedTicksRemaining = fixedSpeedHoldTicks
    }

    /// Returns the exact known-stable amount while the fixed-speed window is
    /// active. Otherwise slower input passes through at its original amount.
    mutating func consumeTick() -> Int {
        if fixedSpeedTicksRemaining > 0 {
            fixedSpeedTicksRemaining -= 1
            return maximumPointsPerTick
        }
        let emitted = min(maximumPointsPerTick, Int(pendingPoints.rounded(.down)))
        guard emitted > 0 else { return 0 }
        pendingPoints -= CGFloat(emitted)
        return emitted
    }
}

/// Detects a stationary page: after this much continuous duplication the
/// content either hit the bottom or cannot scroll at all.
struct BottomDetection: Equatable, Sendable {
    let threshold: TimeInterval
    private var lastChangeAt: Date?

    init(threshold: TimeInterval = 2.0) {
        self.threshold = threshold
    }

    mutating func recordFrame(isDuplicate: Bool, at now: Date = Date()) -> Bool {
        guard isDuplicate else {
            lastChangeAt = now
            return false
        }
        guard let lastChangeAt else {
            // No change seen yet; arm the timer from the first duplicate.
            self.lastChangeAt = now
            return false
        }
        return now.timeIntervalSince(lastChangeAt) >= threshold
    }
}

/// Locates the process that owns the window beneath the selection so
/// synthetic scroll events can be delivered straight to it, unaffected by the
/// capture overlay that swallows the user's real input.
enum WindowTargetLocator {
    static func frontmostPID(at point: CGPoint, excluding ownPID: pid_t) -> pid_t? {
        guard
            let list = CGWindowListCopyWindowInfo(
                [.optionOnScreenOnly, .excludeDesktopElements],
                kCGNullWindowID
            ) as? [[String: Any]]
        else {
            return nil
        }
        // The list is ordered front to back; the first hit is the topmost window.
        for info in list {
            guard
                let pidNumber = info[kCGWindowOwnerPID as String] as? NSNumber,
                let layer = info[kCGWindowLayer as String] as? Int,
                layer == 0,
                let boundsDict = info[kCGWindowBounds as String] as? [String: Any],
                let bounds = CGRect(dictionaryRepresentation: boundsDict as CFDictionary),
                bounds.contains(point)
            else {
                continue
            }
            let pid = pidNumber.int32Value
            guard pid != ownPID else { continue }
            return pid
        }
        return nil
    }
}

/// Reposts the user's downward scroll intent with a stable speed ceiling so
/// stitched frames keep enough overlap. The user keeps control: slow input
/// stays slow, fast input is capped, stopping pauses, and save ends capture.
@MainActor
final class ScrollSpeedNormalizer {
    private let center: CGPoint
    private let targetPID: pid_t
    private let tickInterval: TimeInterval
    private var pacing: ScrollPacing
    private var timer: Timer?
    private(set) var isRunning = false

    init(
        center: CGPoint,
        targetPID: pid_t,
        tickInterval: TimeInterval = ScrollingCaptureRate.frameInterval,
        pacing: ScrollPacing = ScrollPacing()
    ) {
        self.center = center
        self.targetPID = targetPID
        self.tickInterval = tickInterval
        self.pacing = pacing
    }

    func ingest(points: CGFloat) {
        guard isRunning else { return }
        pacing.ingest(points: points)
    }

    func start() {
        guard !isRunning else { return }
        isRunning = true
        let pacingTimer = Timer(timeInterval: tickInterval, repeats: true) {
            [weak self] _ in
            Task { @MainActor [weak self] in
                self?.emitTick()
            }
        }
        // Wheel and trackpad interactions can switch the main run loop into an
        // event-tracking mode. Common mode keeps the speed ceiling active for
        // the entire gesture instead of pausing until the user lifts a finger.
        RunLoop.main.add(pacingTimer, forMode: .common)
        timer = pacingTimer
    }

    func stop() {
        timer?.invalidate()
        timer = nil
        isRunning = false
    }

    private func emitTick() {
        let points = pacing.consumeTick()
        guard points > 0 else { return }
        // Discrete pixel-wheel events behave like a classic mouse wheel: no
        // momentum, no rubber-banding, an exact bounded step every tick.
        guard
            let event = CGEvent(
                scrollWheelEvent2Source: nil,
                units: .pixel,
                wheelCount: 1,
                wheel1: Int32(-points),
                wheel2: 0,
                wheel3: 0
            )
        else {
            return
        }
        event.location = center
        event.postToPid(targetPID)
    }
}
