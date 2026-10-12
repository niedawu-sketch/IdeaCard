# IdeaCard Mac 捕获能力实施计划

> **供智能执行者使用：** 必须使用 `superpowers:subagent-driven-development`（推荐）或 `superpowers:executing-plans`，逐项执行本计划。步骤使用复选框（`- [ ]`）跟踪。

**目标：** 在桌面基础上实现菜单栏、固定全局快捷键、剪贴板、文件导入、区域截图和本地 Vision OCR 的真实捕获流程。

**架构：** Rust `CaptureService` 统一生成捕获草稿和卡片。SwiftPM 构建的 `ideacard-native` sidecar 通过标准输入输出交换单个 JSON 请求，负责 OCR、截图和权限状态。

**技术栈：** Tauri plugins、Rust、arboard、url、zip、quick-xml、pdf-extract、SwiftPM、Vision、AppKit、Vitest、Rust tests、Swift tests

---

## 文件结构

```text
desktop/
├── scripts/build-native.sh
├── src/entities/capture/model.ts
├── src/features/capture/CaptureDialog.tsx
├── src/features/capture/FileImport.tsx
├── src-tauri/native/Package.swift
├── src-tauri/native/Sources/IdeaCardNative/
│   ├── main.swift
│   ├── NativeProtocol.swift
│   ├── OCRService.swift
│   └── ScreenshotService.swift
├── src-tauri/native/Tests/IdeaCardNativeTests/
├── src-tauri/src/capture/
├── src-tauri/src/native/
└── src-tauri/src/menu.rs
```

### 任务 1: 定义捕获草稿、文件类型和限制

**文件：**
- 创建： `desktop/src/entities/capture/model.ts`
- 创建： `desktop/src/entities/capture/model.test.ts`
- 创建： `desktop/src-tauri/src/capture/mod.rs`
- 创建： `desktop/src-tauri/src/capture/file_kind.rs`

- [x] **步骤 1：编写前端分类失败测试**

```ts
import { describe, expect, it } from 'vitest'
import { captureSourceLabel, canSaveCaptureDraft } from './model'

describe('capture model', () => {
  it('requires content unless an attachment exists', () => {
    expect(canSaveCaptureDraft({ content: '   ', attachmentCount: 0 })).toBe(false)
    expect(canSaveCaptureDraft({ content: '   ', attachmentCount: 1 })).toBe(true)
  })

  it('labels automatic source classification', () => {
    expect(captureSourceLabel('web')).toBe('网页文章')
    expect(captureSourceLabel('image')).toBe('图片材料')
  })
})
```

- [x] **步骤 2：运行并确认失败**

运行： `npm test -- src/entities/capture/model.test.ts`

预期：失败，因为 `model.ts` 尚不存在。

- [x] **步骤 3：实现前端捕获模型**

定义 `CaptureKind = 'quick' | 'clipboard' | 'screenshot' | 'file'`、包含标题/内容/笔记/来源/临时附件 ID 的 `CaptureDraft`、`CaptureState = 'loading' | 'ready' | 'saving' | 'failed'`，以及步骤 1 中的两个纯函数。复用卡片模型的 `SourceType`。

- [x] **步骤 4：编写 Rust 文件分类失败测试**

```rust
#[test]
fn maps_supported_extensions_to_source_and_limits() {
    assert_eq!(classify_path(Path::new("report.pdf")).unwrap().source_type, SourceType::Document);
    assert_eq!(classify_path(Path::new("screen.heic")).unwrap().source_type, SourceType::Image);
    assert_eq!(classify_path(Path::new("meeting.mp4")).unwrap().max_bytes, 500 * 1024 * 1024);
}

#[test]
fn rejects_unknown_extension() {
    assert_eq!(classify_path(Path::new("archive.bin")).unwrap_err().code, "unsupported_file_type");
}
```

- [x] **步骤 5：实现 Rust 文件分类**

