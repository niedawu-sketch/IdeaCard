import AppKit
import CoreGraphics

enum ScreenshotFeatureFlags {
    static let scrollingCaptureEnabled = false
}

@MainActor
final class AppKitRegionSelector: RegionSelecting {
    func selectRegion(outputURL: URL) throws -> RegionSelectionResult {
        let session = RegionSelectionSession(outputURL: outputURL)
        return try session.run()
    }
}

@MainActor
private final class RegionSelectionSession {
    private static let maximumNoteCharacters = 20_000

    private let outputURL: URL
    private var windows: [SelectionWindow] = []
    private var result: RegionSelectionResult = .cancelled
    private var failure: Error?
    private var didFinish = false
    private var scrollingCapture: ScrollingCaptureSession?
    private var controlPanel: CaptureControlPanel?
    private var hintPanel: CaptureHintPanel?
    private var resignObserver: NSObjectProtocol?
    private var parentWatchdog: Timer?
    private var noteDraft = ""
    private let sourceApplicationPID: pid_t?

    init(outputURL: URL) {
        self.outputURL = outputURL
        sourceApplicationPID = NSWorkspace.shared.frontmostApplication?.processIdentifier
    }

    func run() throws -> RegionSelectionResult {
        let application = NSApplication.shared
        application.setActivationPolicy(.accessory)
        // While the overlay is up the user must not end up interacting with
        // something else: any focus theft (system permission prompt, another
        // app clicked through, Cmd-Tab) tears the session down instead of
        // leaving an unclosable mask behind.
        resignObserver = NotificationCenter.default.addObserver(
            forName: NSApplication.didResignActiveNotification,
            object: application,
            queue: .main
        ) { [weak self] _ in
            Task { @MainActor [weak self] in
                self?.finish(with: .cancelled)
            }
        }
        // The main app spawns this helper per capture; if it dies, the
        // overlay must never outlive it as an unclosable orphan.
        parentWatchdog = Timer.scheduledTimer(withTimeInterval: 1.0, repeats: true) {
            [weak self] _ in
            Task { @MainActor [weak self] in
                guard getppid() == 1 else { return }
                self?.finish(with: .cancelled)
            }
        }

        for screen in NSScreen.screens {
            let window = SelectionWindow(screen: screen)
            window.selectionView.onBeginSelection = { [weak self, weak view = window.selectionView] in
                guard let self, let view else { return }
                self.windows
                    .map(\.selectionView)
                    .filter { $0 !== view }
                    .forEach { $0.resetSelection() }
                view.setNote(self.noteDraft)
                window.makeKey()
            }
            window.selectionView.onNoteChanged = { [weak self] note in
                self?.noteDraft = String(note.prefix(Self.maximumNoteCharacters))
            }
            window.selectionView.onCancel = { [weak self] in
                self?.finish(with: .cancelled)
            }
            window.selectionView.onSave = { [weak self, weak screen] localRect, note in
                guard let self, let screen else { return }
                self.noteDraft = String(note.prefix(Self.maximumNoteCharacters))
                self.finish(with: .selected(ScreenshotSelection(
                    captureRect: captureRect(for: localRect, on: screen),
                    note: self.noteDraft
                )))
            }
            if ScreenshotFeatureFlags.scrollingCaptureEnabled {
                window.selectionView.onStartScrolling = {
                    [weak self, weak window, weak screen] localRect in
                    guard let self, let window, let screen else { return }
                    self.beginScrollingCapture(
                        localRect: localRect,
                        screen: screen,
                        activeWindow: window
                    )
                }
            }
            windows.append(window)
            window.orderFrontRegardless()
        }

        if let activeWindow = windows.first(where: { $0.frame.contains(NSEvent.mouseLocation) })
            ?? windows.first {
            activeWindow.makeKeyAndOrderFront(nil)
        }
        application.activate(ignoringOtherApps: true)
        NSCursor.crosshair.set()
        application.run()
        NSCursor.arrow.set()
        if let failure {
            throw failure
        }
        return result
    }

    private func finish(with result: RegionSelectionResult) {
        guard !didFinish else { return }
        didFinish = true
        self.result = result
        if let observer = resignObserver {
            NotificationCenter.default.removeObserver(observer)
            resignObserver = nil
        }
        parentWatchdog?.invalidate()
        parentWatchdog = nil
        scrollingCapture?.cancel()
        scrollingCapture = nil
        controlPanel?.orderOut(nil)
        controlPanel = nil
        hintPanel?.orderOut(nil)
        hintPanel = nil
        windows.forEach { $0.orderOut(nil) }
        NSApplication.shared.stop(nil)
        NSApplication.shared.postEvent(
            NSEvent.otherEvent(
                with: .applicationDefined,
                location: .zero,
                modifierFlags: [],
                timestamp: 0,
                windowNumber: 0,
                context: nil,
                subtype: 0,
                data1: 0,
                data2: 0
            )!,
            atStart: false
        )
    }

