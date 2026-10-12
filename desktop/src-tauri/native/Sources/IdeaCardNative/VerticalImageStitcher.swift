import CoreGraphics
import Foundation
import ImageIO
import UniformTypeIdentifiers

enum StitchAppendOutcome: Equatable {
    case initial
    case appended
    case duplicate
    case reverse
    case noOverlap
    case limitReached
}

enum VerticalImageStitcherError: Error {
    case invalidImage
    case noFrames
    case renderFailed
    case unreliableSequence
}

struct VerticalFrameMatch: Equatable {
    let shift: Int
    let score: Double
    let confidence: Double
    let informativeBands: Int
    let fixedBottomHeight: Int
}

enum VerticalFrameRelationship: Equatable {
    case duplicate
    case forward(VerticalFrameMatch)
    case reverse
    case unreliable
}

final class VerticalImageStitcher {
    private let minimumInformativeBands = 3
    private let maximumMatchScore = 3.5
    private let minimumConfidence = 0.75
    private let maximumHeight: Int
    private let forcedFixedBottomHeight: Int?
    private var segments: [CGImage] = []
    private var previousFrame: CGImage?
    private var fixedBottomHeight = 0
    private var fixedBottomSegment: CGImage?
    private var didEvaluateFixedBottom = false
    private(set) var totalHeight = 0

    init(maximumHeight: Int = 30_000, fixedBottomHeight: Int? = nil) {
        self.maximumHeight = maximumHeight
        forcedFixedBottomHeight = fixedBottomHeight
        self.fixedBottomHeight = fixedBottomHeight ?? 0
    }

    func framesAreEquivalent(_ left: CGImage, _ right: CGImage) throws -> Bool {
        try relationship(from: left, to: right) == .duplicate
    }

    func relationship(
        from previousImage: CGImage,
        to currentImage: CGImage
    ) throws -> VerticalFrameRelationship {
        try relationship(
            from: previousImage,
            to: currentImage,
            includeDetailedValidation: true
        )
    }

    func fastRelationship(
        from previousImage: CGImage,
        to currentImage: CGImage
    ) throws -> VerticalFrameRelationship {
        try relationship(
            from: previousImage,
            to: currentImage,
            includeDetailedValidation: false
        )
    }

    private func relationship(
        from previousImage: CGImage,
        to currentImage: CGImage,
        includeDetailedValidation: Bool
    ) throws -> VerticalFrameRelationship {
        guard
            previousImage.width == currentImage.width,
            previousImage.height == currentImage.height,
            previousImage.width > 0,
            previousImage.height > 0
        else {
            throw VerticalImageStitcherError.invalidImage
        }

        let previous = try fingerprint(
            previousImage,
            includeDetailedValidation: includeDetailedValidation
        )
        let current = try fingerprint(
            currentImage,
            includeDetailedValidation: includeDetailedValidation
        )
        let unchanged = rawDifference(previous.coarseGray, current.coarseGray)
        let zero = comparisonScore(
            previous,
            current,
            shift: 0,
            direction: .forward,
            sampleRows: includeDetailedValidation ? 180 : 30
        )
        if unchanged < 0.35
            || (
                zero.informativeBands >= minimumInformativeBands
                    && zero.score < 1.8
            ) {
            return .duplicate
        }

        let maximumShift = min(
            max(4, Int(Double(current.height) * 0.68)),
            max(4, current.height - 32)
        )
        guard maximumShift >= 4 else { return .unreliable }
        let forward = bestMatch(
            previous,
            current,
            maximumShift: maximumShift,
            direction: .forward,
            fast: !includeDetailedValidation
        )
        let reverse = includeDetailedValidation
            ? bestMatch(
                previous,
                current,
                maximumShift: maximumShift,
                direction: .reverse,
                fast: false
            )
            : nil
        let validForward = forward.flatMap {
            validated($0, requireConfidence: includeDetailedValidation)
        }
        let validReverse = reverse.flatMap {
            validated($0, requireConfidence: includeDetailedValidation)
        }

        if let validReverse {
            guard let validForward else { return .reverse }
            if validReverse.score + 0.75 < validForward.score {
                return .reverse
            }
            if abs(validReverse.score - validForward.score) < 0.45 {
                return .unreliable
            }
        }
        if let validForward {
            let fixedBottomHeight: Int
            if
                includeDetailedValidation,
                let previousValidation = previous.validationGray,
                let currentValidation = current.validationGray
            {
                fixedBottomHeight = detectedFixedBottomHeight(
                    previousValidation,
                    currentValidation
                )
            } else {
                fixedBottomHeight = 0
            }
            return .forward(VerticalFrameMatch(
                shift: validForward.shift,
                score: validForward.score,
                confidence: validForward.confidence,
                informativeBands: validForward.informativeBands,
                fixedBottomHeight: fixedBottomHeight
            ))
        }
        return .unreliable
    }