```rust
const DOCUMENTS: &[&str] = &["pdf", "docx", "pptx", "xlsx", "md", "markdown", "txt"];
const IMAGES: &[&str] = &["png", "jpg", "jpeg", "heic"];
const AUDIO: &[&str] = &["mp3", "m4a", "wav", "aac", "flac"];
const VIDEO: &[&str] = &["mp4", "mov", "m4v", "webm"];
const DOCUMENT_LIMIT: u64 = 50 * 1024 * 1024;
const MEDIA_LIMIT: u64 = 500 * 1024 * 1024;
const MAX_BATCH: usize = 20;
```

返回来源类型、附件种类、MIME 类型、提取策略和字节上限。扩展名统一按 ASCII 小写比较，并拒绝没有文件名的路径。

- [x] **步骤 6：验证并提交**

运行： `npm test -- src/entities/capture/model.test.ts && npm run test:rust -- capture::file_kind::tests`

预期：通过。

```bash
git add desktop/src/entities/capture desktop/src-tauri/src/capture
git commit -m "feat: define real capture inputs"
```

### 任务 2: 建立 Swift sidecar 协议和构建

**文件：**
- 创建： `desktop/src-tauri/native/Package.swift`
- 创建： `desktop/src-tauri/native/Sources/IdeaCardNative/main.swift`
- 创建： `desktop/src-tauri/native/Sources/IdeaCardNative/NativeProtocol.swift`
- 创建： `desktop/src-tauri/native/Tests/IdeaCardNativeTests/NativeProtocolTests.swift`
- 创建： `desktop/scripts/build-native.sh`
- 创建： `desktop/scripts/test-native.sh`
- 修改： `desktop/package.json`
- 修改： `desktop/src-tauri/tauri.conf.json`

- [x] **步骤 1：创建 SwiftPM 包和协议失败测试**

`Package.swift` declares macOS 14, executable product `ideacard-native`, target `IdeaCardNative`, and test target `IdeaCardNativeTests`.

```swift
func testOCRRequestRoundTrip() throws {
    let request = NativeRequest(command: .ocr, path: "/tmp/input.png", service: nil, account: nil, secret: nil)
    let data = try JSONEncoder().encode(request)
    let decoded = try JSONDecoder().decode(NativeRequest.self, from: data)
    XCTAssertEqual(decoded.command, .ocr)
    XCTAssertEqual(decoded.path, "/tmp/input.png")
}
```

- [x] **步骤 2：运行并确认失败**

运行： `swift test --package-path src-tauri/native`

预期：失败，因为协议类型尚不存在。

- [x] **步骤 3：实现单请求 JSON 协议**

```swift
enum NativeCommand: String, Codable { case ocr, screenshot, screenPermission, keychainSet, keychainGet, keychainDelete }
struct NativeRequest: Codable { let command: NativeCommand; let path: String?; let service: String?; let account: String?; let secret: String? }
struct NativeResponse: Codable { let ok: Bool; let text: String?; let path: String?; let permission: String?; let errorCode: String?; let message: String? }
```

`main.swift` 读取全部标准输入，只解码并分发一个请求，再向标准输出写入一个 JSON 对象；只有标准输出无法写入时才以非零状态退出。诊断用标准错误只包含错误码，绝不包含请求 JSON 或密钥。

- [x] **步骤 4：建立可重复构建脚本**

创建可执行脚本 `scripts/build-native.sh`。同时创建 `scripts/test-native.sh`，从当前 `xcode-select` 开发者目录向 SwiftPM 显式传入 Swift Testing 框架和运行库路径，确保仅安装 Command Line Tools 时也会真正发现并执行测试：

```bash
#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PACKAGE="$ROOT/src-tauri/native"
DESTINATION="$ROOT/src-tauri/binaries/ideacard-native-aarch64-apple-darwin"
swift build --package-path "$PACKAGE" -c release
BIN_PATH="$(swift build --package-path "$PACKAGE" -c release --show-bin-path)"
mkdir -p "$(dirname "$DESTINATION")"
cp "$BIN_PATH/ideacard-native" "$DESTINATION"
chmod 755 "$DESTINATION"
```

增加 `build:native` 和 `test:native` 脚本；让 Tauri 的开发及构建前置步骤在 Vite 之前运行 `npm run build:native`。在 bundle 配置中加入 `"externalBin": ["binaries/ideacard-native"]`。