    private func beginScrollingCapture(
        localRect: CGRect,
        screen: NSScreen,
        activeWindow: SelectionWindow
    ) {
        guard scrollingCapture == nil else { return }
        guard SyntheticScrollPermission.isTrusted else {
            // The main app owns the permission prompt (it registers a proper
            // "IdeaCard" entry in the Accessibility list); the helper just
            // reports the requirement and exits the capture cleanly.
            finish(with: .accessibilityRequired)
            return
        }
        guard let driver = makeScrollingDriver(localRect: localRect, screen: screen) else {
            failure = NativeServiceError.scrollingTargetUnavailable
            finish(with: .cancelled)
            return
        }
        windows.forEach {
            $0.selectionView.enterScrollingMode()
        }
        let panel = CaptureControlPanel(
            screen: screen,
            selectionRect: localRect,
            onCancel: { [weak self] in
                self?.finish(with: .cancelled)
            },
            onSave: { [weak self] in
                self?.finishScrollingCapture()
            }
        )
        controlPanel = panel
        panel.orderFrontRegardless()
        panel.makeKey()

        windows.forEach {
            $0.selectionView.onScrollingShieldClick = { [weak self] in
                NSApplication.shared.activate(ignoringOtherApps: true)
                self?.bringControlPanelForward()
            }
        }

        let capture = ScrollingCaptureSession(
            screen: screen,
            localRect: localRect,
            outputURL: outputURL,
            scrollingDriver: driver,
            onReady: { [weak self] in
                guard let self else { return }
                self.showHint(
                    "向下滚动即可，滚快会自动限速；滚到底部会自动保存",
                    beside: panel
                )
            },
            onHint: { [weak self] message in
                self?.showHint(message, beside: panel)
            },
            onFinished: { [weak self] result in
                guard let self else { return }
                switch result {
                case let .success(url):
                    self.scrollingCapture = nil
                    self.finish(with: .scrollingCaptured(url))
                case let .failure(error):
                    self.failure = error
                    self.scrollingCapture = nil
                    self.finish(with: .cancelled)
                }
            }
        )
        scrollingCapture = capture
        capture.start()
        activeWindow.selectionView.needsDisplay = true
    }

    /// Builds the scroll normalizer when Accessibility trust lets us repost
    /// the user's scroll intent at a fixed rate. Window hit-testing is the
    /// precise path; the application that was frontmost before the overlay
    /// activated is a safe fallback for pages with unusual window metadata.
    private func makeScrollingDriver(localRect: CGRect, screen: NSScreen) -> ScrollSpeedNormalizer? {
        guard SyntheticScrollPermission.isTrusted else {
            return nil
        }
        let rect = captureRect(for: localRect, on: screen)
        let center = CGPoint(x: rect.midX, y: rect.midY)
        let ownPID = ProcessInfo.processInfo.processIdentifier
        let locatedPID = WindowTargetLocator.frontmostPID(at: center, excluding: ownPID)
        guard let pid = locatedPID ?? sourceApplicationPID, pid != ownPID else {
            return nil
        }
        let driver = ScrollSpeedNormalizer(center: center, targetPID: pid)
        windows.forEach {
            $0.selectionView.onScrollIntent = { [weak driver] points in
                driver?.ingest(points: points)
            }
        }
        return driver
    }

    private func finishScrollingCapture() {
        guard let controlPanel, let scrollingCapture else { return }
        controlPanel.setProcessing()
        showHint("正在生成并校验长图…", beside: controlPanel)
        scrollingCapture.finish()
    }

    private func bringControlPanelForward() {
        guard let controlPanel else { return }
        controlPanel.orderFrontRegardless()
        controlPanel.makeKey()
    }

    private func showHint(_ message: String, beside panel: NSWindow) {
        hintPanel?.orderOut(nil)
        let hint = CaptureHintPanel(message: message, beside: panel)
        hintPanel = hint
        hint.orderFrontRegardless()
        DispatchQueue.main.asyncAfter(deadline: .now() + 2.4) { [weak self, weak hint] in
            guard self?.hintPanel === hint else { return }
            hint?.orderOut(nil)
            self?.hintPanel = nil
        }
    }
}

