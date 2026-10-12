import Foundation
import Testing
@testable import IdeaCardNative

@Suite("ScrollPacingTests")
struct ScrollPacingTests {
    @Test("Preserves input below the stable speed limit")
    func preservesSlowerInput() {
        var pacing = ScrollPacing(maximumPointsPerTick: 15, fixedSpeedHoldTicks: 8)
        pacing.ingest(points: 8)
        #expect(pacing.consumeTick() == 8)
        #expect(pacing.consumeTick() == 0)
    }

    @Test("Stays idle when the user is not scrolling")
    func noIntentNoEmission() {
        var pacing = ScrollPacing(maximumPointsPerTick: 15, fixedSpeedHoldTicks: 8)
        #expect(pacing.consumeTick() == 0)
    }

    @Test("A fast flick produces exactly eight fixed-speed ticks")
    func fastFlickUsesFixedSpeedWindow() {
        var pacing = ScrollPacing(maximumPointsPerTick: 15, fixedSpeedHoldTicks: 8)
        pacing.ingest(points: 500)
        for _ in 0..<8 {
            #expect(pacing.consumeTick() == 15)
        }
        #expect(pacing.consumeTick() == 0)
    }

    @Test("Combined intent reaching the limit enters fixed-speed mode")
    func combinedIntentUsesFixedSpeedWindow() {
        var pacing = ScrollPacing(maximumPointsPerTick: 15, fixedSpeedHoldTicks: 8)
        pacing.ingest(points: 9)
        pacing.ingest(points: 11)
        for _ in 0..<8 {
            #expect(pacing.consumeTick() == 15)
        }
        #expect(pacing.consumeTick() == 0)
    }

    @Test("Sparse fast wheel events never create a zero-speed gap")
    func sparseFastEventsStayContinuous() {
        var pacing = ScrollPacing(maximumPointsPerTick: 15, fixedSpeedHoldTicks: 8)
        for tick in 0..<18 {
            if tick == 0 || tick == 5 || tick == 10 {
                pacing.ingest(points: 500)
            }
            #expect(pacing.consumeTick() == 15)
        }
        #expect(pacing.consumeTick() == 0)
    }

    @Test("Caps sustained fast input at the configured stable speed")
    func sustainedSpeedCeiling() {
        var pacing = ScrollPacing()
        var emittedPoints = 0
        for _ in 0..<Int(ScrollingCaptureRate.framesPerSecond) {
            pacing.ingest(points: 500)
            emittedPoints += pacing.consumeTick()
        }
        #expect(emittedPoints == ScrollingCaptureRate.maximumPointsPerSecond)
    }

    @Test("Ignores upward scrolling")
    func upwardIgnored() {
        var pacing = ScrollPacing(maximumPointsPerTick: 15, fixedSpeedHoldTicks: 8)
        pacing.ingest(points: -20)
        #expect(pacing.consumeTick() == 0)
    }
}

@Suite("BottomDetectionTests")
struct BottomDetectionTests {
    @Test("Reports the bottom only after duplicates persist past the threshold")
    func sustainedDuplicates() {
        var detection = BottomDetection(threshold: 2.0)
        let start = Date()
        #expect(detection.recordFrame(isDuplicate: true, at: start) == false)
        #expect(detection.recordFrame(isDuplicate: true, at: start.addingTimeInterval(1.2)) == false)
        #expect(detection.recordFrame(isDuplicate: true, at: start.addingTimeInterval(2.1)) == true)
    }

    @Test("Any visible change rearms the timer")
    func changeRearms() {
        var detection = BottomDetection(threshold: 2.0)
        let start = Date()
        #expect(detection.recordFrame(isDuplicate: true, at: start) == false)
        #expect(detection.recordFrame(isDuplicate: false, at: start.addingTimeInterval(1.9)) == false)
        #expect(detection.recordFrame(isDuplicate: true, at: start.addingTimeInterval(3.0)) == false)
        #expect(detection.recordFrame(isDuplicate: true, at: start.addingTimeInterval(4.0)) == true)
    }
}