- [x] **步骤 5：验证并提交协议**

运行： `npm run test:native && npm run build:native`

预期：Swift 测试通过，arm64 sidecar 存在且可执行。

```bash
git add desktop/src-tauri/native desktop/scripts desktop/package.json desktop/src-tauri/tauri.conf.json
git commit -m "feat: add macOS native helper protocol"
```

### 任务 3: 实现 Vision OCR

**文件：**
- 创建： `desktop/src-tauri/native/Sources/IdeaCardNative/OCRService.swift`
- 创建： `desktop/src-tauri/native/Tests/IdeaCardNativeTests/OCRServiceTests.swift`

- [x] **步骤 1：编写固定图像 OCR 失败测试**

生成 1000x300 的 `NSImage`，使用系统字体绘制 `IdeaCard 测试 2026`，把 PNG 写入临时 URL，然后断言：

```swift
let text = try OCRService().recognizeText(at: imageURL)
XCTAssertTrue(text.contains("IdeaCard"))
XCTAssertTrue(text.contains("2026"))
```

另测文件缺失时返回 `native_file_missing`，空白图片返回空字符串。

- [x] **步骤 2：运行并确认失败**

运行： `npm run test:native -- --filter OCRServiceTests`

预期：失败，因为 `OCRService` 尚不存在。

- [x] **步骤 3：实现 OCR**

使用 `VNRecognizeTextRequest`、`.accurate`、语言纠正、识别语言 `zh-Hans` 与 `en-US`，并通过 `VNImageRequestHandler(url:)` 执行。按边界框先从上到下、再从左到右排序，以换行连接首选结果。把文件缺失、不支持的图片、Vision 失败和空结果分别映射为独立 `NativeServiceError` 错误码。

- [x] **步骤 4：接入协议并验证**

只有存在 `path` 时才分发 `.ocr`。返回包含 `text` 的 `ok: true`；未识别到文字时 `text` 为空字符串。

运行： `npm run test:native`

预期：全部 Swift 测试通过。

- [x] **步骤 5：提交 OCR**

```bash
git add desktop/src-tauri/native
git commit -m "feat: recognize captured text with Vision"
```

### 任务 4: 实现区域截图与权限状态

**文件：**
- 创建： `desktop/src-tauri/native/Sources/IdeaCardNative/ScreenshotService.swift`
- 创建： `desktop/src-tauri/native/Tests/IdeaCardNativeTests/ScreenshotServiceTests.swift`

- [x] **步骤 1：编写进程注入失败测试**

定义 `ProcessRunning` 协议。假执行器验证截图参数严格为 `-i`、`-x` 和输出路径。退出码为 0 且文件存在时返回成功；退出码为 0 但文件不存在时返回取消；非 0 时返回 `screenshot_failed`。

- [x] **步骤 2：运行并确认失败**

运行： `npm run test:native -- --filter ScreenshotServiceTests`

预期：失败，因为 `ScreenshotService` 尚不存在。

- [x] **步骤 3：实现截图和权限**

用 `CGPreflightScreenCaptureAccess()` 读取状态，只在实际截图时调用 `CGRequestScreenCaptureAccess()`。直接启动 `/usr/sbin/screencapture`，参数固定为 `-i -x` 加输出路径；等待结束后，根据退出码和文件是否存在区分成功、取消与失败。不得经过 shell。

- [x] **步骤 4：接入协议并验证**

`.screenPermission` returns `granted` or `not_granted`. `.screenshot` requires an absolute cache path from Rust and returns path, cancelled, or permission error.

运行： `npm run test:native`

预期：通过，测试期间不打开交互式截图界面。

- [x] **步骤 5：提交截图能力**

```bash
git add desktop/src-tauri/native
git commit -m "feat: capture screen regions on macOS"
```

### 任务 5: 建立 Rust sidecar 客户端

**文件：**
- 创建： `desktop/src-tauri/src/native/mod.rs`
- 创建： `desktop/src-tauri/src/native/client.rs`
- 修改： `desktop/src-tauri/Cargo.toml`
- 修改： `desktop/src-tauri/src/lib.rs`