private func captureRect(for localRect: CGRect, on screen: NSScreen) -> CGRect {
    guard
        let number = screen.deviceDescription[NSDeviceDescriptionKey("NSScreenNumber")] as? NSNumber
    else {
        return .zero
    }
    let displayBounds = CGDisplayBounds(CGDirectDisplayID(number.uint32Value))
    return CGRect(
        x: displayBounds.minX + localRect.minX,
        y: displayBounds.minY + screen.frame.height - localRect.maxY,
        width: localRect.width,
        height: localRect.height
    ).integral
}

@MainActor
private final class SelectionWindow: NSWindow {
    let selectionView: SelectionOverlayView

    convenience init(screen: NSScreen) {
        self.init(
            contentRect: screen.frame,
            styleMask: .borderless,
            backing: .buffered,
            defer: false
        )
    }

    override init(
        contentRect: NSRect,
        styleMask style: NSWindow.StyleMask,
        backing backingStoreType: NSWindow.BackingStoreType,
        defer flag: Bool
    ) {
        selectionView = SelectionOverlayView(
            frame: CGRect(origin: .zero, size: contentRect.size)
        )
        super.init(
            contentRect: contentRect,
            styleMask: style,
            backing: backingStoreType,
            defer: flag
        )
        backgroundColor = .clear
        isOpaque = false
        hasShadow = false
        level = .screenSaver
        collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary]
        acceptsMouseMovedEvents = true
        contentView = selectionView
    }

    override var canBecomeKey: Bool { true }
    override var canBecomeMain: Bool { true }
}

@MainActor
private final class SelectionOverlayView: NSView, NSTextViewDelegate {
    private enum DragMode {
        case new
        case move
        case resize(ResizeHandle)
    }

    private enum ResizeHandle: CaseIterable {
        case topLeft, top, topRight, right, bottomRight, bottom, bottomLeft, left
    }

    private let minimumSelectionSize: CGFloat = 24
    private let maximumNoteCharacters = 20_000
    private let handleSize: CGFloat = 9
    private let confirmationMinimumWidth: CGFloat = 320
    private let confirmationHeight: CGFloat = 124
    private let confirmationPadding: CGFloat = 12
    private let selectionGap: CGFloat = 4
    private let controlGap: CGFloat = 8
    private let buttonSize: CGFloat = 44
    private var anchorPoint: CGPoint = .zero
    private var startingRect: CGRect = .zero
    private var dragMode: DragMode = .new
    private var isScrolling = false
    private(set) var selectionRect: CGRect = .zero
    private let toolbar = ScreenshotConfirmationView()
    private let noteLabel = NSTextField(labelWithString: "我的想法")
    private let noteScrollView = NSScrollView()
    private let noteTextView = ScreenshotNoteTextView(frame: .zero)
    private let cancelButton = NSButton(title: "", target: nil, action: nil)
    private let scrollingButton = NSButton(title: "", target: nil, action: nil)
    private let saveButton = NSButton(title: "", target: nil, action: nil)

