import AppKit

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate {
    private var window: NSWindow?

    func applicationDidFinishLaunching(_ notification: Notification) {
        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 620, height: 430),
            styleMask: [.titled, .closable, .miniaturizable, .resizable],
            backing: .buffered,
            defer: false
        )
        window.title = "IdeaCard 原生语音输入测试"
        window.center()
        window.minSize = NSSize(width: 520, height: 360)

        let stack = NSStackView()
        stack.orientation = .vertical
        stack.alignment = .leading
        stack.spacing = 12
        stack.translatesAutoresizingMaskIntoConstraints = false

        let title = NSTextField(labelWithString: "豆包语音输入兼容性测试")
        title.font = .systemFont(ofSize: 22, weight: .semibold)

        let instruction = NSTextField(
            wrappingLabelWithString: "先点击下面任一输入框，再按住 Fn 说话。这里只使用 macOS 原生输入控件，不包含网页或 Tauri。"
        )
        instruction.font = .systemFont(ofSize: 14)
        instruction.textColor = .secondaryLabelColor

        let singleLabel = NSTextField(labelWithString: "单行输入")
        singleLabel.font = .systemFont(ofSize: 13, weight: .medium)

        let singleField = NSTextField()
        singleField.placeholderString = "点击这里，按住 Fn"
        singleField.font = .systemFont(ofSize: 16)
        singleField.translatesAutoresizingMaskIntoConstraints = false
        singleField.widthAnchor.constraint(equalToConstant: 572).isActive = true
        singleField.heightAnchor.constraint(equalToConstant: 40).isActive = true

        let multiLabel = NSTextField(labelWithString: "多行输入")
        multiLabel.font = .systemFont(ofSize: 13, weight: .medium)

        let textView = NSTextView()
        textView.font = .systemFont(ofSize: 16)
        textView.isRichText = false
        textView.allowsUndo = true
        textView.textContainerInset = NSSize(width: 10, height: 10)

        let scrollView = NSScrollView()
        scrollView.borderType = .bezelBorder
        scrollView.hasVerticalScroller = true
        scrollView.documentView = textView
        scrollView.translatesAutoresizingMaskIntoConstraints = false
        scrollView.widthAnchor.constraint(equalToConstant: 572).isActive = true
        scrollView.heightAnchor.constraint(equalToConstant: 190).isActive = true

        [title, instruction, singleLabel, singleField, multiLabel, scrollView].forEach {
            stack.addArrangedSubview($0)
        }

        let contentView = NSView()
        contentView.addSubview(stack)
        NSLayoutConstraint.activate([
            stack.leadingAnchor.constraint(equalTo: contentView.leadingAnchor, constant: 24),
            stack.trailingAnchor.constraint(lessThanOrEqualTo: contentView.trailingAnchor, constant: -24),
            stack.topAnchor.constraint(equalTo: contentView.topAnchor, constant: 24),
            stack.bottomAnchor.constraint(lessThanOrEqualTo: contentView.bottomAnchor, constant: -24),
            instruction.widthAnchor.constraint(equalToConstant: 572)
        ])

        window.contentView = contentView
        window.makeKeyAndOrderFront(nil)
        window.makeFirstResponder(singleField)
        NSApp.activate(ignoringOtherApps: true)
        self.window = window
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        true
    }
}

@main
struct NativeInputDiagnosticApp {
    @MainActor
    static func main() {
        let application = NSApplication.shared
        let delegate = AppDelegate()
        application.delegate = delegate
        application.setActivationPolicy(.regular)
        application.run()
    }
}
