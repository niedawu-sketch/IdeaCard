import AppKit
import CoreGraphics
import CoreImage
import CoreMedia
import Foundation
@preconcurrency import ScreenCaptureKit

@MainActor
final class ScrollingCaptureSession {
    typealias HintHandler = (String) -> Void
    typealias CompletionHandler = (Result<URL, Error>) -> Void

    private let screen: NSScreen
    private let localRect: CGRect
    private let outputURL: URL
    private let scrollingDriver: ScrollSpeedNormalizer?
    private let onReady: () -> Void
    private let onHint: HintHandler
    private let onFinished: CompletionHandler
    private let provider = ScreenCaptureFrameProvider()
    private var processor: ScrollingCaptureProcessor!
    private let startedAt = Date()
    private var timeoutTimer: Timer?
    private var isReady = false
    private var isStopped = false
    private var isFinishing = false
    private var didComplete = false

    init(
        screen: NSScreen,
        localRect: CGRect,
        outputURL: URL,
        scrollingDriver: ScrollSpeedNormalizer?,
        onReady: @escaping () -> Void,
        onHint: @escaping HintHandler,
        onFinished: @escaping CompletionHandler
    ) {
        self.screen = screen
        self.localRect = localRect
        self.outputURL = outputURL
        self.scrollingDriver = scrollingDriver
        self.onReady = onReady
        self.onHint = onHint
        self.onFinished = onFinished
        processor = ScrollingCaptureProcessor(
            outputURL: outputURL,
            onEvent: { [weak self] event in
                self?.handleProcessorEvent(event)
            }
        )
    }

    func start() {
        guard
            let number = screen.deviceDescription[NSDeviceDescriptionKey("NSScreenNumber")] as? NSNumber
        else {
            complete(.failure(NativeServiceError.screenshotFailed))
            return
        }
        let sourceRect = CGRect(
            x: localRect.minX,
            y: screen.frame.height - localRect.maxY,
            width: localRect.width,
            height: localRect.height
        ).integral
        provider.start(
            displayID: CGDirectDisplayID(number.uint32Value),
            sourceRect: sourceRect,
            scale: screen.backingScaleFactor,
            onFrame: { [weak self] image in
                self?.receive(image)
            }
        ) { [weak self] result in
            guard let self else { return }
            switch result {
            case .success:
                self.startTimeout()
            case let .failure(error):
                self.complete(.failure(error))
            }
        }
    }

    func finish() {
        guard !isFinishing, !didComplete else { return }
        isFinishing = true
        scrollingDriver?.stop()
        stopTimeout()
        provider.stop { [weak self] result in
            guard let self else { return }
            switch result {
            case .success:
                self.processor.finish { [weak self] result in
                    guard let self else { return }
                    switch result {
                    case let .success(url):
                        self.complete(.success(url))
                    case let .failure(error):
                        self.complete(.failure(error))
                    }
                }
            case let .failure(error):
                self.complete(.failure(error))
            }
        }
    }

    func cancel() {
        guard !didComplete else { return }
        isStopped = true
        scrollingDriver?.stop()
        stopTimeout()
        processor.cancel()
        provider.stop { _ in }
    }

    private func startTimeout() {
        timeoutTimer = Timer.scheduledTimer(withTimeInterval: 0.5, repeats: true) {
            [weak self] _ in
            Task { @MainActor [weak self] in
                self?.checkTimeout()
            }
        }
    }

    private func checkTimeout() {
        guard !isStopped, !isFinishing else { return }
        if Date().timeIntervalSince(startedAt) >= 120 {
            isStopped = true
            stopTimeout()
            provider.stop { _ in }
            onHint("已达到最长采集时间，请保存或取消")
        }
    }

    private func receive(_ image: CGImage) {
        guard !isStopped, !isFinishing else { return }
        processor.submit(image)
    }

    private func stopTimeout() {
        timeoutTimer?.invalidate()
        timeoutTimer = nil
    }