    func append(_ image: CGImage) throws -> StitchAppendOutcome {
        guard image.width > 0, image.height > 0 else {
            throw VerticalImageStitcherError.invalidImage
        }
        guard let previousFrame else {
            segments = [image]
            self.previousFrame = image
            totalHeight = image.height
            return totalHeight >= maximumHeight ? .limitReached : .initial
        }
        guard previousFrame.width == image.width, previousFrame.height == image.height else {
            throw VerticalImageStitcherError.invalidImage
        }

        switch try relationship(from: previousFrame, to: image) {
        case .duplicate:
            self.previousFrame = image
            return .duplicate
        case .reverse:
            return .reverse
        case .unreliable:
            return .noOverlap
        case let .forward(match):
            guard try prepareFixedBottom(
                previousFrame: previousFrame,
                currentFrame: image,
                detectedHeight: match.fixedBottomHeight
            ) else {
                return .noOverlap
            }
            let available = maximumHeight - totalHeight
            guard available > 0 else { return .limitReached }
            let appendedHeight = min(match.shift, available)
            let cropY = image.height - fixedBottomHeight - appendedHeight
            guard
                appendedHeight > 0,
                cropY >= 0,
                let newSegment = image.cropping(to: CGRect(
                    x: 0,
                    y: cropY,
                    width: image.width,
                    height: appendedHeight
                ))
            else {
                throw VerticalImageStitcherError.invalidImage
            }
            segments.append(newSegment)
            totalHeight += appendedHeight
            self.previousFrame = image
            return appendedHeight < match.shift || totalHeight >= maximumHeight
                ? .limitReached
                : .appended
        }
    }

    func writePNG(to outputURL: URL) throws {
        guard let first = segments.first, !segments.isEmpty else {
            throw VerticalImageStitcherError.noFrames
        }
        guard
            let context = CGContext(
                data: nil,
                width: first.width,
                height: totalHeight,
                bitsPerComponent: 8,
                bytesPerRow: first.width * 4,
                space: CGColorSpaceCreateDeviceRGB(),
                bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
            )
        else {
            throw VerticalImageStitcherError.renderFailed
        }

        var top = totalHeight
        for segment in segments {
            top -= segment.height
            context.draw(
                segment,
                in: CGRect(x: 0, y: top, width: segment.width, height: segment.height)
            )
        }
        if let fixedBottomSegment {
            top -= fixedBottomSegment.height
            context.draw(
                fixedBottomSegment,
                in: CGRect(
                    x: 0,
                    y: top,
                    width: fixedBottomSegment.width,
                    height: fixedBottomSegment.height
                )
            )
        }
        guard top == 0 else {
            throw VerticalImageStitcherError.renderFailed
        }
        guard
            let image = context.makeImage(),
            let destination = CGImageDestinationCreateWithURL(
                outputURL as CFURL,
                UTType.png.identifier as CFString,
                1,
                nil
            )
        else {
            throw VerticalImageStitcherError.renderFailed
        }
        CGImageDestinationAddImage(destination, image, nil)
        guard CGImageDestinationFinalize(destination) else {
            throw VerticalImageStitcherError.renderFailed
        }
    }