- [x] **步骤 1：添加仅 Rust 可用的 shell 插件**

运行： `cd desktop/src-tauri && cargo add tauri-plugin-shell@2`

在 `lib.rs` 中初始化插件；不得向任何 WebView capability 添加 shell 权限。

- [x] **步骤 2：编写协议客户端失败测试**

```rust
#[async_trait]
pub trait NativeTransport: Send + Sync {
    async fn request(&self, request: NativeRequest) -> Result<NativeResponse, AppError>;
}
```

使用假传输层测试畸形 JSON、sidecar 退出、原生返回 `ok: false`、OCR 空字符串和截图取消的映射。

- [x] **步骤 3：运行并确认失败**

运行： `npm run test:rust -- native::client::tests`

预期：失败，因为客户端尚不存在。

- [x] **步骤 4：实现真实 transport**

使用 `app.shell().sidecar("ideacard-native")`，把序列化请求写入子进程标准输入并收集标准输出；只接受一个 JSON 响应，输出上限为 10 MB。把进程、JSON 和原生错误映射为稳定 `AppError` 错误码，绝不记录请求或响应正文。

- [x] **步骤 5：验证并提交**

运行： `npm run test:rust -- native::client::tests && npm run test:native`

预期：通过。

```bash
git add desktop/src-tauri/Cargo.toml desktop/src-tauri/Cargo.lock desktop/src-tauri/src/native desktop/src-tauri/src/lib.rs
git commit -m "feat: call the macOS native helper"
```

### 任务 6: 实现剪贴板捕获

**文件：**
- 创建： `desktop/src-tauri/src/capture/clipboard.rs`
- 创建： `desktop/src-tauri/src/capture/service.rs`
- 修改： `desktop/src-tauri/Cargo.toml`

- [x] **步骤 1：添加剪贴板和图片依赖**

运行：

```bash
cd desktop/src-tauri
cargo add arboard url image
```

- [x] **步骤 2：编写四类剪贴板失败测试**

使用假 `ClipboardReading` 并断言以下精确结果：

```rust
assert_eq!(service.read_text("临时问题")?.source_type, SourceType::Personal);
assert_eq!(service.read_text("https://example.com/article")?.source_uri, "https://example.com/article");
assert_eq!(service.read_text("https://example.com/article")?.source_name, "example.com");
assert_eq!(service.read_text("   ").unwrap_err().code, "clipboard_empty");
```

对于 RGBA 输入，断言已写入临时 PNG、来源类型为图片，并且假原生客户端返回的 OCR 文本成为草稿内容。

- [x] **步骤 3：运行并确认失败**

运行： `npm run test:rust -- capture::clipboard::tests`

预期：失败，因为剪贴板捕获尚不存在。

- [x] **步骤 4：实现剪贴板适配器**

存在图片字节时优先读取图片，否则读取文本。只有 HTTP/HTTPS 地址归类为网页来源。剪贴板图片以 UUID 文件名写入应用缓存并调用 OCR；创建草稿失败时移除缓存文件。

- [x] **步骤 5：验证并提交**

运行： `npm run test:rust -- capture::clipboard::tests`

预期：通过。

```bash
git add desktop/src-tauri/Cargo.toml desktop/src-tauri/Cargo.lock desktop/src-tauri/src/capture
git commit -m "feat: capture clipboard materials"
```

### 任务 7: 实现文档、Office 和媒体导入

**文件：**
- 创建： `desktop/src-tauri/src/capture/extractors/mod.rs`
- 创建： `desktop/src-tauri/src/capture/extractors/text.rs`
- 创建： `desktop/src-tauri/src/capture/extractors/pdf.rs`
- 创建： `desktop/src-tauri/src/capture/extractors/office.rs`
- 创建： `desktop/src-tauri/src/capture/import.rs`
- 修改： `desktop/src-tauri/Cargo.toml`

- [x] **步骤 1：添加解析依赖**

运行：

