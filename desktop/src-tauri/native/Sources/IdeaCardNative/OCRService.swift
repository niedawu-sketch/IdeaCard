import Foundation
import ImageIO
import Vision

enum NativeServiceError: Error, Equatable, Sendable {
    case fileMissing
    case imageUnsupported
    case ocrFailed
    case screenPermissionDenied
    case screenshotFailed
    case scrollingCaptureUnreliable
    case scrollingTargetUnavailable
    case accessibilityNotTrusted

    var code: String {
        switch self {
        case .fileMissing:
            "native_file_missing"
        case .imageUnsupported:
            "native_image_unsupported"
        case .ocrFailed:
            "native_ocr_failed"
        case .screenPermissionDenied:
            "screen_permission_denied"
        case .screenshotFailed:
            "screenshot_failed"
        case .scrollingCaptureUnreliable:
            "scrolling_capture_unreliable"
        case .scrollingTargetUnavailable:
            "scrolling_target_unavailable"
        case .accessibilityNotTrusted:
            "accessibility_not_trusted"
        }
    }

    var message: String {
        switch self {
        case .fileMissing:
            "找不到需要识别的图片"
        case .imageUnsupported:
            "无法读取这张图片"
        case .ocrFailed:
            "图片文字识别失败"
        case .screenPermissionDenied:
            "没有屏幕录制权限"
        case .screenshotFailed:
            "区域截图失败"
        case .scrollingCaptureUnreliable:
            "页面内容无法可靠拼接，未生成卡片"
        case .scrollingTargetUnavailable:
            "无法控制目标页面的滚动"
        case .accessibilityNotTrusted:
            "滚动截图需要辅助功能权限"
        }
    }
}

struct OCRService {
    func recognizeText(at imageURL: URL) throws -> String {
        guard imageURL.isFileURL, FileManager.default.fileExists(atPath: imageURL.path) else {
            throw NativeServiceError.fileMissing
        }
        guard
            let imageSource = CGImageSourceCreateWithURL(imageURL as CFURL, nil),
            CGImageSourceGetCount(imageSource) > 0,
            let image = CGImageSourceCreateImageAtIndex(imageSource, 0, nil)
        else {
            throw NativeServiceError.imageUnsupported
        }

        if image.height > 1_600, imageOrientation(imageSource) == .up {
            return try recognizeTallImage(image).joined(separator: "\n")
        }
        return try recognizeImage(at: imageURL).joined(separator: "\n")
    }

    private func recognizeTallImage(_ image: CGImage) throws -> [String] {
        let tileHeight = 1_400
        let overlap = 200
        let step = tileHeight - overlap
        var lines: [String] = []
        var top = 0
        while top < image.height {
            let height = min(tileHeight, image.height - top)
            guard let tile = image.cropping(to: CGRect(
                x: 0,
                y: top,
                width: image.width,
                height: height
            )) else {
                throw NativeServiceError.ocrFailed
            }
            let hasPrevious = top > 0
            let hasNext = top + height < image.height
            let topTrim = hasPrevious ? CGFloat(overlap) / 2 : 0
            let bottomTrim = hasNext ? CGFloat(overlap) / 2 : 0
            let observations = try recognizeObservations(in: tile)
            lines.append(contentsOf: observations.compactMap { observation in
                let centerFromTop = (1 - observation.boundingBox.midY) * CGFloat(height)
                guard
                    centerFromTop >= topTrim,
                    centerFromTop <= CGFloat(height) - bottomTrim
                else {
                    return nil
                }
                return observation.topCandidates(1).first?.string
            })
            if !hasNext { break }
            top += step
        }
        return lines
    }

    private func recognizeImage(at imageURL: URL) throws -> [String] {
        let request = VNRecognizeTextRequest()
        configure(request)

        do {
            try VNImageRequestHandler(url: imageURL).perform([request])
        } catch {
            throw NativeServiceError.ocrFailed
        }

        return (request.results ?? []).sorted(by: readingOrder)
            .compactMap { $0.topCandidates(1).first?.string }
    }

    private func recognizeObservations(in image: CGImage) throws -> [VNRecognizedTextObservation] {
        let request = VNRecognizeTextRequest()
        configure(request)
        do {
            try VNImageRequestHandler(cgImage: image).perform([request])
        } catch {
            throw NativeServiceError.ocrFailed
        }
        return (request.results ?? []).sorted(by: readingOrder)
    }

    private func configure(_ request: VNRecognizeTextRequest) {
        request.recognitionLevel = .accurate
        request.usesLanguageCorrection = true
        request.recognitionLanguages = ["zh-Hans", "en-US"]
    }

    private func imageOrientation(_ source: CGImageSource) -> CGImagePropertyOrientation {
        guard
            let properties = CGImageSourceCopyPropertiesAtIndex(source, 0, nil)
                as? [CFString: Any],
            let rawValue = properties[kCGImagePropertyOrientation] as? NSNumber
        else {
            return .up
        }
        return CGImagePropertyOrientation(rawValue: rawValue.uint32Value) ?? .up
    }

    private func readingOrder(_ left: VNRecognizedTextObservation, _ right: VNRecognizedTextObservation) -> Bool {
        if left.boundingBox.maxY != right.boundingBox.maxY {
            return left.boundingBox.maxY > right.boundingBox.maxY
        }
        return left.boundingBox.minX < right.boundingBox.minX
    }
}