    func writeValidatedPNG(frames: [CGImage], to outputURL: URL) throws {
        guard frames.count >= 2 else {
            throw VerticalImageStitcherError.unreliableSequence
        }
        let probe = VerticalImageStitcher(maximumHeight: maximumHeight)
        var detectedBottomHeights: [Int] = []
        for index in 1..<frames.count {
            guard case let .forward(match) = try probe.relationship(
                from: frames[index - 1],
                to: frames[index]
            ) else {
                throw VerticalImageStitcherError.unreliableSequence
            }
            if match.fixedBottomHeight > 0 {
                detectedBottomHeights.append(match.fixedBottomHeight)
            }
        }
        let validator = VerticalImageStitcher(
            maximumHeight: maximumHeight,
            fixedBottomHeight: resolvedFixedBottomHeight(
                detectedBottomHeights,
                edgeCount: frames.count - 1
            )
        )
        for (index, frame) in frames.enumerated() {
            switch try validator.append(frame) {
            case .initial where index == 0:
                break
            case .appended where index > 0:
                break
            case .limitReached where index > 0:
                break
            default:
                throw VerticalImageStitcherError.unreliableSequence
            }
        }
        guard
            let first = frames.first,
            validator.totalHeight > first.height
        else {
            throw VerticalImageStitcherError.unreliableSequence
        }
        try validator.writePNG(to: outputURL)
        try validator.validateWrittenPNG(at: outputURL)
    }

    private enum MatchDirection {
        case forward
        case reverse
    }

    private struct MatchCandidate {
        let shift: Int
        let score: Double
        let confidence: Double
        let informativeBands: Int
    }

    private struct GrayImage {
        let width: Int
        let height: Int
        let pixels: [UInt8]
    }

    private struct FrameFingerprint {
        let coarseGray: GrayImage
        let validationGray: GrayImage?
        let bins: Int
        let rowFeatures: [Float]

        var width: Int { coarseGray.width }
        var height: Int { coarseGray.height }
    }

    private struct BandScore {
        let score: Double
        let informativeBands: Int
    }

    private func fingerprint(
        _ image: CGImage,
        includeDetailedValidation: Bool
    ) throws -> FrameFingerprint {
        let coarseGray = try grayscale(image, width: min(128, image.width))
        let validationGray = includeDetailedValidation
            ? try grayscale(image, width: min(1_024, image.width))
            : nil
        let width = coarseGray.width
        let height = coarseGray.height
        let pixels = coarseGray.pixels
        let bins = min(10, max(5, width / 12))
        var rowFeatures = [Float](repeating: 0, count: height * bins)
        guard width >= 3, height >= 3 else {
            return FrameFingerprint(
                coarseGray: coarseGray,
                validationGray: validationGray,
                bins: bins,
                rowFeatures: rowFeatures
            )
        }
        let edgeInset = max(2, width / 50)
        let startX = min(edgeInset, width - 2)
        let endX = max(startX + 1, width - edgeInset)
        var counts = [Int](repeating: 0, count: bins)
        for x in startX..<endX {
            let bin = min(bins - 1, x * bins / width)
            counts[bin] += 1
        }
        for y in 1..<(height - 1) {
            for x in startX..<endX {
                let center = Int(pixels[y * width + x])
                let horizontal = abs(
                    Int(pixels[y * width + x + 1])
                        - Int(pixels[y * width + x - 1])
                )
                let vertical = abs(
                    Int(pixels[(y + 1) * width + x])
                        - Int(pixels[(y - 1) * width + x])
                )
                let darkness = max(0, 242 - center) / 3
                let value = min(255, horizontal + vertical + darkness)
                let bin = min(bins - 1, x * bins / width)
                rowFeatures[y * bins + bin] += Float(value)
            }
            for bin in 0..<bins where counts[bin] > 0 {
                rowFeatures[y * bins + bin] /= Float(counts[bin])
            }
        }
        return FrameFingerprint(
            coarseGray: coarseGray,
            validationGray: validationGray,
            bins: bins,
            rowFeatures: rowFeatures
        )
    }

    private func grayscale(_ image: CGImage, width: Int) throws -> GrayImage {
        let height = image.height
        var pixels = [UInt8](repeating: 0, count: width * height)
        guard
            let context = CGContext(
                data: &pixels,
                width: width,
                height: height,
                bitsPerComponent: 8,
                bytesPerRow: width,
                space: CGColorSpaceCreateDeviceGray(),
                bitmapInfo: CGImageAlphaInfo.none.rawValue
            )
        else {
            throw VerticalImageStitcherError.invalidImage
        }
        context.interpolationQuality = .low
        context.draw(image, in: CGRect(x: 0, y: 0, width: width, height: height))
        return GrayImage(width: width, height: height, pixels: pixels)
    }

