import AppKit
import CoreGraphics
import CoreText
import Foundation
import ImageIO
import Testing
@testable import IdeaCardNative

@Suite("VerticalImageStitcher")
struct VerticalImageStitcherTests {
    @Test("Appends only the new bottom content from an overlapping frame")
    func appendsNewBottomContent() throws {
        let stitcher = VerticalImageStitcher()

        #expect(try stitcher.append(makeFrame(offset: 0)) == .initial)
        #expect(try stitcher.append(makeFrame(offset: 40)) == .appended)
        #expect(stitcher.totalHeight == 160)
    }

    @Test("Ignores duplicate and reverse frames")
    func ignoresDuplicateAndReverseFrames() throws {
        let stitcher = VerticalImageStitcher()
        let first = makeFrame(offset: 0)

        #expect(try stitcher.append(first) == .initial)
        #expect(try stitcher.append(first) == .duplicate)
        #expect(try stitcher.append(makeFrame(offset: 40)) == .appended)
        #expect(try stitcher.append(makeFrame(offset: 10)) == .reverse)
        #expect(stitcher.totalHeight == 160)
    }

    @Test("Distinguishes a settled frame from content that is still moving")
    func detectsFrameStability() throws {
        let stitcher = VerticalImageStitcher()
        let settled = makeFrame(offset: 40)

        #expect(try stitcher.framesAreEquivalent(settled, settled))
        #expect(try !stitcher.framesAreEquivalent(settled, makeFrame(offset: 43)))
    }

    @Test("Rejects ambiguous shifts when content repeats")
    func repeatedContentIsRejected() throws {
        let stitcher = VerticalImageStitcher()

        #expect(try stitcher.append(makeFrame(offset: 0, period: 40)) == .initial)
        #expect(try stitcher.append(makeFrame(offset: 20, period: 40)) == .noOverlap)
        #expect(stitcher.totalHeight == 120)
    }

    @Test("Rejects frames without a trustworthy overlap")
    func rejectsMissingOverlap() throws {
        let stitcher = VerticalImageStitcher()
        let first = makeFrame(offset: 0)
        let unrelated = makeFrame(offset: 700)

        #expect(try stitcher.append(first) == .initial)
        #expect(try stitcher.append(unrelated) == .noOverlap)
        #expect(stitcher.totalHeight == 120)
    }

    @Test("Stops exactly at the configured maximum height")
    func enforcesMaximumHeight() throws {
        let stitcher = VerticalImageStitcher(maximumHeight: 150)

        #expect(try stitcher.append(makeFrame(offset: 0)) == .initial)
        #expect(try stitcher.append(makeFrame(offset: 40)) == .limitReached)
        #expect(stitcher.totalHeight == 150)
    }

