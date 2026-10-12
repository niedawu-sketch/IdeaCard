# IdeaCard 桌面端

`desktop/` 是 IdeaCard Mac 应用的正式桌面工程，使用 Tauri 2、React、TypeScript 和 Rust。

当前已完成 Mac MVP 阶段 4：除了快速记录、整理、归档和多种材料捕获，还支持正文贴图、概念注释与集合、卡片内 AI 延伸，以及 OpenAI、OpenAI 兼容接口和 Anthropic Claude 接入。原有 `prototype/` 继续作为交互参照，不直接改造成桌面应用。

## 环境要求

- macOS 14 或更高版本
- Node.js 22.12 或更高版本的 22.x，或 Node.js 24.x
- npm 10.9.2（最低支持 npm 10）
- 通过 rustup 安装的 Rust 1.97.0 工具链
- Xcode Command Line Tools
- Swift 6.2 或更高版本的工具链；运行 Swift 原生测试需要开发目录中的 `Testing` 框架

当前 Swift 原生组件构建脚本只支持 Apple Silicon（arm64），尚不支持 Intel Mac、Windows 或 Linux 桌面构建。

项目不支持 Node.js 23.x 或 25 及以上版本。Node.js 和 npm 的约束记录在 `package.json`，Rust 工具链、`rustfmt`、`clippy` 和 `aarch64-apple-darwin` 目标记录在 `rust-toolchain.toml`。

确认 Rust 工具链可用：

```bash
rustc --version
cargo --version
```

## 安装依赖

在本目录运行：

```bash
npm ci
```

依赖版本由 `package-lock.json` 和 `src-tauri/Cargo.lock` 固定，不要手工修改锁文件。

## 开发启动

启动 Tauri 桌面应用：

```bash
npm run tauri dev
```

日常界面与交互优化默认使用此开发版验收，代码更新后自动刷新，不重复生成和安装 `.dmg`。只有用户明确要求“打包”或进入阶段性交付时，才构建安装包。开发版与 `/Applications/IdeaCard.app` 共用本地数据、钥匙串和全局快捷键，启动前必须正常退出安装版，避免两个进程同时写入数据或争抢快捷键。

只启动前端开发服务器：

```bash
npm run dev
```

前端开发服务器固定使用 `http://localhost:1420`。

## 前端测试

运行全部前端测试：

```bash
npm test
```

运行桌面界面冒烟测试：

```bash
npm run test:e2e
```

冒烟测试使用本机 Google Chrome，在 1280px 和 1100px 宽度下检查主界面、快速记录窗口、文本右键菜单和卡片生命周期操作。

## Rust 测试

```bash
npm run test:rust
```

## Swift 原生测试

```bash
npm run test:native
```

测试脚本使用当前开发目录中的 `Testing` 框架。本次验证所用 Command Line Tools 已包含该框架；如果所用版本缺少 `Testing` 模块，需要使用包含该模块的 Xcode 工具链。

## 代码检查

```bash
npm run lint
```

## 构建

构建前端生产资源：

```bash
npm run build
```

构建本机安装用 macOS `.dmg`：

```bash
npm run tauri build -- --bundles dmg
hdiutil verify src-tauri/target/release/bundle/dmg/IdeaCard_0.1.0_aarch64.dmg
codesign --verify src-tauri/target/release/bundle/dmg/IdeaCard_0.1.0_aarch64.dmg
```

以上命令沿用维护者本机的签名身份。其他开发者可在构建命令中覆盖它，使用 ad-hoc 签名生成自己的 `.app`，无需修改项目配置或导入维护者的证书：

```bash
npm run tauri -- build --config '{"bundle":{"macOS":{"signingIdentity":"-"}}}' --bundles app
```