    var onBeginSelection: (() -> Void)?
    var onNoteChanged: ((String) -> Void)?
    var onCancel: (() -> Void)?
    var onSave: ((CGRect, String) -> Void)?
    var onStartScrolling: ((CGRect) -> Void)?
    /// Fed with the user's downward scroll intent (points) while a normalized
    /// scrolling capture is running; the overlay swallows the real events.
    var onScrollIntent: ((CGFloat) -> Void)?
    /// A click landed on the shield during a scrolling capture: the session
    /// uses it to reactivate and bring the control panel back to the front.
    var onScrollingShieldClick: (() -> Void)?

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        wantsLayer = true
        configureToolbar()
    }

    required init?(coder: NSCoder) {
        nil
    }

    override var acceptsFirstResponder: Bool { true }

    override func acceptsFirstMouse(for event: NSEvent?) -> Bool {
        true
    }

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        window?.makeFirstResponder(self)
    }

    override func draw(_ dirtyRect: NSRect) {
        super.draw(dirtyRect)
        NSColor.clear.setFill()
        bounds.fill(using: .copy)
        let mask = NSBezierPath(rect: bounds)
        if !selectionRect.isEmpty {
            mask.appendRect(selectionRect)
            mask.windingRule = .evenOdd
        }
        NSColor(calibratedRed: 0.08, green: 0.14, blue: 0.1, alpha: 0.56).setFill()
        mask.fill()

        guard !selectionRect.isEmpty else { return }
        let accent = NSColor(calibratedRed: 0.18, green: 0.52, blue: 0.31, alpha: 1)
        accent.setStroke()
        let border = NSBezierPath(rect: selectionRect.insetBy(dx: 0.5, dy: 0.5))
        border.lineWidth = 2
        border.stroke()
        if !isScrolling {
            for rect in handleRects().values {
                NSColor.white.setFill()
                accent.setStroke()
                let handle = NSBezierPath(ovalIn: rect)
                handle.lineWidth = 2
                handle.fill()
                handle.stroke()
            }
        }
    }

    override func mouseDown(with event: NSEvent) {
        // While a scrolling capture runs the overlay shields the page from
        // clicks, so an accidental click cannot follow a link mid-capture.
        // The click still reactivates the session: after a system prompt or
        // another app took focus, this is the way back to Esc and the panel.
        guard !isScrolling else {
            onScrollingShieldClick?()
            return
        }
        window?.makeKey()
        window?.makeFirstResponder(self)
        let point = convert(event.locationInWindow, from: nil)
        onBeginSelection?()
        anchorPoint = point
        startingRect = selectionRect
        toolbar.isHidden = true

        if let handle = handle(at: point) {
            dragMode = .resize(handle)
        } else if selectionRect.contains(point), !selectionRect.isEmpty {
            dragMode = .move
        } else {
            dragMode = .new
            selectionRect = CGRect(origin: point, size: .zero)
        }
        needsDisplay = true
    }

    override func mouseDragged(with event: NSEvent) {
        guard !isScrolling else { return }
        let point = constrained(convert(event.locationInWindow, from: nil))
        switch dragMode {
        case .new:
            selectionRect = normalizedRect(from: anchorPoint, to: point).intersection(bounds)
        case .move:
            let offset = CGPoint(x: point.x - anchorPoint.x, y: point.y - anchorPoint.y)
            selectionRect = movedRect(startingRect, by: offset, inside: bounds)
        case let .resize(handle):
            selectionRect = resizedRect(startingRect, handle: handle, to: point)
        }
        needsDisplay = true
    }

    override func mouseUp(with event: NSEvent) {
        guard !isScrolling else { return }
        mouseDragged(with: event)
        guard
            selectionRect.width >= minimumSelectionSize,
            selectionRect.height >= minimumSelectionSize
        else {
            resetSelection()
            return
        }
        positionToolbar()
        toolbar.isHidden = false
        window?.makeFirstResponder(noteTextView)
        needsDisplay = true
    }

    override func keyDown(with event: NSEvent) {
        guard !isScrolling else {
            if event.keyCode == 53 {
                onCancel?()
            }
            return
        }
        switch event.keyCode {
        case 36, 76:
            saveSelection()
        case 53:
            onCancel?()
        default:
            super.keyDown(with: event)
        }
    }

    override func scrollWheel(with event: NSEvent) {
        guard isScrolling, let onScrollIntent else {
            super.scrollWheel(with: event)
            return
        }
        let rawDelta = event.hasPreciseScrollingDeltas
            ? event.scrollingDeltaY
            : event.scrollingDeltaY * 10
        // Positive means the user wants to move down the page, whatever the
        // device's natural-scrolling convention is.
        let downwardPoints = event.isDirectionInvertedFromDevice ? rawDelta : -rawDelta
        onScrollIntent(downwardPoints)
    }

    func resetSelection() {
        guard !isScrolling else { return }
        selectionRect = .zero
        toolbar.isHidden = true
        needsDisplay = true
    }

    func enterScrollingMode() {
        isScrolling = true
        toolbar.isHidden = true
        needsDisplay = true
    }

    func setNote(_ note: String) {
        guard noteTextView.string != note else { return }
        noteTextView.string = String(note.prefix(maximumNoteCharacters))
    }

    private func configureToolbar() {
        toolbar.frame.size = CGSize(
            width: confirmationMinimumWidth,
            height: confirmationHeight
        )
        toolbar.wantsLayer = true
        toolbar.layer?.backgroundColor = NSColor.white.cgColor
        toolbar.layer?.cornerRadius = 8
        toolbar.shadow = NSShadow()
        toolbar.shadow?.shadowColor = NSColor.black.withAlphaComponent(0.22)
        toolbar.shadow?.shadowBlurRadius = 16
        toolbar.shadow?.shadowOffset = CGSize(width: 0, height: -4)
        toolbar.isHidden = true

        noteLabel.font = .systemFont(ofSize: 13, weight: .medium)
        noteLabel.textColor = NSColor(calibratedRed: 0.15, green: 0.19, blue: 0.16, alpha: 1)
        noteLabel.setAccessibilityLabel("我的想法")

        noteScrollView.borderType = .noBorder
        noteScrollView.drawsBackground = false
        noteScrollView.hasVerticalScroller = true
        noteScrollView.autohidesScrollers = true
        noteScrollView.wantsLayer = true
        noteScrollView.layer?.backgroundColor = NSColor.textBackgroundColor.cgColor
        noteScrollView.layer?.borderColor = NSColor.separatorColor.cgColor
        noteScrollView.layer?.borderWidth = 1
        noteScrollView.layer?.cornerRadius = 6

        noteTextView.delegate = self
        noteTextView.isRichText = false
        noteTextView.importsGraphics = false
        noteTextView.allowsUndo = true
        noteTextView.isAutomaticQuoteSubstitutionEnabled = false
        noteTextView.isAutomaticDashSubstitutionEnabled = false
        noteTextView.font = .systemFont(ofSize: 13)
        noteTextView.textColor = .labelColor
        noteTextView.backgroundColor = .textBackgroundColor
        noteTextView.textContainerInset = CGSize(width: 8, height: 6)
        noteTextView.isVerticallyResizable = true
        noteTextView.isHorizontallyResizable = false
        noteTextView.autoresizingMask = [.width]
        noteTextView.textContainer?.widthTracksTextView = true
        noteTextView.setAccessibilityLabel("我的想法")
        noteTextView.setAccessibilityHelp("可输入文字或按住 Fn 使用豆包语音输入")
        noteTextView.onCancel = { [weak self] in
            self?.onCancel?()
        }
        noteTextView.onSave = { [weak self] in
            self?.saveSelection()
        }
        noteScrollView.documentView = noteTextView

        let symbolConfiguration = NSImage.SymbolConfiguration(pointSize: 15, weight: .semibold)
        cancelButton.image = NSImage(
            systemSymbolName: "xmark",
            accessibilityDescription: "取消"
        )?.withSymbolConfiguration(symbolConfiguration)
        cancelButton.imagePosition = .imageOnly
        cancelButton.imageScaling = .scaleProportionallyDown
        cancelButton.bezelStyle = .rounded
        cancelButton.toolTip = "取消（Esc）"
        cancelButton.setAccessibilityLabel("取消截图")
        cancelButton.setAccessibilityHelp("按 Esc 也可以取消")
        cancelButton.target = self
        cancelButton.action = #selector(cancelSelection)

        if ScreenshotFeatureFlags.scrollingCaptureEnabled {
            scrollingButton.image = NSImage(
                systemSymbolName: "rectangle.stack",
                accessibilityDescription: "滚动截图"
            )?.withSymbolConfiguration(symbolConfiguration)
            scrollingButton.imagePosition = .imageOnly
            scrollingButton.imageScaling = .scaleProportionallyDown
            scrollingButton.bezelStyle = .rounded
            scrollingButton.contentTintColor = NSColor(
                calibratedRed: 0.12,
                green: 0.32,
                blue: 0.19,
                alpha: 1
            )
            scrollingButton.toolTip = "进入滚动截图"
            scrollingButton.setAccessibilityLabel("进入滚动截图")
            scrollingButton.setAccessibilityHelp("锁定选区并开始连续采集")
            scrollingButton.target = self
            scrollingButton.action = #selector(startScrollingSelection)
        }

        saveButton.image = NSImage(
            systemSymbolName: "checkmark",
            accessibilityDescription: "保存"
        )?.withSymbolConfiguration(symbolConfiguration.applying(
            NSImage.SymbolConfiguration(paletteColors: [.white])
        ))
        saveButton.image?.isTemplate = false
        saveButton.imagePosition = .imageOnly
        saveButton.imageScaling = .scaleProportionallyDown
        saveButton.bezelStyle = .rounded
        saveButton.bezelColor = NSColor(calibratedRed: 0.12, green: 0.32, blue: 0.19, alpha: 1)
        saveButton.contentTintColor = .white
        saveButton.toolTip = "保存截图（⌘↩）"
        saveButton.setAccessibilityLabel("保存普通截图")
        saveButton.setAccessibilityHelp("按 Command + Enter 也可以保存普通截图")
        saveButton.target = self
        saveButton.action = #selector(saveSelection)

        toolbar.addSubview(noteLabel)
        toolbar.addSubview(noteScrollView)
        toolbar.addSubview(cancelButton)
        if ScreenshotFeatureFlags.scrollingCaptureEnabled {
            toolbar.addSubview(scrollingButton)
        }
        toolbar.addSubview(saveButton)
        addSubview(toolbar)
    }

    private func positionToolbar() {
        let edgeInset: CGFloat = 8
        let maximumWidth = max(confirmationMinimumWidth, bounds.width - edgeInset * 2)
        let width = min(
            max(selectionRect.width, confirmationMinimumWidth),
            maximumWidth
        )
        let toolbarSize = CGSize(width: width, height: confirmationHeight)
        toolbar.frame.size = toolbarSize
        layoutToolbarContent(width: width)
        var origin = CGPoint(
            x: selectionRect.maxX - toolbarSize.width,
            y: selectionRect.minY - toolbarSize.height - selectionGap
        )
        if origin.y < edgeInset {
            let insideBottom = selectionRect.minY + selectionGap
            if insideBottom + toolbarSize.height <= selectionRect.maxY - selectionGap {
                origin.y = insideBottom
            } else {
                origin.y = selectionRect.maxY + selectionGap
            }
        }
        origin.x = min(max(origin.x, edgeInset), bounds.maxX - toolbarSize.width - edgeInset)
        origin.y = min(max(origin.y, edgeInset), bounds.maxY - toolbarSize.height - edgeInset)
        toolbar.setFrameOrigin(origin)
    }

    private func layoutToolbarContent(width: CGFloat) {
        let buttonCount: CGFloat = ScreenshotFeatureFlags.scrollingCaptureEnabled ? 3 : 2
        let buttonsWidth = buttonCount * buttonSize + (buttonCount - 1) * controlGap
        let buttonsStartX = width - confirmationPadding - buttonsWidth
        noteLabel.frame = CGRect(
            x: confirmationPadding,
            y: 89,
            width: max(buttonsStartX - confirmationPadding - controlGap, 1),
            height: 18
        )
        noteScrollView.frame = CGRect(
            x: confirmationPadding,
            y: 12,
            width: width - confirmationPadding * 2,
            height: 58
        )
        noteTextView.frame = CGRect(
            origin: .zero,
            size: noteScrollView.contentSize
        )

        cancelButton.frame = CGRect(
            x: buttonsStartX,
            y: 76,
            width: buttonSize,
            height: buttonSize
        )
        if ScreenshotFeatureFlags.scrollingCaptureEnabled {
            scrollingButton.frame = CGRect(
                x: buttonsStartX + buttonSize + controlGap,
                y: 76,
                width: buttonSize,
                height: buttonSize
            )
        }
        let saveIndex: CGFloat = ScreenshotFeatureFlags.scrollingCaptureEnabled ? 2 : 1
        saveButton.frame = CGRect(
            x: buttonsStartX + (buttonSize + controlGap) * saveIndex,
            y: 76,
            width: buttonSize,
            height: buttonSize
        )
    }

    func textView(
        _ textView: NSTextView,
        shouldChangeTextIn affectedCharRange: NSRange,
        replacementString: String?
    ) -> Bool {
        let replacement = replacementString ?? ""
        let updated = (textView.string as NSString).replacingCharacters(
            in: affectedCharRange,
            with: replacement
        )
        return updated.count <= maximumNoteCharacters
    }

    func textDidChange(_ notification: Notification) {
        guard notification.object as? NSTextView === noteTextView else { return }
        onNoteChanged?(noteTextView.string)
    }

    @objc private func cancelSelection() {
        onCancel?()
    }

    @objc private func saveSelection() {
        guard !selectionRect.isEmpty else { return }
        onSave?(selectionRect, noteTextView.string)
    }

    @objc private func startScrollingSelection() {
        guard ScreenshotFeatureFlags.scrollingCaptureEnabled, !selectionRect.isEmpty else {
            return
        }
        onStartScrolling?(selectionRect)
    }

    private func constrained(_ point: CGPoint) -> CGPoint {
        CGPoint(
            x: min(max(point.x, bounds.minX), bounds.maxX),
            y: min(max(point.y, bounds.minY), bounds.maxY)
        )
    }

    private func normalizedRect(from start: CGPoint, to end: CGPoint) -> CGRect {
        CGRect(
            x: min(start.x, end.x),
            y: min(start.y, end.y),
            width: abs(end.x - start.x),
            height: abs(end.y - start.y)
        )
    }

    private func movedRect(_ rect: CGRect, by offset: CGPoint, inside container: CGRect) -> CGRect {
        var moved = rect.offsetBy(dx: offset.x, dy: offset.y)
        moved.origin.x = min(max(moved.minX, container.minX), container.maxX - moved.width)
        moved.origin.y = min(max(moved.minY, container.minY), container.maxY - moved.height)
        return moved
    }

    private func resizedRect(_ rect: CGRect, handle: ResizeHandle, to point: CGPoint) -> CGRect {
        var left = rect.minX
        var right = rect.maxX
        var bottom = rect.minY
        var top = rect.maxY
        if [.topLeft, .left, .bottomLeft].contains(handle) { left = point.x }
        if [.topRight, .right, .bottomRight].contains(handle) { right = point.x }
        if [.bottomLeft, .bottom, .bottomRight].contains(handle) { bottom = point.y }
        if [.topLeft, .top, .topRight].contains(handle) { top = point.y }
        if right - left < minimumSelectionSize {
            if [.topLeft, .left, .bottomLeft].contains(handle) {
                left = right - minimumSelectionSize
            } else {
                right = left + minimumSelectionSize
            }
        }
        if top - bottom < minimumSelectionSize {
            if [.bottomLeft, .bottom, .bottomRight].contains(handle) {
                bottom = top - minimumSelectionSize
            } else {
                top = bottom + minimumSelectionSize
            }
        }
        return CGRect(x: left, y: bottom, width: right - left, height: top - bottom)
            .standardized
            .intersection(bounds)
    }

    private func handle(at point: CGPoint) -> ResizeHandle? {
        handleRects().first(where: { $0.value.insetBy(dx: -4, dy: -4).contains(point) })?.key
    }

    private func handleRects() -> [ResizeHandle: CGRect] {
        guard !selectionRect.isEmpty else { return [:] }
        let centers: [ResizeHandle: CGPoint] = [
            .topLeft: CGPoint(x: selectionRect.minX, y: selectionRect.maxY),
            .top: CGPoint(x: selectionRect.midX, y: selectionRect.maxY),
            .topRight: CGPoint(x: selectionRect.maxX, y: selectionRect.maxY),
            .right: CGPoint(x: selectionRect.maxX, y: selectionRect.midY),
            .bottomRight: CGPoint(x: selectionRect.maxX, y: selectionRect.minY),
            .bottom: CGPoint(x: selectionRect.midX, y: selectionRect.minY),
            .bottomLeft: CGPoint(x: selectionRect.minX, y: selectionRect.minY),
            .left: CGPoint(x: selectionRect.minX, y: selectionRect.midY),
        ]
        return centers.mapValues { center in
            CGRect(
                x: center.x - handleSize / 2,
                y: center.y - handleSize / 2,
                width: handleSize,
                height: handleSize
            )
        }
    }
}

