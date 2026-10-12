import Foundation

enum NativeCommand: String, Codable, Sendable {
    case ocr
    case screenshot
    case screenPermission
    case keychainSet
    case keychainGet
    case keychainDelete
}

struct NativeRequest: Codable, Equatable, Sendable {
    let command: NativeCommand
    let path: String?
    let service: String?
    let account: String?
    let secret: String?
}

struct NativeResponse: Codable, Equatable, Sendable {
    let ok: Bool
    let text: String?
    let path: String?
    let permission: String?
    let captureKind: String?
    let errorCode: String?
    let message: String?
    let cancelled: Bool?

    init(
        ok: Bool,
        text: String?,
        path: String?,
        permission: String?,
        captureKind: String? = nil,
        errorCode: String?,
        message: String?,
        cancelled: Bool? = nil
    ) {
        self.ok = ok
        self.text = text
        self.path = path
        self.permission = permission
        self.captureKind = captureKind
        self.errorCode = errorCode
        self.message = message
        self.cancelled = cancelled
    }

    static func failure(code: String, message: String) -> NativeResponse {
        NativeResponse(
            ok: false,
            text: nil,
            path: nil,
            permission: nil,
            captureKind: nil,
            errorCode: code,
            message: message
        )
    }

    static func success(
        text: String? = nil,
        path: String? = nil,
        permission: String? = nil,
        captureKind: String? = nil
    ) -> NativeResponse {
        NativeResponse(
            ok: true,
            text: text,
            path: path,
            permission: permission,
            captureKind: captureKind,
            errorCode: nil,
            message: nil
        )
    }

    static func cancelledCapture() -> NativeResponse {
        NativeResponse(
            ok: true,
            text: nil,
            path: nil,
            permission: nil,
            captureKind: nil,
            errorCode: nil,
            message: nil,
            cancelled: true
        )
    }
}

@MainActor
func dispatchNativeRequest(_ request: NativeRequest) -> NativeResponse {
    dispatchNativeRequest(request, screenshotService: ScreenshotService())
}

@MainActor
func dispatchNativeRequest(
    _ request: NativeRequest,
    screenshotService: ScreenshotService
) -> NativeResponse {
    switch request.command {
    case .ocr:
        guard let path = request.path, !path.isEmpty else {
            return .failure(code: "invalid_request", message: "缺少图片路径")
        }
        do {
            return .success(text: try OCRService().recognizeText(at: URL(fileURLWithPath: path)))
        } catch let error as NativeServiceError {
            return .failure(code: error.code, message: error.message)
        } catch {
            return .failure(code: "native_ocr_failed", message: "图片文字识别失败")
        }
    case .screenPermission:
        return .success(permission: screenshotService.permissionStatus().rawValue)
    case .screenshot:
        guard let path = request.path, path.hasPrefix("/") else {
            return .failure(code: "invalid_request", message: "截图输出路径无效")
        }
        do {
            switch try screenshotService.captureRegion(to: URL(fileURLWithPath: path)) {
            case let .captured(outputURL, captureKind, note):
                return .success(
                    text: note,
                    path: outputURL.path,
                    captureKind: captureKind.rawValue
                )
            case .cancelled:
                return .cancelledCapture()
            }
        } catch let error as NativeServiceError {
            return .failure(code: error.code, message: error.message)
        } catch {
            return .failure(code: "screenshot_failed", message: "区域截图失败")
        }
    case .keychainSet, .keychainGet, .keychainDelete:
        return .failure(code: "native_command_unavailable", message: "原生能力尚未就绪")
    }
}
