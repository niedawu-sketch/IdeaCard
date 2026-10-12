import AppKit
import Foundation
import Testing
@testable import IdeaCardNative

@Suite("OCRServiceTests")
struct OCRServiceTests {
    @Test("Recognizes text from a generated image")
    @MainActor
    func recognizesGeneratedImage() throws {
        let imageURL = try writePNG(text: "IdeaCard 测试 2026")
        defer { try? FileManager.default.removeItem(at: imageURL) }

        let text = try OCRService().recognizeText(at: imageURL)

        #expect(text.contains("IdeaCard"))
        #expect(text.contains("2026"))
    }

    @Test("Returns an empty string for a blank image")
    @MainActor
    func blankImageReturnsEmptyText() throws {
        let imageURL = try writePNG(text: nil)
        defer { try? FileManager.default.removeItem(at: imageURL) }

        #expect(try OCRService().recognizeText(at: imageURL) == "")
    }

    @Test("Recognizes text from both ends of a tall screenshot")
    @MainActor
    func recognizesTallScreenshotInTiles() throws {
        let imageURL = try writeTallPNG()
        defer { try? FileManager.default.removeItem(at: imageURL) }

        let text = try OCRService().recognizeText(at: imageURL)

        #expect(text.contains("TOP SECTION 2026"))
        #expect(text.contains("BOTTOM SECTION 2026"))
    }

    @Test("Maps a missing file to a stable error")
    func missingFileHasStableError() {
        let missingURL = FileManager.default.temporaryDirectory
            .appending(path: "ideacard-missing-\(UUID().uuidString).png")

        expectServiceError(code: "native_file_missing") {
            _ = try OCRService().recognizeText(at: missingURL)
        }
    }

    @Test("Maps unsupported image data to a stable error")
    func unsupportedImageHasStableError() throws {
        let invalidURL = FileManager.default.temporaryDirectory
            .appending(path: "ideacard-invalid-\(UUID().uuidString).png")
        try Data("not an image".utf8).write(to: invalidURL)
        defer { try? FileManager.default.removeItem(at: invalidURL) }

        expectServiceError(code: "native_image_unsupported") {
            _ = try OCRService().recognizeText(at: invalidURL)
        }
    }

    @Test("Dispatches OCR requests through the native protocol")
    @MainActor
    func protocolDispatchesOCR() throws {
        let imageURL = try writePNG(text: "IdeaCard 2026")
        defer { try? FileManager.default.removeItem(at: imageURL) }

        let response = dispatchNativeRequest(
            NativeRequest(
                command: .ocr,
                path: imageURL.path,
                service: nil,
                account: nil,
                secret: nil
            )
        )

        #expect(response.ok)
        #expect(response.text?.contains("IdeaCard") == true)
        #expect(response.errorCode == nil)
    }

    @MainActor
    private func writePNG(text: String?) throws -> URL {
        let image = NSImage(size: NSSize(width: 1_000, height: 300))
        image.lockFocus()
        NSColor.white.setFill()
        NSRect(x: 0, y: 0, width: 1_000, height: 300).fill()
        if let text {
            (text as NSString).draw(
                at: NSPoint(x: 50, y: 105),
                withAttributes: [
                    .font: NSFont.systemFont(ofSize: 72, weight: .medium),
                    .foregroundColor: NSColor.black,
                ]
            )
        }
        image.unlockFocus()

        guard
            let tiff = image.tiffRepresentation,
            let bitmap = NSBitmapImageRep(data: tiff),
            let png = bitmap.representation(using: .png, properties: [:])
        else {
            throw FixtureError.pngEncodingFailed
        }

        let url = FileManager.default.temporaryDirectory
            .appending(path: "ideacard-ocr-\(UUID().uuidString).png")
        try png.write(to: url)
        return url
    }

    @MainActor
    private func writeTallPNG() throws -> URL {
        let size = NSSize(width: 1_000, height: 2_600)
        let image = NSImage(size: size)
        image.lockFocus()
        NSColor.white.setFill()
        NSRect(origin: .zero, size: size).fill()
        let attributes: [NSAttributedString.Key: Any] = [
            .font: NSFont.systemFont(ofSize: 60, weight: .medium),
            .foregroundColor: NSColor.black,
        ]
        ("TOP SECTION 2026" as NSString).draw(
            at: NSPoint(x: 50, y: 2_350),
            withAttributes: attributes
        )
        ("BOTTOM SECTION 2026" as NSString).draw(
            at: NSPoint(x: 50, y: 120),
            withAttributes: attributes
        )
        image.unlockFocus()

        guard
            let tiff = image.tiffRepresentation,
            let bitmap = NSBitmapImageRep(data: tiff),
            let png = bitmap.representation(using: .png, properties: [:])
        else {
            throw FixtureError.pngEncodingFailed
        }
        let url = FileManager.default.temporaryDirectory
            .appending(path: "ideacard-ocr-tall-\(UUID().uuidString).png")
        try png.write(to: url)
        return url
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

private enum FixtureError: Error {
    case pngEncodingFailed
}