    private func bestMatch(
        _ previous: FrameFingerprint,
        _ current: FrameFingerprint,
        maximumShift: Int,
        direction: MatchDirection,
        fast: Bool
    ) -> MatchCandidate? {
        let coarseStep = max(2, previous.height / 240)
        let sampleRows = fast ? 30 : 180
        var coarseMatches: [MatchCandidate] = []
        var shift = 4
        while shift <= maximumShift {
            let result = comparisonScore(
                previous,
                current,
                shift: shift,
                direction: direction,
                sampleRows: sampleRows
            )
            if result.informativeBands >= minimumInformativeBands {
                coarseMatches.append(MatchCandidate(
                    shift: shift,
                    score: result.score,
                    confidence: 0,
                    informativeBands: result.informativeBands
                ))
            }
            shift += coarseStep
        }
        guard !coarseMatches.isEmpty else { return nil }

        let seeds = coarseMatches.sorted { $0.score < $1.score }.prefix(10)
        var refinedByShift: [Int: MatchCandidate] = [:]
        for seed in seeds {
            let lower = max(1, seed.shift - coarseStep)
            let upper = min(maximumShift, seed.shift + coarseStep)
            for candidate in lower...upper {
                let result = comparisonScore(
                    previous,
                    current,
                    shift: candidate,
                    direction: direction,
                    sampleRows: sampleRows
                )
                guard result.informativeBands >= minimumInformativeBands else { continue }
                let match = MatchCandidate(
                    shift: candidate,
                    score: result.score,
                    confidence: 0,
                    informativeBands: result.informativeBands
                )
                if refinedByShift[candidate] == nil
                    || result.score < refinedByShift[candidate]!.score {
                    refinedByShift[candidate] = match
                }
            }
        }
        let refined = refinedByShift.values
            .sorted { $0.score < $1.score }
            .prefix(12)
        let verified: [MatchCandidate]
        if
            let previousValidation = previous.validationGray,
            let currentValidation = current.validationGray
        {
            verified = refined.compactMap { candidate -> MatchCandidate? in
                let detailed = detailedComparisonScore(
                    previousValidation,
                    currentValidation,
                    shift: candidate.shift,
                    direction: direction,
                    sampleRows: sampleRows
                )
                guard detailed.informativeBands >= minimumInformativeBands else {
                    return nil
                }
                return MatchCandidate(
                    shift: candidate.shift,
                    score: max(candidate.score, detailed.score * 0.35),
                    confidence: 0,
                    informativeBands: min(
                        candidate.informativeBands,
                        detailed.informativeBands
                    )
                )
            }
        } else {
            verified = Array(refined)
        }
        let sorted = verified.sorted { $0.score < $1.score }
        guard let best = sorted.first else { return nil }
        let separation = max(4, coarseStep * 2)
        let second = sorted.first {
            abs($0.shift - best.shift) >= separation
        }
        let confidence = (second?.score ?? maximumMatchScore + minimumConfidence)
            - best.score
        return MatchCandidate(
            shift: best.shift,
            score: best.score,
            confidence: confidence,
            informativeBands: best.informativeBands
        )
    }

    private func comparisonScore(
        _ previous: FrameFingerprint,
        _ current: FrameFingerprint,
        shift: Int,
        direction: MatchDirection,
        sampleRows: Int
    ) -> BandScore {
        let overlap = previous.height - shift
        guard overlap > 32 else {
            return BandScore(score: .greatestFiniteMagnitude, informativeBands: 0)
        }
        let start = max(2, min(previous.height / 12, overlap / 5))
        let end = overlap - max(2, min(previous.height / 16, overlap / 5))
        guard end - start >= 24 else {
            return BandScore(score: .greatestFiniteMagnitude, informativeBands: 0)
        }

        let bandCount = 5
        let rowStep = max(1, (end - start) / sampleRows)
        var differences = [Double](repeating: 0, count: bandCount)
        var counts = [Int](repeating: 0, count: bandCount)
        var y = start
        while y < end {
            let previousY: Int
            let currentY: Int
            switch direction {
            case .forward:
                previousY = y + shift
                currentY = y
            case .reverse:
                previousY = y
                currentY = y + shift
            }
            let band = min(
                bandCount - 1,
                (y - start) * bandCount / max(1, end - start)
            )
            for bin in 0..<previous.bins {
                let left = previous.rowFeatures[previousY * previous.bins + bin]
                let right = current.rowFeatures[currentY * current.bins + bin]
                guard max(left, right) >= 1.5 else { continue }
                differences[band] += Double(abs(left - right))
                counts[band] += 1
            }
            y += rowStep
        }
        var bandScores: [Double] = []
        for band in 0..<bandCount where counts[band] >= 8 {
            bandScores.append(differences[band] / Double(counts[band]))
        }
        guard !bandScores.isEmpty else {
            return BandScore(score: .greatestFiniteMagnitude, informativeBands: 0)
        }
        return BandScore(
            score: conservativeBandScore(bandScores),
            informativeBands: bandScores.count
        )
    }

