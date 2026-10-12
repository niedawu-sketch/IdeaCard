# IdeaCard 原生语音输入桥实现计划

**状态：** 三种隐藏桥接尝试均实测失败；可见普通文本字段扩展已完成自动化与实机验收。

**目标：** 在不增加语音按钮、不内置语音识别的前提下，让 IdeaCard 的可编辑区域通过按住 `Fn` 使用豆包语音输入；说话过程中流式显示识别草稿，结束后把最终结果提交到原光标或选区。

**设计规格：** `docs/superpowers/specs/2026-07-27-native-voice-input-bridge-design.md`

**技术栈：** React 19、TypeScript、TipTap 3、Tauri 2、Rust、objc2、AppKit、Vitest、Cargo tests、Playwright

## 实施原则

- 第一阶段只验证最关键的 `目标聚焦 → 原生第一响应者已预备 → Fn → 豆包文字` 时序。
- 时序未在真实 `.app` 通过前，不扩展到全部字段和 TipTap。
- 不增加任何可见语音 UI，用户交互完全由豆包提供。
- 不修改数据库、`.env`、密钥和 CI/CD。
- 不覆盖当前截图、AI 和界面精简相关未提交改动。
- 每个阶段先补测试，再写实现，再运行本阶段验证。

## 第二版修正

第一版诊断顺序为：前端目标有效 → 收到 `Fn` → 原生 `NSTextView` 成功成为第一响应者 → 豆包没有出现且原生文本无变化。说明 `Fn` 后切换晚于豆包锁定输入目标。

第二版不做豆包专用 API：

1. 普通字段或 TipTap 聚焦后，等待 WebView 完成本轮聚焦和选区初始化，再预备标准 `NSTextView`；
2. `Fn` 只开始和结束草稿会话，不再承担第一响应者切换；
3. 物理键盘 `keyDown` 到达前恢复 WebView，使普通输入、快捷键、粘贴和撤销走原网页路径；
4. 网页 `keyUp`、`pointerup` 和组合输入结束后，延后一轮事件再重新预备原生客户端；同一目标重复上报时刷新一次第一响应者，避免首次聚焦尚未完成初始化；
5. 豆包通过标准文本系统写入原生客户端后，沿用现有完整草稿替换逻辑回填普通字段或 TipTap。
6. `Fn` 的本地事件监听器只记录按住状态和时间，不清空文本客户端、不创建会话、不向前端发事件；业务会话由豆包第一份真实文字懒启动。
7. 原生文本变化通知只登记刷新任务，同一轮 AppKit 编辑合并后再读取完整草稿并跨桥发送，避免在输入法回调中连续同步网页造成主线程转圈。

## 第三版修正

第二版虽能获得豆包流式文字，但 WebView 与隐藏 `NSTextView` 之间的第一响应者往返会让语音和普通键盘都出现转圈。普通键盘体验是硬约束，因此停止使用长期预占第一响应者：

1. 不安装 `Fn` / `keyDown` 本地监听器；
2. 不创建隐藏 `NSTextView`，WebView 始终保留第一响应者；
3. 编辑目标聚焦稳定后，只激活当前 `WKContentView` 已有的 `NSTextInputContext`；
4. 键盘、快捷键、粘贴、撤销、系统输入法和豆包结果都直接进入 WebView；
5. 先验证键盘完全恢复，再实测豆包是否能在首次聚焦后直接启动；若仍失败，结论是当前 Tauri/WKWebView 不能在不影响键盘的前提下用第一响应者桥接解决，需要转向真正的原生编辑控件或上游 WebView 兼容性方案。

实测结果：主动激活 `WKContentView` 的 `NSTextInputContext` 会导致中文输入源无法生效，因此第三版也停止。当前代码不再安装事件监听器、不创建隐藏文本客户端，也不操作 WebView 输入上下文，优先保证英文键盘、中文输入法、快捷键和粘贴等基础输入完全恢复。

## 真实原生标题字段实验