    @Test("Writes one PNG with the stitched dimensions")
    func writesStitchedPNG() throws {
        let stitcher = VerticalImageStitcher()
        _ = try stitcher.append(makeFrame(offset: 0))
        _ = try stitcher.append(makeFrame(offset: 40))
        let output = FileManager.default.temporaryDirectory
            .appending(path: "ideacard-stitch-\(UUID().uuidString).png")
        defer { try? FileManager.default.removeItem(at: output) }

        try stitcher.writePNG(to: output)

        let source = try #require(CGImageSourceCreateWithURL(output as CFURL, nil))
        let properties = try #require(
            CGImageSourceCopyPropertiesAtIndex(source, 0, nil) as? [CFString: Any]
        )
        #expect(properties[kCGImagePropertyPixelWidth] as? Int == 80)
        #expect(properties[kCGImagePropertyPixelHeight] as? Int == 160)
        let image = try #require(CGImageSourceCreateImageAtIndex(source, 0, nil))
        #expect(pixel(in: image, x: 0, y: 0) == expectedPixel(contentY: 0, x: 0))
        #expect(pixel(in: image, x: 0, y: 159) == expectedPixel(contentY: 159, x: 0))
    }

    @Test("Stitches deterministic black text on a white page pixel-perfectly")
    func stitchesRenderedTextPage() throws {
        let page = makeTextPage(width: 520, height: 1_000)
        let frames = stride(from: 0, through: 700, by: 100).compactMap {
            textViewport(page: page, offset: $0, height: 300)
        }
        let output = temporaryPNG()
        defer { try? FileManager.default.removeItem(at: output) }

        try VerticalImageStitcher().writeValidatedPNG(frames: frames, to: output)

        let stitched = try loadImage(at: output)
        #expect(stitched.width == page.width)
        #expect(stitched.height == page.height)
        #expect(imageBytes(stitched) == imageBytes(page))
    }

    @Test("Stitches sparse Retina-sized text anchors without cumulative seam drift")
    func stitchesSparseRetinaTextAnchors() throws {
        let page = makeTextPage(width: 1_360, height: 3_500)
        let offsets = [0, 430, 860, 1_290, 1_720, 2_150, 2_240]
        let frames = offsets.compactMap {
            textViewport(page: page, offset: $0, height: 1_260)
        }
        let output = temporaryPNG()
        defer { try? FileManager.default.removeItem(at: output) }

        try VerticalImageStitcher().writeValidatedPNG(frames: frames, to: output)

        let stitched = try loadImage(at: output)
        #expect(stitched.width == page.width)
        #expect(stitched.height == page.height)
        #expect(imageBytes(stitched) == imageBytes(page))
    }

    @Test("Stitches unique markers beside repeated text and keeps a fixed bottom strip once")
    func stitchesRepeatedSectionsWithFixedBottom() throws {
        let page = makeRepeatedSectionPage(width: 900, height: 2_000)
        let offsets = [0, 250, 500, 750, 1_000, 1_250, 1_300]
        let frames = offsets.compactMap {
            textViewport(
                page: page,
                offset: $0,
                height: 700,
                fixedHeaderHeight: 52,
                fixedFooterHeight: 16
            )
        }
        let output = temporaryPNG()
        defer { try? FileManager.default.removeItem(at: output) }

        try VerticalImageStitcher().writeValidatedPNG(frames: frames, to: output)

        let stitched = try loadImage(at: output)
        #expect(stitched.width == page.width)
        #expect(stitched.height == page.height)
        #expect(blueMarkerPixelCount(in: stitched) == (page.width - 40) * 5)
    }

    @Test("Ignores a fixed header while matching scrolling text")
    func handlesFixedHeader() throws {
        let page = makeTextPage(width: 520, height: 900)
        let first = try #require(textViewport(
            page: page,
            offset: 0,
            height: 300,
            fixedHeaderHeight: 44
        ))
        let second = try #require(textViewport(
            page: page,
            offset: 100,
            height: 300,
            fixedHeaderHeight: 44
        ))

        let relationship = try VerticalImageStitcher().relationship(
            from: first,
            to: second
        )

        guard case let .forward(match) = relationship else {
            Issue.record("Expected a forward match, got \(relationship)")
            return
        }
        #expect(abs(match.shift - 100) <= 1)
    }

    @Test("Keeps a fixed header only once in the rendered long image")
    func rendersFixedHeaderOnce() throws {
        let page = makeTextPage(width: 520, height: 900)
        let frames = stride(from: 0, through: 600, by: 100).compactMap {
            textViewport(
                page: page,
                offset: $0,
                height: 300,
                fixedHeaderHeight: 44
            )
        }
        let output = temporaryPNG()
        defer { try? FileManager.default.removeItem(at: output) }

        try VerticalImageStitcher().writeValidatedPNG(frames: frames, to: output)

        let stitched = try loadImage(at: output)
        #expect(stitched.height == page.height)
        #expect(redMarkerPixelCount(in: stitched) == 210 * 12)
    }

    @Test("Rejects horizontal content drift instead of creating misaligned text")
    func rejectsHorizontalDrift() throws {
        let page = makeTextPage(width: 520, height: 900)
        let first = try #require(textViewport(page: page, offset: 0, height: 300))
        let second = try #require(textViewport(page: page, offset: 100, height: 300))
        let shifted = shiftContent(second, horizontallyBy: 2)

        let relationship = try VerticalImageStitcher().relationship(
            from: first,
            to: shifted
        )

        #expect(relationship == .unreliable)
    }

    @Test("Uses continuous intermediate frames to bridge a fast scroll")
    func bridgesFastScrollWithIntermediateFrames() throws {
        let page = makeTextPage(width: 520, height: 900)
        let first = try #require(textViewport(page: page, offset: 0, height: 300))
        let middle = try #require(textViewport(page: page, offset: 160, height: 300))
        let last = try #require(textViewport(page: page, offset: 320, height: 300))
        let stitcher = VerticalImageStitcher()

        #expect(try stitcher.relationship(from: first, to: last) == .unreliable)
        guard
            case .forward = try stitcher.relationship(from: first, to: middle),
            case .forward = try stitcher.relationship(from: middle, to: last)
        else {
            Issue.record("Expected the intermediate frame to preserve continuity")
            return
        }
    }

    @Test("Tracks continuous Retina frames with coarse features before offline validation")
    func tracksContinuousFramesWithoutDetailedValidation() throws {
        let page = makeTextPage(width: 1_360, height: 3_500)
        let first = try #require(textViewport(page: page, offset: 0, height: 1_160))
        let stitcher = VerticalImageStitcher()

        for offset in stride(from: 80, through: 720, by: 80) {
            let frame = try #require(textViewport(
                page: page,
                offset: offset,
                height: 1_160
            ))
            guard case let .forward(match) = try stitcher.fastRelationship(
                from: first,
                to: frame
            ) else {
                Issue.record("Expected a forward coarse match at offset \(offset)")
                continue
            }
            #expect(abs(match.shift - offset) <= 1)
        }
    }

    @Test("Still rejects a coarse candidate when offline detail validation finds drift")
    func offlineValidationRejectsCoarseHorizontalDrift() throws {
        let page = makeTextPage(width: 520, height: 900)
        let first = try #require(textViewport(page: page, offset: 0, height: 300))
        let second = try #require(textViewport(page: page, offset: 100, height: 300))
        let shifted = shiftContent(second, horizontallyBy: 2)
        let output = temporaryPNG()
        defer { try? FileManager.default.removeItem(at: output) }

        #expect(throws: VerticalImageStitcherError.self) {
            try VerticalImageStitcher().writeValidatedPNG(
                frames: [first, shifted],
                to: output
            )
        }
        #expect(!FileManager.default.fileExists(atPath: output.path))
    }

    @Test("Refuses to save a one-frame rolling capture")
    func rejectsOneFrameCapture() throws {
        let page = makeTextPage(width: 520, height: 600)
        let frame = try #require(textViewport(page: page, offset: 0, height: 300))
        let output = temporaryPNG()
        defer { try? FileManager.default.removeItem(at: output) }

        #expect(throws: VerticalImageStitcherError.self) {
            try VerticalImageStitcher().writeValidatedPNG(frames: [frame], to: output)
        }
        #expect(!FileManager.default.fileExists(atPath: output.path))
    }

    private func makeFrame(
        offset: Int,
        width: Int = 80,
        height: Int = 120,
        period: Int? = nil
    ) -> CGImage {
        var pixels = [UInt8](repeating: 0, count: width * height * 4)
        for y in 0..<height {
            let contentY = y + offset
            let patternY = period.map { contentY % $0 } ?? contentY
            for x in 0..<width {
                let index = (y * width + x) * 4
                pixels[index] = UInt8((patternY * 31 + x * 17 + patternY / 7) % 251)
                pixels[index + 1] = UInt8((patternY * 13 + x * 29 + x / 5) % 253)
                pixels[index + 2] = UInt8((patternY * 47 + x * 7 + patternY / 11) % 247)
                pixels[index + 3] = 255
            }
        }
        let data = Data(pixels) as CFData
        let provider = CGDataProvider(data: data)!
        return CGImage(
            width: width,
            height: height,
            bitsPerComponent: 8,
            bitsPerPixel: 32,
            bytesPerRow: width * 4,
            space: CGColorSpaceCreateDeviceRGB(),
            bitmapInfo: CGBitmapInfo(
                rawValue: CGImageAlphaInfo.premultipliedLast.rawValue
            ),
            provider: provider,
            decode: nil,
            shouldInterpolate: false,
            intent: .defaultIntent
        )!
    }

    private func makeTextPage(width: Int, height: Int) -> CGImage {
        let colorSpace = CGColorSpaceCreateDeviceRGB()
        let context = CGContext(
            data: nil,
            width: width,
            height: height,
            bitsPerComponent: 8,
            bytesPerRow: width * 4,
            space: colorSpace,
            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
        )!
        context.setFillColor(CGColor(gray: 1, alpha: 1))
        context.fill(CGRect(x: 0, y: 0, width: width, height: height))
        let font = CTFontCreateWithName("PingFangSC-Regular" as CFString, 17, nil)
        for row in 0..<max(24, height / 39 - 1) {
            let y = CGFloat(height - 34 - row * 39)
            guard y >= 8 else { break }
            let text = String(
                format: "第 %02d 行  稳定滚动截图需要保持文字连续  section-%02d",
                row + 1,
                (row * 7) % 19
            )
            let line = CTLineCreateWithAttributedString(NSAttributedString(
                string: text,
                attributes: [
                    .font: font,
                    .foregroundColor: NSColor(
                        calibratedWhite: row.isMultiple(of: 3) ? 0.08 : 0.2,
                        alpha: 1
                    ),
                ]
            ))
            context.textPosition = CGPoint(x: 22 + CGFloat((row % 4) * 7), y: y)
            CTLineDraw(line, context)
            if row.isMultiple(of: 5) {
                context.setFillColor(CGColor(gray: 0.82, alpha: 1))
                context.fill(CGRect(
                    x: 18,
                    y: y - 10,
                    width: CGFloat(width - 36),
                    height: 1
                ))
            }
        }
        return context.makeImage()!
    }

    private func makeRepeatedSectionPage(width: Int, height: Int) -> CGImage {
        let context = CGContext(
            data: nil,
            width: width,
            height: height,
            bitsPerComponent: 8,
            bytesPerRow: width * 4,
            space: CGColorSpaceCreateDeviceRGB(),
            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
        )!
        context.setFillColor(CGColor(gray: 1, alpha: 1))
        context.fill(CGRect(x: 0, y: 0, width: width, height: height))
        let headingFont = CTFontCreateWithName("PingFangSC-Semibold" as CFString, 18, nil)
        let bodyFont = CTFontCreateWithName("PingFangSC-Regular" as CFString, 16, nil)
        let body = "每一段都是相同正文，用来验证重复内容不能覆盖唯一标记。"
        for section in 0..<20 {
            let top = height - 48 - section * 98
            let marker = String(
                format: "第 %02d 段    MARK-%02d-%03d",
                section + 1,
                section + 1,
                (section * 37) % 999
            )
            drawLine(
                marker,
                font: headingFont,
                color: NSColor(calibratedRed: 0.1, green: 0.34, blue: 0.2, alpha: 1),
                at: CGPoint(x: 24, y: top),
                in: context
            )
            drawLine(
                body,
                font: bodyFont,
                color: NSColor(calibratedWhite: 0.14, alpha: 1),
                at: CGPoint(x: 24, y: top - 34),
                in: context
            )
            context.setFillColor(CGColor(gray: 0.88, alpha: 1))
            context.fill(CGRect(x: 20, y: top - 52, width: width - 40, height: 1))
        }
        return context.makeImage()!
    }

    private func drawLine(
        _ text: String,
        font: CTFont,
        color: NSColor,
        at point: CGPoint,
        in context: CGContext
    ) {
        let line = CTLineCreateWithAttributedString(NSAttributedString(
            string: text,
            attributes: [
                .font: font,
                .foregroundColor: color,
            ]
        ))
        context.textPosition = point
        CTLineDraw(line, context)
    }

    private func textViewport(
        page: CGImage,
        offset: Int,
        height: Int,
        fixedHeaderHeight: Int = 0,
        fixedFooterHeight: Int = 0
    ) -> CGImage? {
        guard let viewport = page.cropping(to: CGRect(
            x: 0,
            y: offset,
            width: page.width,
            height: height
        )) else {
            return nil
        }
        guard fixedHeaderHeight > 0 || fixedFooterHeight > 0 else { return viewport }
        let context = CGContext(
            data: nil,
            width: page.width,
            height: height,
            bitsPerComponent: 8,
            bytesPerRow: page.width * 4,
            space: CGColorSpaceCreateDeviceRGB(),
            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
        )!
        context.draw(viewport, in: CGRect(x: 0, y: 0, width: page.width, height: height))
        if fixedHeaderHeight > 0 {
            context.setFillColor(CGColor(gray: 0.97, alpha: 1))
            context.fill(CGRect(
                x: 0,
                y: height - fixedHeaderHeight,
                width: page.width,
                height: fixedHeaderHeight
            ))
            context.setFillColor(CGColor(red: 0.9, green: 0.08, blue: 0.08, alpha: 1))
            context.fill(CGRect(
                x: 22,
                y: height - fixedHeaderHeight + 13,
                width: 210,
                height: 12
            ))
        }
        if fixedFooterHeight > 0 {
            context.setFillColor(CGColor(gray: 0.91, alpha: 1))
            context.fill(CGRect(
                x: 0,
                y: 0,
                width: page.width,
                height: fixedFooterHeight
            ))
            context.setFillColor(CGColor(
                red: 0.28,
                green: 0.42,
                blue: 0.72,
                alpha: 1
            ))
            context.fill(CGRect(
                x: 20,
                y: 5,
                width: page.width - 40,
                height: 5
            ))
        }
        return context.makeImage()
    }

    private func shiftContent(_ image: CGImage, horizontallyBy offset: Int) -> CGImage {
        let context = CGContext(
            data: nil,
            width: image.width,
            height: image.height,
            bitsPerComponent: 8,
            bytesPerRow: image.width * 4,
            space: CGColorSpaceCreateDeviceRGB(),
            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
        )!
        context.setFillColor(CGColor(gray: 1, alpha: 1))
        context.fill(CGRect(x: 0, y: 0, width: image.width, height: image.height))
        context.draw(
            image,
            in: CGRect(
                x: offset,
                y: 0,
                width: image.width,
                height: image.height
            )
        )
        return context.makeImage()!
    }

    private func temporaryPNG() -> URL {
        FileManager.default.temporaryDirectory
            .appending(path: "ideacard-stable-stitch-\(UUID().uuidString).png")
    }

    private func loadImage(at url: URL) throws -> CGImage {
        let source = try #require(CGImageSourceCreateWithURL(url as CFURL, nil))
        return try #require(CGImageSourceCreateImageAtIndex(source, 0, nil))
    }

    private func imageBytes(_ image: CGImage) -> Data {
        guard let data = image.dataProvider?.data else { return Data() }
        return data as Data
    }

    private func expectedPixel(contentY: Int, x: Int) -> [UInt8] {
        [
            UInt8((contentY * 31 + x * 17 + contentY / 7) % 251),
            UInt8((contentY * 13 + x * 29 + x / 5) % 253),
            UInt8((contentY * 47 + x * 7 + contentY / 11) % 247),
            255,
        ]
    }

    private func redMarkerPixelCount(in image: CGImage) -> Int {
        guard
            let data = image.dataProvider?.data,
            let bytes = CFDataGetBytePtr(data)
        else {
            return 0
        }
        var count = 0
        for y in 0..<image.height {
            for x in 0..<image.width {
                let offset = y * image.bytesPerRow + x * 4
                if bytes[offset] > 200, bytes[offset + 1] < 80, bytes[offset + 2] < 80 {
                    count += 1
                }
            }
        }
        return count
    }

    private func blueMarkerPixelCount(in image: CGImage) -> Int {
        guard
            let data = image.dataProvider?.data,
            let bytes = CFDataGetBytePtr(data)
        else {
            return 0
        }
        var count = 0
        for y in 0..<image.height {
            for x in 0..<image.width {
                let offset = y * image.bytesPerRow + x * 4
                if bytes[offset] < 100,
                    bytes[offset + 1] > 80,
                    bytes[offset + 1] < 150,
                    bytes[offset + 2] > 150 {
                    count += 1
                }
            }
        }
        return count
    }

    private func pixel(in image: CGImage, x: Int, y: Int) -> [UInt8] {
        guard
            let data = image.dataProvider?.data,
            let bytes = CFDataGetBytePtr(data)
        else {
            return []
        }
        let offset = y * image.bytesPerRow + x * 4
        return Array(UnsafeBufferPointer(start: bytes + offset, count: 4))
    }
}