@MainActor
private final class ScreenshotConfirmationView: NSView {
    override func mouseDown(with event: NSEvent) {}
    override func mouseDragged(with event: NSEvent) {}
    override func mouseUp(with event: NSEvent) {}
}

@MainActor
private final class ScreenshotNoteTextView: NSTextView {
    var onCancel: (() -> Void)?
    var onSave: (() -> Void)?

    override func keyDown(with event: NSEvent) {
        if event.keyCode == 53 {
            onCancel?()
            return
        }
        let modifiers = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
        if [36, 76].contains(event.keyCode), modifiers.contains(.command) {
            onSave?()
            return
        }
        super.keyDown(with: event)
    }
}

@MainActor
private final class CaptureControlPanel: NSPanel {
    private let onCancel: () -> Void
    private let onSave: () -> Void
    private var cancelButton: NSButton!
    private var saveButton: NSButton!
    private var isProcessing = false

    init(
        screen: NSScreen,
        selectionRect: CGRect,
        onCancel: @escaping () -> Void,
        onSave: @escaping () -> Void
    ) {
        self.onCancel = onCancel
        self.onSave = onSave
        let size = CGSize(width: 104, height: 52)
        let gap: CGFloat = 8
        let edgeInset: CGFloat = 8
        var origin = CGPoint(
            x: screen.frame.minX + selectionRect.maxX - size.width,
            y: screen.frame.minY + selectionRect.minY - size.height - gap
        )
        if origin.y < screen.frame.minY + edgeInset {
            origin.y = screen.frame.minY + selectionRect.minY + gap
        }
        origin.x = min(
            max(origin.x, screen.frame.minX + edgeInset),
            screen.frame.maxX - size.width - edgeInset
        )
        origin.y = min(
            max(origin.y, screen.frame.minY + edgeInset),
            screen.frame.maxY - size.height - edgeInset
        )
        super.init(
            contentRect: CGRect(origin: origin, size: size),
            styleMask: [.borderless],
            backing: .buffered,
            defer: false
        )
        isOpaque = false
        backgroundColor = .clear
        hasShadow = true
        level = .screenSaver
        collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary]