用户确认后，仅在卡片详情标题执行第四版实验：

1. 新增 AppKit `NSTextField` 生命周期模块与 Tauri 同步命令；
2. 新增前端 `NativeTextField` 宿主组件，负责值、位置、可见性和聚焦状态同步；
3. 只替换 `CardDetail` 的标题 input，其他字段保持安全基线；
4. 非 Tauri 环境和测试继续使用原 HTML input；
5. 自动化验证只证明数据同步和回退，豆包、中文输入法与粘贴必须在真实开发 App 中手测；
6. 任一基础输入出现延迟、输入源失效或弹层遮挡错误，立即回退该实验，不扩展范围。

**实机结果（2026-07-27）：** 普通键盘、中文输入法、豆包首次流式输入和复制粘贴四项全部通过。真实 `NSTextField` 实验完成；后续扩展必须把普通单行、多行纯文本和 TipTap 分成独立阶段。

## 当前扩展阶段

本阶段只推广已经验证过的可见原生控件路线：

1. 把单个 `NSTextField` 生命周期升级成按字段 ID 管理的多控件集合；
2. 新增同样可见、直接接收系统输入的 `NSTextView` 多行宿主；
3. 覆盖收件箱搜索、快速记录、确认捕获、概念名称、卡片标题/想法/来源字段；
4. 保留 HTML 控件作为布局、标签和非 Tauri 回退，不增加语音按钮或状态 UI；
5. 不改 TipTap、AI 对话输入框、设置页敏感字段、`select` 和 checkbox；
6. 完成自动化回归后，再在开发 App 中逐项验证普通键盘、中文输入法、首次豆包流式输入、当前光标位置和复制粘贴。

AI 对话输入框含 Enter 发送和动态高度，TipTap 含富文本节点、选区及撤销历史，两者不能当作普通 `textarea` 直接替换，后续分别设计。

**自动化结果（2026-07-27）：** ESLint、TypeScript/Vite 生产构建、157 个前端测试、204 个 Rust 测试、43 个 Swift 原生测试和 9 个 Playwright 端到端测试全部通过。自动化不能替代豆包、中文输入法和 AppKit 光标行为的真实验收。

**实机结果（2026-07-27）：** 收集箱搜索框和卡片“我的想法”两个代表字段均无问题，分别验证了单行 `NSTextField` 与多行 `NSTextView`。本阶段完成；TipTap 和 AI 对话输入框继续作为后续独立任务。

## 任务 1：建立纯状态模型

**文件：**

- 新建 `desktop/src-tauri/src/native/voice_input_state.rs`
- 修改 `desktop/src-tauri/src/native/mod.rs`

**步骤：**

1. 定义 `VoiceTargetToken`、`VoiceSessionId` 和状态枚举。
2. 定义输入事件：目标变化、`Fn` 按下/松开、其他键、原生完整草稿变化、稳定超时、总超时、取消。
3. 定义输出动作：启用不可见原生文本客户端、替换流式草稿、最终提交、取消回滚、恢复网页目标。
4. 覆盖以下 Rust 单元测试：
   - 没有目标时忽略 `Fn`；
   - 快速点按取消；
   - 组合键取消并恢复；
   - 完整草稿变化替换临时范围而不是追加；
   - 松开后继续接收和显示最终草稿；
   - 稳定文字提交；
   - 旧会话 ID 不能关闭新会话；
   - 目标失效不提交。

**验证：**

```bash
npm run test:rust
```

## 任务 2：制作最小 AppKit 时序验证

**文件：**

- 新建 `desktop/src-tauri/src/native/voice_input.rs`
- 新建 `desktop/src-tauri/src/commands/voice_input_commands.rs`
- 修改 `desktop/src-tauri/src/commands/mod.rs`
- 修改 `desktop/src-tauri/src/lib.rs`
- 修改 `desktop/src-tauri/Cargo.toml`
- 修改 `desktop/src-tauri/Cargo.lock`
- 临时在现有普通文本字段注册单个测试目标