    private func detailedComparisonScore(
        _ previous: GrayImage,
        _ current: GrayImage,
        shift: Int,
        direction: MatchDirection,
        sampleRows: Int = 180
    ) -> BandScore {
        let overlap = previous.height - shift
        guard overlap > 32 else {
            return BandScore(score: .greatestFiniteMagnitude, informativeBands: 0)
        }
        let start = max(2, min(previous.height / 12, overlap / 5))
        let end = overlap - max(2, min(previous.height / 16, overlap / 5))
        guard end - start >= 24 else {
            return BandScore(score: .greatestFiniteMagnitude, informativeBands: 0)
        }
        let bandCount = 5
        let rowStep = max(1, (end - start) / sampleRows)
        let columnStep = max(1, previous.width / 96)
        let edgeInset = max(2, previous.width / 50)
        let endX = max(edgeInset + 1, previous.width - edgeInset)
        var differences = [Double](repeating: 0, count: bandCount)
        var counts = [Int](repeating: 0, count: bandCount)
        var y = start
        while y < end {
            let previousY: Int
            let currentY: Int
            switch direction {
            case .forward:
                previousY = y + shift
                currentY = y
            case .reverse:
                previousY = y
                currentY = y + shift
            }
            let band = min(
                bandCount - 1,
                (y - start) * bandCount / max(1, end - start)
            )
            var x = edgeInset
            while x < endX {
                let left = Int(previous.pixels[previousY * previous.width + x])
                let right = Int(current.pixels[currentY * current.width + x])
                if min(left, right) < 246 {
                    differences[band] += Double(abs(left - right))
                    counts[band] += 1
                }
                x += columnStep
            }
            y += rowStep
        }
        var bandScores: [Double] = []
        for band in 0..<bandCount where counts[band] >= 8 {
            bandScores.append(differences[band] / Double(counts[band]))
        }
        guard !bandScores.isEmpty else {
            return BandScore(score: .greatestFiniteMagnitude, informativeBands: 0)
        }
        return BandScore(
            score: conservativeBandScore(bandScores),
            informativeBands: bandScores.count
        )
    }

    private func conservativeBandScore(_ scores: [Double]) -> Double {
        guard !scores.isEmpty else { return .greatestFiniteMagnitude }
        let sorted = scores.sorted()
        guard sorted.count >= 3 else { return sorted.last! }
        // A fixed header or one animated strip may legitimately disagree.
        // Two disagreeing regions indicate repeated content or a false vertical shift.
        return sorted[sorted.count - 2]
    }

    private func rawDifference(_ left: GrayImage, _ right: GrayImage) -> Double {
        guard
            left.width == right.width,
            left.height == right.height,
            !left.pixels.isEmpty
        else {
            return .greatestFiniteMagnitude
        }
        let rowStep = max(1, left.height / 160)
        let columnStep = max(1, left.width / 80)
        var difference = 0
        var count = 0
        var y = 0
        while y < left.height {
            var x = 0
            while x < left.width {
                let index = y * left.width + x
                difference += abs(Int(left.pixels[index]) - Int(right.pixels[index]))
                count += 1
                x += columnStep
            }
            y += rowStep
        }
        return count == 0 ? .greatestFiniteMagnitude : Double(difference) / Double(count)
    }

    private func validated(
        _ candidate: MatchCandidate,
        requireConfidence: Bool
    ) -> VerticalFrameMatch? {
        guard
            candidate.score <= maximumMatchScore,
            (!requireConfidence || candidate.confidence >= minimumConfidence),
            candidate.informativeBands >= minimumInformativeBands
        else {
            return nil
        }
        return VerticalFrameMatch(
            shift: candidate.shift,
            score: candidate.score,
            confidence: candidate.confidence,
            informativeBands: candidate.informativeBands,
            fixedBottomHeight: 0
        )
    }

