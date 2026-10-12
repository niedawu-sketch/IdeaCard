import Foundation

func writeDiagnostic(_ code: String) {
    try? FileHandle.standardError.write(contentsOf: Data("\(code)\n".utf8))
}

let response: NativeResponse

do {
    let input = try FileHandle.standardInput.readToEnd() ?? Data()
    let request = try JSONDecoder().decode(NativeRequest.self, from: input)
    response = dispatchNativeRequest(request)
} catch {
    writeDiagnostic("invalid_request")
    response = .failure(code: "invalid_request", message: "无法读取原生请求")
}

do {
    var output = try JSONEncoder().encode(response)
    output.append(0x0A)
    try FileHandle.standardOutput.write(contentsOf: output)
} catch {
    writeDiagnostic("output_failed")
    exit(EXIT_FAILURE)
}