**步骤：**

1. 将 `objc2`、`objc2-app-kit`、`objc2-foundation` 声明为 macOS 直接依赖。
2. 在 Tauri 主线程安装 `flagsChanged`、`keyDown`、`keyUp` 本地监听。
3. 仅当主窗口当前存在有效测试目标时响应单独 `Fn`。
4. 在主窗口原生视图层级中挂载一个不绘制、不接收鼠标、无持久内容的 `NSTextView`，把它设为第一响应者。
5. 每次原生文本变化都把当前完整草稿通过 Tauri 事件发送到前端。
6. 前端只对一个普通文本框完成流式临时范围替换、取消回滚和最终提交，用于验证端到端时序。
7. 构建并签名真实 `.app`，不以浏览器和 `tauri dev` 结果替代。

**自动验证：**

```bash
npm run test:rust
npm run build
npm run tauri build
codesign --verify --deep --strict src-tauri/target/release/bundle/macos/IdeaCard.app
```

**人工验证门槛：**

1. 打开真实 `.app`，点击测试字段。
2. 按住 `Fn`，确认豆包悬浮窗出现。
3. 说出包含中文、英文和标点的一句话。
4. 持续说话时，确认原字段同步显示逐步变化的识别草稿，不等待松开后才出现。
5. 松开 `Fn`，确认延迟到达的最终修订继续更新，稳定后完成提交。
6. 执行一次撤销，确认恢复会话前内容和原选区。
7. 快速点按 `Fn`，确认不改变内容。
8. 测试至少一个 `Fn + F键` 组合，确认未被破坏。

**决策：**

- 通过：继续任务 3。
- 不通过：停止完整开发，保留最小诊断证据并回报，不添加按钮式降级。

## 任务 3：实现普通文本控件目标注册

**文件：**

- 新建 `desktop/src/features/voice-input/voiceInputTargets.ts`
- 新建 `desktop/src/features/voice-input/voiceInputTargets.test.ts`
- 新建 `desktop/src/features/voice-input/VoiceInputBridge.tsx`
- 新建 `desktop/src/features/voice-input/VoiceInputBridge.test.tsx`
- 修改 `desktop/src/app/App.tsx`
- 修改 `desktop/src/services/desktop/contracts.ts`
- 修改 `desktop/src/services/desktop/tauriClient.ts`
- 修改相关服务测试夹具

**步骤：**

1. 用全局捕获阶段监听注册普通 `input` / `textarea`，不逐个业务组件增加重复代码。
2. 支持 `text`、`search`、`email`、`tel`、`url` 和 `textarea`。
3. 排除 `password`、`disabled`、`readOnly` 和未显示控件。
4. 在 focus、selection、input、composition 变化时更新目标快照。
5. 目标快照保存 DOM 元素、令牌、原值、原选区、当前临时范围和内容版本；Rust 只收到令牌与可用状态。
6. 每份完整草稿整体替换当前临时范围；不得把草稿差异误当作只追加字符。
7. 取消时恢复原值和原选区；最终提交时先无历史恢复原状态，再用一次正式编辑写入最终文字。
8. 单行字段规范化换行，每次可见草稿更新触发标准 `input` 事件。
9. 目标失效、内容版本冲突或组合输入进行中时拒绝提交，并在安全时回滚临时草稿。
10. 覆盖流式回改、插入、替换、取消回滚、多行、单行换行、只读、密码、stale target 和 composition 测试。

**验证：**

```bash
npm test -- voiceInputTarget VoiceInputBridge
npm run lint
npm run build
```

## 任务 4：接入 TipTap

**文件：**

- 修改 `desktop/src/features/editor/RichTextEditor.tsx`
- 修改 `desktop/src/features/editor/RichTextEditor.test.tsx`
- 修改 `desktop/src/features/voice-input/voiceInputTarget.ts`

**步骤：**