    private func detectedFixedBottomHeight(
        _ previous: GrayImage,
        _ current: GrayImage
    ) -> Int {
        guard previous.width == current.width, previous.height == current.height else {
            return 0
        }
        let maximumRows = min(240, previous.height / 4)
        let columnStep = max(1, previous.width / 320)
        let edgeInset = max(2, previous.width / 50)
        let endX = max(edgeInset + 1, previous.width - edgeInset)
        var detectedRows = 0
        var informativeRows = 0
        var trailingMismatchRows = 0

        for y in stride(
            from: previous.height - 1,
            through: max(0, previous.height - maximumRows),
            by: -1
        ) {
            var difference = 0
            var samples = 0
            var informativePixels = 0
            var x = edgeInset
            while x < endX {
                let left = Int(previous.pixels[y * previous.width + x])
                let right = Int(current.pixels[y * current.width + x])
                difference += abs(left - right)
                if min(left, right) < 238 {
                    informativePixels += 1
                }
                samples += 1
                x += columnStep
            }
            let average = samples == 0
                ? Double.greatestFiniteMagnitude
                : Double(difference) / Double(samples)
            if average <= 2.4 {
                detectedRows += 1
                trailingMismatchRows = 0
                if informativePixels >= max(3, samples / 50) {
                    informativeRows += 1
                }
            } else {
                trailingMismatchRows += 1
                if trailingMismatchRows > 2 {
                    break
                }
                detectedRows += 1
            }
        }

        let height = detectedRows - trailingMismatchRows
        guard height >= 4, informativeRows >= 2 else { return 0 }
        return height
    }

    private func prepareFixedBottom(
        previousFrame: CGImage,
        currentFrame: CGImage,
        detectedHeight: Int
    ) throws -> Bool {
        if !didEvaluateFixedBottom {
            didEvaluateFixedBottom = true
            let height = forcedFixedBottomHeight ?? detectedHeight
            guard height > 0 else { return true }
            guard detectedHeight >= height - 2 else { return false }
            guard
                height < previousFrame.height / 3,
                let first = segments.first,
                let trimmed = first.cropping(to: CGRect(
                    x: 0,
                    y: 0,
                    width: first.width,
                    height: first.height - height
                )),
                let footer = currentFrame.cropping(to: CGRect(
                    x: 0,
                    y: currentFrame.height - height,
                    width: currentFrame.width,
                    height: height
                ))
            else {
                throw VerticalImageStitcherError.invalidImage
            }
            segments[0] = trimmed
            fixedBottomHeight = height
            fixedBottomSegment = footer
            return true
        }

        guard fixedBottomHeight > 0 else {
            return detectedHeight == 0
        }
        if forcedFixedBottomHeight != nil {
            guard detectedHeight >= fixedBottomHeight - 2 else { return false }
        } else {
            guard abs(detectedHeight - fixedBottomHeight) <= 2 else {
                return false
            }
        }
        guard let footer = currentFrame.cropping(to: CGRect(
            x: 0,
            y: currentFrame.height - fixedBottomHeight,
            width: currentFrame.width,
            height: fixedBottomHeight
        )) else {
            throw VerticalImageStitcherError.invalidImage
        }
        fixedBottomSegment = footer
        return true
    }

    private func resolvedFixedBottomHeight(
        _ candidates: [Int],
        edgeCount: Int
    ) -> Int? {
        guard
            !candidates.isEmpty,
            edgeCount > 0,
            candidates.count * 2 >= edgeCount
        else {
            return nil
        }
        let sorted = candidates.sorted()
        return sorted[(sorted.count - 1) / 5]
    }

    private func validateWrittenPNG(at url: URL) throws {
        guard
            let source = CGImageSourceCreateWithURL(url as CFURL, nil),
            CGImageSourceGetCount(source) == 1,
            let properties = CGImageSourceCopyPropertiesAtIndex(source, 0, nil)
                as? [CFString: Any],
            properties[kCGImagePropertyPixelWidth] as? Int == segments.first?.width,
            properties[kCGImagePropertyPixelHeight] as? Int == totalHeight,
            CGImageSourceCreateImageAtIndex(source, 0, nil) != nil
        else {
            throw VerticalImageStitcherError.renderFailed
        }
    }
}