构建结果位于 `src-tauri/target/release/bundle/macos/IdeaCard.app`。ad-hoc 签名适用于本地开发验证，尚未经过 Apple 公证。签名方式见 [Tauri 官方说明](https://v2.tauri.app/distribute/sign/macos/)。

构建产物位于：

```text
src-tauri/target/release/bundle/dmg/IdeaCard_0.1.0_aarch64.dmg
```

`tauri.conf.json` 的 `bundle.icon` 必须显式包含 `icons/icon.icns`，否则 macOS 应用不会生成 `CFBundleIconFile`，安装后会显示系统占位图标。图标透明主图为 `src-tauri/icons/ideacard-icon.png`，各平台尺寸由 `npx tauri icon` 生成。

当前安装包使用本机的 `IdeaCard Local Signing` 身份签名，供当前 Mac 安装验证；尚未配置 Apple Developer ID 公证，不用于公开分发。

本仓库的开源交付包含源码与必要资源；本地数据库、附件、密钥、证书与安装包不进入 Git。

## 本地数据

IdeaCard 的业务数据只保存在本机：

```text
~/Library/Application Support/com.niedawu.ideacard/ideacard.sqlite3
~/Library/Application Support/com.niedawu.ideacard/attachments/
~/Library/Caches/com.niedawu.ideacard/attachments/
```

- `ideacard.sqlite3`：卡片与后续分析相关结构的 SQLite 数据库。
- 数据目录下的 `attachments/`：原始附件的正式存储位置。
- 缓存目录下的 `attachments/`：附件处理过程使用的可再生成缓存。

应用启动时会自动创建目录并执行版本化数据库迁移。数据库打开或迁移失败时会返回明确错误，不会静默创建备用数据库。

## 阶段 1 已实现

- 基于 SQLite 的版本化迁移、外键约束和本地附件仓库基础。
- 卡片真实创建、查询、更新，以及正文重复提示。
- 三栏收集箱、搜索、来源筛选和卡片详情编辑。
- 快速记录标题、正文与个人想法，保存后自动选中新卡片。
- 两种用户可见来源：个人记录、文档资料；历史网页、图片和音视频值继续兼容读取，当前不使用标签。
- 加载、创建、更新失败时的可恢复错误提示。
- 前端单元测试、桌面界面冒烟测试、Rust 测试和 `.app` 构建链路。

## 阶段 2 已实现

- 菜单栏入口：快速记录、捕获剪贴板、截图并识别、打开 IdeaCard 和退出。
- 三个固定全局快捷键：`Control+Option+Space` 快速记录、`Control+Option+V` 捕获剪贴板、`Control+1` 截图并识别；菜单栏对应动作右侧同步显示快捷键提示。
- 剪贴板普通文本、HTTP/HTTPS 网址和图片捕获；图片优先于文字，并通过 Vision OCR 提取文字。
- 区域截图、屏幕录制权限检查、取消处理、OCR、附件保存和图片卡片创建。
- PDF、DOCX、PPTX、XLSX、Markdown、TXT、图片和音视频文件导入；每批最多 20 个文件，单项失败不影响同批其他文件。
- 原始附件本地保存、卡片详情打开附件，以及提取失败时保留附件。
- 统一捕获确认框、逐文件导入结果和可恢复错误提示；确认框支持 `Esc` 关闭。

## 阶段 2 验收

验收日期：2026-07-14。

- 在真实 `.app` 中完成冷启动、主窗口前台唤起、关闭后驻留、菜单栏存在和三组全局快捷键注册验证。
- 在真实 `.app` 中验证剪贴板普通文本保留换行、HTTP/HTTPS 地址归类为文档资料、PNG 图片归类为个人记录且保留原图。
- 在真实 `.app` 中验证区域截图取消后不创建卡片；截图成功、权限拒绝和恢复地址由 Rust 与 Swift 测试夹具覆盖。验收过程未修改系统屏幕录制权限。
- 在真实 `.app` 中验证原生文件选择器与前端桥接可用；PDF、DOCX、PPTX、XLSX、Markdown、TXT、图片、媒体、损坏文件和提取失败保留附件由 Rust 集成测试生成夹具覆盖。界面脚本未向本地业务库写入额外导入数据。
- 验证重启后 SQLite 卡片仍然存在，主窗口和附件目录沿用同一本地工作区。
- 构建产物包含 `ideacard` 与 `ideacard-native` 两个 arm64 可执行文件。将完整 `.app` 复制到仓库目录外后，sidecar 独立 OCR 得到 `IdeaCard OCR 2026`。
- 当前 `.app` 使用 ad-hoc 本地签名，可通过 `codesign --verify --deep --strict` 的完整 bundle 校验；尚未配置开发者证书、强化运行时、公证、DMG 或公开分发。

## 阶段 3 已实现

- 文本右键菜单统一为应用内中文菜单：编辑字段只显示剪切、复制、粘贴、全选，普通文字选区只显示复制。
- 左侧工作区增加“收集箱 / 已归档”入口及独立数量，搜索和来源筛选只作用于当前工作区。
- 收集箱卡片可归档，也可经过二次确认直接永久删除；有未保存修改时不可归档，归档或删除后自动选择下一张卡片。
- 已归档卡片只读，可恢复到待整理或已整理，也可经过二次确认后永久删除。
- 永久删除同步清理卡片记录、附件记录和应用管理的本地附件；数据库删除失败时恢复已暂存文件。
- 应用启动时自动清理上次遗留的附件临时删除区。
- 主窗口空白区域长按约 260 毫秒后可直接拖动到另一块显示器；按钮、输入框、文字、卡片和弹窗保持原有交互。

## 阶段 4 已实现

- 原始内容和快速记录改为结构化正文，可在光标位置粘贴 PNG、JPEG、HEIC 和 WebP 图片；单图最大 20 MB。
- 图片二进制保存在本地附件目录，SQLite 只保存正文结构和附件引用；旧卡片读取时自动转换为结构化正文。
- 原文选中文字后，应用内右键菜单提供“AI 解释概念”；确认后在原文生成注释并保存到“概念集合”。
- 概念详情显示解释和所有来源卡片，归档来源仍保留；点击来源可返回对应卡片。
- 卡片详情突出“原始内容”和“我的想法”，来源与附件默认收起；AI 延伸由详情头部按钮打开右侧抽屉。
- 每张卡片保存独立 AI 对话和网页引用，可继续追问、联网查询或停止当前生成。用户消息发送后立即出现；官方 OpenAI、DeepSeek、OpenAI 兼容接口和 Anthropic Claude 均使用服务端流式响应，AI 延伸和概念解释在首段到达前显示轻量等待，之后连续追加。联网查询当前支持官方 OpenAI 和 `deepseek-v4-flash`。
- 模型设置位于左侧栏底部齿轮入口，支持官方 OpenAI、DeepSeek、OpenAI 兼容接口和 Anthropic Claude。DeepSeek 默认使用 `https://api.deepseek.com` 与 `deepseek-v4-flash`；切换到其他 DeepSeek 模型时会禁用联网查询。
- API Key 仅保存到 macOS 钥匙串，按供应商和接口地址隔离；SQLite、前端状态和日志不保存密钥。
- 数据库使用版本化迁移；当前迁移包含结构化正文、概念与 AI 回复详情，保留已有卡片、附件和旧分析记录。

## 语音输入

IdeaCard 在受支持的普通文本字段中使用真实、可见的 AppKit 控件：单行是 `NSTextField`，多行纯文本是 `NSTextView`。它们直接承担普通键盘、中文输入法、复制粘贴和豆包语音输入，不增加语音按钮、浮窗或确认步骤；收集箱搜索框和卡片“我的想法”已分别完成单行与多行实机验收。AI 延伸提问区沿用多行原生控件，并通过原生提交事件保留 `Enter` 发送、`Shift + Enter` 换行。TipTap 和设置页敏感字段暂不改造。

隐藏原生客户端、按键前后切换第一响应者以及主动激活 WebView 输入上下文均已因破坏键盘或中文输入法而停用。当前只扩展已经验证过的可见原生字段路线；TipTap 富文本和设置页敏感字段仍未获得豆包兼容，后续扩展必须按控件类型分别实机验收。

## 尚未实现

- 自动概念抽取和跨卡片自动关联。
- 云同步、跨设备同步和多人协作。
- Apple Developer ID 签名、公证和正式公开发布

本阶段规格和实施记录见：

```text
docs/superpowers/specs/2026-07-19-structured-card-ai-design.md
docs/superpowers/plans/2026-07-19-structured-card-ai.md
```