        let content = NSView(frame: CGRect(origin: .zero, size: size))
        content.wantsLayer = true
        content.layer?.backgroundColor = NSColor.white.cgColor
        content.layer?.cornerRadius = 8
        cancelButton = makeButton(
            symbol: "xmark",
            accessibilityLabel: "取消滚动截图",
            frame: CGRect(x: 4, y: 4, width: 44, height: 44),
            action: #selector(cancelCapture),
            isPrimary: false
        )
        saveButton = makeButton(
            symbol: "checkmark",
            accessibilityLabel: "保存滚动截图",
            frame: CGRect(x: 56, y: 4, width: 44, height: 44),
            action: #selector(saveCapture),
            isPrimary: true
        )
        content.addSubview(cancelButton)
        content.addSubview(saveButton)
        contentView = content
    }

    override var canBecomeKey: Bool { true }

    override func keyDown(with event: NSEvent) {
        guard !isProcessing else { return }
        switch event.keyCode {
        case 36, 76:
            onSave()
        case 53:
            onCancel()
        default:
            super.keyDown(with: event)
        }
    }

    func setProcessing() {
        guard !isProcessing else { return }
        isProcessing = true
        cancelButton.isEnabled = false
        saveButton.isEnabled = false
        saveButton.image = NSImage(
            systemSymbolName: "ellipsis",
            accessibilityDescription: "正在生成并校验长图"
        )?.withSymbolConfiguration(
            NSImage.SymbolConfiguration(pointSize: 15, weight: .semibold)
        )
        saveButton.setAccessibilityLabel("正在生成并校验长图")
    }

    private func makeButton(
        symbol: String,
        accessibilityLabel: String,
        frame: CGRect,
        action: Selector,
        isPrimary: Bool
    ) -> NSButton {
        let button = NSButton(frame: frame)
        let configuration = NSImage.SymbolConfiguration(pointSize: 15, weight: .semibold)
        button.image = NSImage(
            systemSymbolName: symbol,
            accessibilityDescription: accessibilityLabel
        )?.withSymbolConfiguration(configuration)
        button.imagePosition = .imageOnly
        button.imageScaling = .scaleProportionallyDown
        button.bezelStyle = .rounded
        button.target = self
        button.action = action
        button.setAccessibilityLabel(accessibilityLabel)
        if isPrimary {
            button.bezelColor = NSColor(calibratedRed: 0.12, green: 0.32, blue: 0.19, alpha: 1)
            button.contentTintColor = .white
        }
        return button
    }

    @objc private func cancelCapture() {
        guard !isProcessing else { return }
        onCancel()
    }

    @objc private func saveCapture() {
        guard !isProcessing else { return }
        onSave()
    }
}