    private func handleProcessorEvent(_ event: ScrollingCaptureProcessor.Event) {
        guard !didComplete else { return }
        switch event {
        case .ready:
            guard !isReady else { return }
            isReady = true
            onReady()
            scrollingDriver?.start()
        case let .hint(message):
            onHint(message)
        case let .autoFinish(reason, didScroll):
            scrollingDriver?.stop()
            if scrollingDriver != nil, didScroll {
                switch reason {
                case .contentExhausted:
                    onHint("已到底部，正在保存…")
                case .heightLimitReached:
                    onHint("已达到最大截图长度，正在保存…")
                }
                finish()
            } else if scrollingDriver != nil {
                onHint("选区内容没有滚动，请按 Esc 取消")
            } else {
                switch reason {
                case .contentExhausted:
                    onHint("页面已停止滚动，可以保存了")
                case .heightLimitReached:
                    onHint("已达到最大截图长度，请保存")
                }
            }
        case let .failure(error):
            isFinishing = true
            scrollingDriver?.stop()
            stopTimeout()
            provider.stop { [weak self] _ in
                self?.complete(.failure(error))
            }
        }
    }

    private func complete(_ result: Result<URL, Error>) {
        guard !didComplete else { return }
        didComplete = true
        isStopped = true
        scrollingDriver?.stop()
        stopTimeout()
        if case .failure = result {
            processor.cancel()
        }
        onFinished(result)
    }
}

private final class ScrollingCaptureProcessor: @unchecked Sendable {
    enum Event: Sendable {
        case ready
        case hint(String)
        case autoFinish(AutoFinishReason, didScroll: Bool)
        case failure(NativeServiceError)
    }

    typealias FinishHandler = @MainActor @Sendable (
        Result<URL, NativeServiceError>
    ) -> Void

    private let outputURL: URL
    private let onEvent: @MainActor @Sendable (Event) -> Void
    private let processingQueue = DispatchQueue(
        label: "com.niedawu.ideacard.scrolling-analysis",
        qos: .userInitiated
    )
    private let pendingLock = NSLock()
    private let matcher = VerticalImageStitcher()
    private let maximumPendingFrames = 12
    private var pendingFrames: [CGImage] = []
    private var isWorkerScheduled = false
    private var isAcceptingFrames = true
    private var isCancelled = false
    private var finishHandler: FinishHandler?

    // The following state is accessed only from processingQueue.
    private var frames: [CGImage] = []
    private var latestFrame: CGImage?
    private var lastAnchorFrame: CGImage?
    private var estimatedHeight = 0
    private var reachedHeightLimit = false
    private var hasUnresolvedGap = false
    private var forwardFrameCount = 0
    private var unreliableFrameCount = 0
    private var reverseFrameCount = 0
    private var bottomDetection = BottomDetection()
    private var didEmitAutoFinish = false

    init(
        outputURL: URL,
        onEvent: @escaping @MainActor @Sendable (Event) -> Void
    ) {
        self.outputURL = outputURL
        self.onEvent = onEvent
    }

    func submit(_ image: CGImage) {
        pendingLock.lock()
        guard isAcceptingFrames, !isCancelled else {
            pendingLock.unlock()
            return
        }
        if pendingFrames.count < maximumPendingFrames {
            pendingFrames.append(image)
        } else {
            // Keep the earliest continuity frames and always retain the newest endpoint.
            pendingFrames[pendingFrames.count - 1] = image
        }
        let shouldSchedule = !isWorkerScheduled
        isWorkerScheduled = true
        pendingLock.unlock()
        if shouldSchedule {
            processingQueue.async { [weak self] in
                self?.drain()
            }
        }
    }

    @MainActor
    func finish(completion: @escaping FinishHandler) {
        pendingLock.lock()
        guard !isCancelled else {
            pendingLock.unlock()
            completion(.failure(.scrollingCaptureUnreliable))
            return
        }
        isAcceptingFrames = false
        finishHandler = completion
        let shouldSchedule = !isWorkerScheduled
        isWorkerScheduled = true
        pendingLock.unlock()
        if shouldSchedule {
            processingQueue.async { [weak self] in
                self?.drain()
            }
        }
    }