```bash
cd desktop/src-tauri
cargo add pdf-extract zip quick-xml encoding_rs
```

- [x] **步骤 2：编写提取器失败测试**

生成临时夹具并证明：

```rust
assert_eq!(extract_txt(&utf8_path)?.text, "第一行\n第二行");
assert!(extract_pdf(&pdf_path)?.text.contains("IdeaCard PDF"));
assert_eq!(extract_docx(&docx_path)?.text, "标题\n正文");
assert!(extract_pptx(&pptx_path)?.text.starts_with("## 幻灯片 1"));
assert!(extract_xlsx(&xlsx_path)?.text.contains("## 工作表：Sheet1"));
assert_eq!(extract_office(&corrupt_path).unwrap_err().code, "file_extraction_failed");
```

- [x] **步骤 3：运行并确认失败**

运行： `npm run test:rust -- capture::extractors::tests`

预期：失败，因为提取器尚不存在。

- [x] **步骤 4：实现提取器接口**

```rust
pub trait TextExtractor {
    fn extract(&self, path: &Path) -> Result<ExtractionResult, AppError>;
}

pub struct ExtractionResult {
    pub status: ExtractionStatus,
    pub text: String,
    pub error: Option<String>,
}
```

PDF 使用 `pdf_extract::extract_text`，Office XML 使用 `quick_xml::Reader`，绝不执行公式或宏。本地提取文本上限为 2000 万个 Unicode 字符，超限返回 `file_text_too_large`；提取失败时保留已复制附件。

- [x] **步骤 5：实现最多二十文件导入**

复制任何文件前先校验数量、扩展名和大小。每个文件先存附件、再提取文本，随后在同一事务中写入卡片和附件行。事务失败时只移除本次新复制的附件。每条路径分别返回成功或失败；单个损坏文件不回滚同批其他成功项。

- [x] **步骤 6：验证并提交**

运行： `npm run test:rust -- capture::extractors::tests capture::import::tests`

预期：通过。

```bash
git add desktop/src-tauri/Cargo.toml desktop/src-tauri/Cargo.lock desktop/src-tauri/src/capture
git commit -m "feat: import local document materials"
```

### 任务 8: 实现截图、菜单栏和全局快捷键

**文件：**
- 创建： `desktop/src-tauri/src/menu.rs`
- 创建： `desktop/src-tauri/src/capture/screenshot.rs`
- 修改： `desktop/src-tauri/src/lib.rs`
- 修改： `desktop/src-tauri/Cargo.toml`

- [x] **步骤 1：添加全局快捷键依赖并写映射测试**

运行： `cd desktop/src-tauri && cargo add tauri-plugin-global-shortcut@2`

```rust
assert_eq!(action_for_shortcut("Control+Alt+Space"), Some(CaptureAction::Quick));
assert_eq!(action_for_shortcut("Control+Alt+V"), Some(CaptureAction::Clipboard));
assert_eq!(action_for_shortcut("Control+Alt+S"), Some(CaptureAction::Screenshot));
assert_eq!(action_for_shortcut("Control+Alt+X"), None);
```

- [x] **步骤 2：运行并确认失败**

运行： `npm run test:rust -- menu::tests capture::screenshot::tests`

预期：失败，因为菜单和截图模块尚不存在。

- [x] **步骤 3：实现菜单栏**

创建菜单栏项目 `快速记录`、`捕获剪贴板`、`截图并识别`、`打开 IdeaCard`、分隔线和 `退出`。捕获项目发出 `capture://quick` 或 `capture://clipboard` 事件，或启动截图。打开主窗口时显示并聚焦；关闭主窗口时隐藏；只有点击退出才结束应用。

- [x] **步骤 4：实现快捷键降级**

三个快捷键独立注册并分别保存状态。任一注册失败时发出 `shortcut://status`，但不得阻止其他快捷键、菜单栏或主窗口启动。

- [x] **步骤 5：实现截图编排**

创建缓存路径并调用原生截图；取消时直接返回且不创建卡片；成功时调用 OCR、保存附件、创建图片卡片，并在存储后移除缓存文件。权限失败时附带恢复地址 `x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture`。