1. 在 TipTap `focus`、`blur`、`selectionUpdate` 时注册和更新目标。
2. 快照保存编辑器挂载令牌、`from` / `to`、原文档 slice 和文档 revision。
3. 每份流式草稿用 `addToHistory: false` 的 transaction 整体替换上一份临时草稿。
4. 取消时用无历史 transaction 恢复原 slice 和选区。
5. 最终提交时先无历史恢复原 slice，再用一个正式 transaction 插入最终纯文本，保证一次撤销恢复会话前内容。
6. 提交时校验 editor 未销毁、仍可编辑、revision 未发生不兼容变化。
7. 验证流式回改、段落换行、图片节点、概念 mark 和撤销历史不被破坏。
8. read-only 编辑器不注册目标。

**验证：**

```bash
npm test -- RichTextEditor voiceInputTarget
npm run lint
npm run build
```

## 任务 5：完成不可见原生会话

**文件：**

- 修改 `desktop/src-tauri/src/native/voice_input.rs`
- 修改 `desktop/src-tauri/src/native/voice_input_state.rs`
- 修改 `desktop/src-tauri/src/commands/voice_input_commands.rs`

**步骤：**

1. 完成 180ms 快速点按、功能键取消、流式草稿事件、450ms 最终稳定提交和 8s 总超时。
2. 完成目标失效时的临时草稿回滚。
3. 会话结束清空 `NSTextView`，释放临时原生第一响应者并恢复 WebView。
4. 保证不可见文本视图不绘制、不参与鼠标命中和辅助布局。
5. 保证输入桥初始化失败时不影响普通输入和应用启动。
6. 验证代码和样式中没有新增语音按钮、浮层、面板、状态文字或错误提示。

**验证：**

```bash
npm run test:rust
npm run lint
npm test
npm run build
```

## 任务 6：全量回归与文档同步

**文件：**

- 修改 `docs/2026-07-20-doubao-voice-input-compatibility.md`
- 修改 `desktop/README.md`
- 必要时修改 `PRODUCT.md`
- 必要时新增或修改 `desktop/e2e/` 用例

**步骤：**

1. 把兼容性文档状态从“暂缓处理”改为实际实现与限制。
2. 删除 README 中与实测冲突的旧描述，记录真实 `.app` 验证结果。
3. 运行前端、Rust、Swift、构建和 e2e 全量验证。
4. 构建签名 `.app`，逐项执行设计规格中的 12 条验收标准。
5. 记录不能自动化的 `Fn + 豆包` 实测结果，不以代码编译成功代替。

**全量验证：**

```bash
npm run lint
npm test
npm run build
npm run test:rust
npm run test:native
npm run test:e2e
npm run tauri build
codesign --verify --deep --strict src-tauri/target/release/bundle/macos/IdeaCard.app
```

## 风险与停止条件

### 风险 1：豆包过早锁定 WKWebView

如果豆包在本地 `NSEvent` 监听改变第一响应者之前就固定了目标，主进程原生桥也无法完成直接 `Fn`。任务 2 用最小真实 `.app` 提前验证这一点。

### 风险 2：`Fn` 功能键被破坏

输入桥会短暂接管第一响应者。必须证明组合键事件到达前可以同步恢复 WebView；否则不能发布。

### 风险 3：流式识别会回改已有文字

豆包可能在按住和松开后多次改写已经显示的词语、标点与分段。每次传递并整体替换完整临时草稿，不把变化误判成只追加；最终仍采用稳定窗口与总超时。

### 风险 4：目标在识别期间变化

使用挂载令牌和内容 revision；宁可回滚临时草稿，也不把文字写入错误字段。

### 停止条件

以下任一情况发生时停止并向用户报告，不自行扩大权限或改变交互：

- 本地事件监听时序无法让豆包识别原生 `NSTextView`；
- 需要新增辅助功能或输入监控权限；
- 无法保持 `Fn` 功能键行为；
- 只能通过额外点击按钮才能稳定工作。