    func cancel() {
        pendingLock.lock()
        isCancelled = true
        isAcceptingFrames = false
        pendingFrames.removeAll()
        finishHandler = nil
        pendingLock.unlock()
    }

    private func drain() {
        while true {
            pendingLock.lock()
            if isCancelled {
                isWorkerScheduled = false
                pendingLock.unlock()
                return
            }
            let image = pendingFrames.isEmpty ? nil : pendingFrames.removeFirst()
            if image == nil, let completion = finishHandler {
                finishHandler = nil
                isWorkerScheduled = false
                pendingLock.unlock()
                renderAndComplete(completion)
                return
            }
            if image == nil {
                isWorkerScheduled = false
                pendingLock.unlock()
                return
            }
            pendingLock.unlock()

            do {
                try process(image!)
            } catch {
                pendingLock.lock()
                isAcceptingFrames = false
                pendingFrames.removeAll()
                isWorkerScheduled = false
                pendingLock.unlock()
                emit(.failure(.scrollingCaptureUnreliable))
                return
            }
        }
    }

    private func process(_ image: CGImage) throws {
        latestFrame = image
        guard !reachedHeightLimit else { return }
        guard let previous = lastAnchorFrame else {
            frames = [image]
            lastAnchorFrame = image
            estimatedHeight = image.height
            captureTrace("initial frame=\(image.width)x\(image.height)")
            emit(.ready)
            return
        }
        let relationship = try matcher.fastRelationship(from: previous, to: image)
        trackBottomProgress(relationship)
        switch relationship {
        case .duplicate:
            hasUnresolvedGap = false
        case .reverse:
            reverseFrameCount += 1
            hasUnresolvedGap = true
        case .unreliable:
            unreliableFrameCount += 1
            hasUnresolvedGap = true
        case let .forward(match):
            forwardFrameCount += 1
            hasUnresolvedGap = false
            guard match.shift >= anchorRetentionThreshold(for: image) else {
                return
            }
            guard estimatedHeight + match.shift <= 30_000 else {
                reachedHeightLimit = true
                didEmitAutoFinish = true
                emit(.autoFinish(.heightLimitReached, didScroll: true))
                return
            }
            frames.append(image)
            lastAnchorFrame = image
            estimatedHeight += match.shift
            captureTrace(
                "anchor retained count=\(frames.count) shift=\(match.shift) "
                    + "score=\(match.score) confidence=\(match.confidence) "
                    + "estimatedHeight=\(estimatedHeight)"
            )
        }
    }

    private func renderAndComplete(_ completion: @escaping FinishHandler) {
        captureTrace(
            "finish anchors=\(frames.count) estimatedHeight=\(estimatedHeight) "
                + "unresolvedGap=\(hasUnresolvedGap) forward=\(forwardFrameCount) "
                + "unreliable=\(unreliableFrameCount) reverse=\(reverseFrameCount)"
        )
        let result: Result<URL, NativeServiceError>
        do {
            try includeFinalFrameIfNeeded()
            captureTrace("render anchors=\(frames.count) estimatedHeight=\(estimatedHeight)")
            try VerticalImageStitcher().writeValidatedPNG(frames: frames, to: outputURL)
            result = .success(outputURL)
        } catch {
            captureTrace("render error=\(String(describing: error)) anchors=\(frames.count)")
            try? FileManager.default.removeItem(at: outputURL)
            result = .failure(.scrollingCaptureUnreliable)
        }
        Task { @MainActor in
            completion(result)
        }
    }