@MainActor
private final class CaptureHintPanel: NSPanel {
    init(message: String, beside panel: NSWindow) {
        let label = NSTextField(labelWithString: message)
        label.font = .systemFont(ofSize: 13, weight: .medium)
        label.textColor = .white
        label.alignment = .center
        label.sizeToFit()
        let size = CGSize(width: label.frame.width + 28, height: 36)
        let screen = panel.screen ?? NSScreen.main
        var origin = CGPoint(
            x: panel.frame.maxX - size.width,
            y: panel.frame.maxY + 8
        )
        if let screen {
            origin.x = min(max(origin.x, screen.frame.minX + 8), screen.frame.maxX - size.width - 8)
            origin.y = min(max(origin.y, screen.frame.minY + 8), screen.frame.maxY - size.height - 8)
        }
        super.init(
            contentRect: CGRect(origin: origin, size: size),
            styleMask: [.borderless, .nonactivatingPanel],
            backing: .buffered,
            defer: false
        )
        isOpaque = false
        backgroundColor = .clear
        hasShadow = true
        level = .screenSaver
        ignoresMouseEvents = true
        collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary]

        let content = NSView(frame: CGRect(origin: .zero, size: size))
        content.wantsLayer = true
        content.layer?.backgroundColor = NSColor.black.withAlphaComponent(0.78).cgColor
        content.layer?.cornerRadius = 8
        label.frame = CGRect(x: 14, y: 9, width: label.frame.width, height: 18)
        content.addSubview(label)
        contentView = content
    }
}