- [x] **步骤 6：验证并提交**

运行： `npm run test:rust -- menu::tests capture::screenshot::tests`

预期：通过，测试期间不进行真实全局注册或交互式截图。

```bash
git add desktop/src-tauri/Cargo.toml desktop/src-tauri/Cargo.lock desktop/src-tauri/src/menu.rs desktop/src-tauri/src/capture/screenshot.rs desktop/src-tauri/src/lib.rs
git commit -m "feat: add global capture entry points"
```

### 任务 9: 接入统一捕获界面和导入进度

**文件：**
- 创建： `desktop/src/features/capture/CaptureDialog.tsx`
- 创建： `desktop/src/features/capture/CaptureDialog.test.tsx`
- 创建： `desktop/src/features/capture/FileImport.tsx`
- 创建： `desktop/src/features/capture/FileImport.test.tsx`
- 修改： `desktop/src/services/desktop/contracts.ts`
- 修改： `desktop/src/services/desktop/tauriClient.ts`
- 修改： `desktop/src/pages/InboxPage.tsx`

- [x] **步骤 1：扩展服务契约**

```ts
readClipboardDraft(): Promise<CaptureDraft>
saveCaptureDraft(draft: CaptureDraft): Promise<Card>
selectAndImportFiles(): Promise<FileImportResult[]>
captureScreenshot(): Promise<{ status: 'saved'; card: Card } | { status: 'cancelled' }>
getCaptureStatus(): Promise<CaptureSystemStatus>
revealAttachment(id: string): Promise<void>
```

`revealAttachment` 在 Rust 侧先按附件 ID 查询相对路径，再通过附件仓库安全解析，最后交给系统默认应用打开；前端不得接收或拼接本地绝对路径。

- [x] **步骤 2：编写捕获界面失败测试**

分别编写带具体断言的测试：保存失败保留输入内容；URL 草稿显示 `网页文章`；截图取消不弹错误；OCR 为空但有一个附件时允许保存；文件导入为每个返回项渲染一行，并同时显示同批成功与失败结果。

- [x] **步骤 3：运行并确认失败**

运行： `npm test -- src/features/capture`

预期：失败，因为界面模块尚不存在。

- [x] **步骤 4：实现事件编排**

应用挂载时只订阅一次 `capture://quick` 和 `capture://clipboard`，卸载时取消订阅。只保留一个对话框；第二个事件只聚焦对话框，不重置已有输入。截图在对话框外执行，完成后选中新卡片。

- [x] **步骤 5：实现文件导入界面**

通过 `tauriClient` 使用原生文件对话框，只允许批准的扩展名，一次最多向 Rust 发送二十条路径；分别渲染成功、提取为空、提取失败、大小超限和类型不支持状态。

- [x] **步骤 6：验证并提交**

运行： `npm test -- src/features/capture && npm run lint && npm run build`

预期：通过。

```bash
git add desktop/src/features/capture desktop/src/services/desktop desktop/src/pages/InboxPage.tsx
git commit -m "feat: connect real desktop capture UI"
```

### 任务 10: 验证阶段 2 真实捕获

**文件：**
- 修改： `desktop/README.md`

- [x] **步骤 1：运行自动验证**

运行： `npm test && npm run lint && npm run build && npm run test:rust && npm run test:native`

预期：全部通过。

- [x] **步骤 2：在 Tauri 开发应用中验收**

验证菜单栏项目、三个快捷键、剪贴板文本/URL/图片、截图成功/取消/权限拒绝、PDF/DOCX/PPTX/XLSX/MD/TXT/图片/媒体导入、提取失败时保留附件，以及重启持久化。把准确结果记录在 `desktop/README.md` 的 `阶段 2 验收` 下。

- [x] **步骤 3：构建 `.app` 并复验 sidecar**

运行： `npm run tauri build -- --bundles app`

预期：打包后的应用包含 sidecar，并能在仓库目录之外完成 OCR。

- [x] **步骤 4：提交验收记录**

```bash
git add desktop/README.md
git commit -m "docs: record real capture verification"
```