    private func includeFinalFrameIfNeeded() throws {
        guard
            !reachedHeightLimit,
            let latestFrame,
            let previous = lastAnchorFrame
        else {
            return
        }
        switch try matcher.relationship(from: previous, to: latestFrame) {
        case .duplicate:
            captureTrace("final frame duplicate unresolvedGap=\(hasUnresolvedGap)")
            guard !hasUnresolvedGap else {
                throw VerticalImageStitcherError.unreliableSequence
            }
        case let .forward(match):
            captureTrace("final frame forward shift=\(match.shift) score=\(match.score)")
            guard estimatedHeight + match.shift <= 30_000 else {
                reachedHeightLimit = true
                return
            }
            frames.append(latestFrame)
            lastAnchorFrame = latestFrame
            estimatedHeight += match.shift
            hasUnresolvedGap = false
        case .reverse:
            captureTrace("final frame reverse")
            throw VerticalImageStitcherError.unreliableSequence
        case .unreliable:
            captureTrace("final frame unreliable")
            throw VerticalImageStitcherError.unreliableSequence
        }
    }

    private func anchorRetentionThreshold(for image: CGImage) -> Int {
        max(32, Int(Double(image.height) * 0.34))
    }

    private func trackBottomProgress(_ relationship: VerticalFrameRelationship) {
        guard !reachedHeightLimit, !didEmitAutoFinish else { return }
        guard bottomDetection.recordFrame(isDuplicate: relationship == .duplicate) else {
            return
        }
        didEmitAutoFinish = true
        emit(.autoFinish(.contentExhausted, didScroll: forwardFrameCount > 0))
    }

    private func emit(_ event: Event) {
        Task { @MainActor in
            onEvent(event)
        }
    }
}

private func captureTrace(_ message: String) {
    guard ProcessInfo.processInfo.environment["IDEACARD_CAPTURE_TRACE"] == "1" else {
        return
    }
    try? FileHandle.standardError.write(
        contentsOf: Data("[scroll-capture] \(message)\n".utf8)
    )
}

