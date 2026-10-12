import Foundation
import Testing
@testable import IdeaCardNative

@Suite("Native protocol")
struct NativeProtocolTests {
    @Test("OCR request round-trips")
    func ocrRequestRoundTrip() throws {
        let request = NativeRequest(
            command: .ocr,
            path: "/tmp/input.png",
            service: nil,
            account: nil,
            secret: nil
        )

        let data = try JSONEncoder().encode(request)
        let decoded = try JSONDecoder().decode(NativeRequest.self, from: data)

        #expect(decoded.command == .ocr)
        #expect(decoded.path == "/tmp/input.png")
        #expect(decoded.secret == nil)
    }

    @Test("Failure response uses stable error fields")
    func failureResponseUsesStableErrorFields() throws {
        let response = NativeResponse(
            ok: false,
            text: nil,
            path: nil,
            permission: nil,
            errorCode: "invalid_request",
            message: "无法读取原生请求"
        )

        let data = try JSONEncoder().encode(response)
        let decoded = try JSONDecoder().decode(NativeResponse.self, from: data)

        #expect(decoded.ok == false)
        #expect(decoded.errorCode == "invalid_request")
        #expect(decoded.message == "无法读取原生请求")
    }
}