private final class ScreenCaptureFrameProvider: NSObject, SCStreamOutput, SCStreamDelegate,
    @unchecked Sendable
{
    private let outputQueue = DispatchQueue(
        label: "com.niedawu.ideacard.scrolling-capture",
        qos: .userInitiated
    )
    private let imageContext = CIContext(options: [.cacheIntermediates: false])
    private let frameLock = NSLock()
    private var stream: SCStream?
    private var frameHandler: (@MainActor @Sendable (CGImage) -> Void)?
    private var stopHandler: (@MainActor @Sendable (Result<Void, Error>) -> Void)?
    private var didReportStop = false
    private var pendingFrames: [CGImage] = []
    private var deliveryScheduled = false
    private let maximumPendingFrames = 8

    @MainActor
    func start(
        displayID: CGDirectDisplayID,
        sourceRect: CGRect,
        scale: CGFloat,
        onFrame: @escaping @MainActor @Sendable (CGImage) -> Void,
        completion: @escaping @MainActor @Sendable (Result<Void, Error>) -> Void
    ) {
        frameHandler = onFrame
        SCShareableContent.getExcludingDesktopWindows(
            false,
            onScreenWindowsOnly: true
        ) { content, error in
            let transfer = UnsafeTransfer((content, error))
            Task { @MainActor in
                let (content, error) = transfer.value
                if let error {
                    completion(.failure(error))
                    return
                }
                guard
                    let content,
                    let display = content.displays.first(where: { $0.displayID == displayID })
                else {
                    completion(.failure(NativeServiceError.screenshotFailed))
                    return
                }
                let ownApplications = content.applications.filter {
                    $0.processID == ProcessInfo.processInfo.processIdentifier
                }
                let filter = SCContentFilter(
                    display: display,
                    excludingApplications: ownApplications,
                    exceptingWindows: []
                )
                let configuration = SCStreamConfiguration()
                configuration.sourceRect = sourceRect
                configuration.width = max(1, Int(sourceRect.width * scale))
                configuration.height = max(1, Int(sourceRect.height * scale))
                configuration.showsCursor = false
                configuration.capturesAudio = false
                configuration.minimumFrameInterval = CMTime(
                    value: 1,
                    timescale: ScrollingCaptureRate.framesPerSecond
                )
                configuration.queueDepth = 5
                let stream = SCStream(
                    filter: filter,
                    configuration: configuration,
                    delegate: self
                )
                do {
                    try stream.addStreamOutput(
                        self,
                        type: .screen,
                        sampleHandlerQueue: self.outputQueue
                    )
                } catch {
                    completion(.failure(error))
                    return
                }
                self.stream = stream
                Task { @MainActor in
                    do {
                        try await stream.startCapture()
                        completion(.success(()))
                    } catch {
                        completion(.failure(error))
                    }
                }
            }
        }
    }

    @MainActor
    func stop(
        completion: @escaping @MainActor @Sendable (Result<Void, Error>) -> Void
    ) {
        guard let stream else {
            completion(.success(()))
            return
        }
        stopHandler = completion
        stream.stopCapture { [weak self] error in
            let transfer = UnsafeTransfer(error)
            Task { @MainActor in
                self?.finishStop(transfer.value.map(Result.failure) ?? .success(()))
            }
        }
    }

    nonisolated func stream(
        _ stream: SCStream,
        didOutputSampleBuffer sampleBuffer: CMSampleBuffer,
        of outputType: SCStreamOutputType
    ) {
        guard
            outputType == .screen,
            sampleBuffer.isValid,
            let attachments = CMSampleBufferGetSampleAttachmentsArray(
                sampleBuffer,
                createIfNecessary: false
            ) as? [[SCStreamFrameInfo: Any]],
            let statusRawValue = attachments.first?[.status] as? Int,
            SCFrameStatus(rawValue: statusRawValue) == .complete,
            let pixelBuffer = CMSampleBufferGetImageBuffer(sampleBuffer)
        else {
            return
        }
        let source = CIImage(cvPixelBuffer: pixelBuffer)
        guard let image = imageContext.createCGImage(source, from: source.extent) else {
            return
        }
        enqueue(image)
    }

    nonisolated func stream(_ stream: SCStream, didStopWithError error: Error) {
        let transfer = UnsafeTransfer(error)
        Task { @MainActor [weak self] in
            self?.finishStop(.failure(transfer.value))
        }
    }

    @MainActor
    private func finishStop(_ result: Result<Void, Error>) {
        guard !didReportStop else { return }
        didReportStop = true
        deliverAllPendingFrames()
        stream = nil
        frameHandler = nil
        let handler = stopHandler
        stopHandler = nil
        handler?(result)
    }

    nonisolated private func enqueue(_ image: CGImage) {
        frameLock.lock()
        if pendingFrames.count < maximumPendingFrames {
            pendingFrames.append(image)
        } else {
            // Preserve the earliest continuity frames and keep the newest endpoint.
            // Final validation will reject the sequence if that endpoint is too far away.
            pendingFrames[pendingFrames.count - 1] = image
        }
        let shouldSchedule = !deliveryScheduled
        deliveryScheduled = true
        frameLock.unlock()
        guard shouldSchedule else { return }
        Task { @MainActor [weak self] in
            self?.deliverPendingFrame()
        }
    }

    @MainActor
    private func deliverPendingFrame() {
        frameLock.lock()
        let image = pendingFrames.isEmpty ? nil : pendingFrames.removeFirst()
        frameLock.unlock()
        guard let image else {
            frameLock.lock()
            deliveryScheduled = false
            frameLock.unlock()
            return
        }
        frameHandler?(image)
        frameLock.lock()
        let hasMore = !pendingFrames.isEmpty
        if !hasMore {
            deliveryScheduled = false
        }
        frameLock.unlock()
        if hasMore {
            Task { @MainActor [weak self] in
                self?.deliverPendingFrame()
            }
        }
    }

    @MainActor
    private func deliverAllPendingFrames() {
        while true {
            frameLock.lock()
            let image = pendingFrames.isEmpty ? nil : pendingFrames.removeFirst()
            if image == nil {
                deliveryScheduled = false
            }
            frameLock.unlock()
            guard let image else { return }
            frameHandler?(image)
        }
    }
}

private final class UnsafeTransfer<Value>: @unchecked Sendable {
    let value: Value

    init(_ value: Value) {
        self.value = value
    }
}
