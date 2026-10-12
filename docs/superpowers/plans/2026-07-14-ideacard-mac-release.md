# IdeaCard Mac 发布验收实施计划

> **供智能执行者使用：** 必须使用 `superpowers:subagent-driven-development`（推荐）或 `superpowers:executing-plans`，逐项执行本计划。步骤使用复选框（`- [ ]`）跟踪。

**目标：** 补齐启动与失败恢复、安全门禁、自动端到端流程和真实 `.app` 验收，形成当前 Mac 可持续使用的 MVP 交付物。

**架构：** 自动测试分三层：浏览器端用注入式假桌面服务验证完整 UI 流程，Rust/Swift 集成测试验证桌面与系统边界，真实 `.app` 验收验证菜单栏、快捷键、权限和 Keychain。最终用逐项证据矩阵证明设计规格的十四条验收标准。

**技术栈：** Vitest、Playwright、Rust integration tests、Swift tests、Tauri bundler、macOS `open`、`codesign`、shell security scans

---

## 文件结构

```text
desktop/
├── e2e/complete-flow.spec.ts
├── e2e/failure-recovery.spec.ts
├── playwright.config.ts
├── scripts/check-secrets.sh
├── scripts/verify-app-bundle.sh
├── src/app/StartupGate.tsx
├── src/shared/errors/RecoveryPanel.tsx
└── src-tauri/icons/                    # IdeaCard 应用图标
docs/verification/2026-07-14-ideacard-mac-mvp-acceptance.md
```

### 任务 1: 完成启动门禁和可恢复错误界面

**文件：**
- 创建： `desktop/src/app/StartupGate.tsx`
- 创建： `desktop/src/app/StartupGate.test.tsx`
- 创建： `desktop/src/shared/errors/RecoveryPanel.tsx`
- 创建： `desktop/src/shared/errors/RecoveryPanel.test.tsx`
- 修改： `desktop/src/app/App.tsx`

- [ ] **步骤 1：编写启动失败测试**

测试数据库打开或迁移失败时显示错误码、信息、恢复动作和数据目录；此时不得渲染空收集箱，也不得提供会创建另一份数据库的重试。成功路径必须在 `initializeApp` 返回就绪后才加载收集箱。

- [ ] **步骤 2：编写恢复操作测试**

为录屏权限、API Key 缺失、快捷键冲突、附件缺失、提取失败和网络失败使用具体错误数据。断言界面只出现允许的动作：打开系统设置、打开应用设置、重试操作、显示数据目录或关闭提示。

- [ ] **步骤 3：运行并确认失败**

运行： `npm test -- src/app/StartupGate.test.tsx src/shared/errors/RecoveryPanel.test.tsx`

预期：失败，因为启动门禁和恢复面板尚不存在。

- [ ] **步骤 4：实现启动门禁和恢复映射**

`StartupGate` 使用 `initializing | ready | blocked` 三种状态。数据库阻断错误使应用保持只读，并且只提供显示数据目录与退出操作。`RecoveryPanel` 通过穷尽映射处理稳定错误码；未知错误码只显示信息和关闭按钮，不虚构恢复动作。

- [ ] **步骤 5：验证并提交**

运行： `npm test -- src/app/StartupGate.test.tsx src/shared/errors/RecoveryPanel.test.tsx`

预期：通过。

```bash
git add desktop/src/app desktop/src/shared/errors
git commit -m "feat: add recoverable desktop startup"
```

### 任务 2: 建立无系统依赖的完整 UI 端到端夹具

**文件：**
- 创建： `desktop/src/test/e2e/e2eService.ts`
- 创建： `desktop/src/test/e2e/e2eState.ts`
- 修改： `desktop/src/services/desktop/context.tsx`
- 创建： `desktop/playwright.config.ts`
- 创建： `desktop/e2e/complete-flow.spec.ts`
- 创建： `desktop/e2e/failure-recovery.spec.ts`

- [ ] **步骤 1：只在开发模式注入 E2E 服务**

仅当 `import.meta.env.DEV` 与查询参数 `e2e=1` 同时成立时，Provider 才使用 `e2eService`；生产构建始终使用 `tauriDesktopService`。增加构建测试，在 `dist` 中搜索哨兵字符串 `E2E_ONLY_SENTINEL`，并断言生产构建不包含该字符串。

- [ ] **步骤 2：编写完整流程 Playwright 测试**

`complete-flow.spec.ts` performs exact visible actions: create quick card, capture clipboard URL, import document result, filter each source, edit note/source, select two cards, start streamed AI with two deltas and one web citation, ask follow-up, save insight, continue session, reload, and assert all local fake state remains.

- [ ] **步骤 3：编写失败恢复 Playwright 测试**

在端到端假服务中用确定性开关触发剪贴板为空、OCR 为空、部分导入失败、快捷键冲突、API Key 缺失、鉴权失败、流中断、取消、上下文超限和附件缺失。逐项断言草稿或上下文得到保留，并显示准确的恢复动作。

- [ ] **步骤 4：运行并确认失败**

运行： `npm run test:e2e`

预期： FAIL until fixture and flows are implemented.

- [ ] **步骤 5：配置并通过 Playwright**

使用系统 Chrome，测试 1280x800 与 1620x900 两个视口；首次重试时记录 trace，失败时截图，Vite 测试服务使用 1421 端口。断言两个视口均无控制台或页面错误，也无横向溢出。

运行： `npm run test:e2e`

预期：全部端到端测试通过。

- [ ] **步骤 6：提交端到端流程**

```bash
git add desktop/src/test/e2e desktop/src/services/desktop/context.tsx desktop/playwright.config.ts desktop/e2e
git commit -m "test: cover complete desktop workflows"
```

### 任务 3: 增加桌面命令与重启集成测试

**文件：**
- 创建： `desktop/src-tauri/tests/desktop_commands.rs`
- 创建： `desktop/src-tauri/tests/restart_persistence.rs`
- 创建： `desktop/src-tauri/tests/migration_failure.rs`

- [ ] **步骤 1：编写真实临时目录命令测试**

用临时数据及缓存路径、假原生适配器和假 OpenAI 适配器构造 `AppState`。依次调用快速记录、附件导入、会话、完整消息、引用、结论和设置状态对应的服务函数，并在每次调用后断言数据库行和文件状态。

- [ ] **步骤 2：编写重启测试**

创建第一个 `AppState`，写入卡片、附件、会话和结论后释放；再从相同路径创建第二个状态，断言所有实体和附件字节完全一致，并断言恢复过程不依赖 Zustand 或 `localStorage`。

- [ ] **步骤 3：编写迁移失败测试**

复制测试数据库，在测试配置中注入无效的待执行迁移；断言启动被阻止、事务回滚、原结构版本保持不变，并且没有出现第二个数据库文件。

- [ ] **步骤 4：运行并通过**

运行： `npm run test:rust --test desktop_commands --test restart_persistence --test migration_failure`

预期：通过。

- [ ] **步骤 5：提交集成测试**

```bash
git add desktop/src-tauri/tests
git commit -m "test: verify desktop persistence boundaries"
```

### 任务 4: 建立密钥泄漏和权限安全门禁

**文件：**
- 创建： `desktop/scripts/check-secrets.sh`
- 创建： `desktop/scripts/check-secrets.test.sh`
- 修改： `desktop/package.json`
- 修改： `desktop/src-tauri/capabilities/default.json`

- [ ] **步骤 1：编写扫描脚本失败测试**

测试脚本先创建临时干净目录并断言扫描通过，再把 `sk-test-E2E_ONLY_SENTINEL` 写入文件；断言扫描失败并报告文件名，但不回显完整字符串。

- [ ] **步骤 2：实现安全扫描**

`check-secrets.sh` scans tracked/untracked source, `dist`, `.app` contents, SQLite files, and logs for OpenAI key prefixes and the known runtime verification sentinel. Exclude `node_modules`, Cargo target intermediates before bundle scan, `.git`, and Keychain. Print file path and rule name only.

- [ ] **步骤 3：收紧 Tauri capability**

只允许核心窗口、事件操作以及明确需要的对话框和外部打开能力。不得授予 shell 执行、任意文件系统、任意 HTTP 或前端 sidecar 权限；只由 Rust 使用的插件无需 WebView 权限。

- [ ] **步骤 4：检查日志和命令参数**

增加测试，通过 `Debug` 和 `Display` 格式化凭证、原生请求、OpenAI 请求及 `AppError`，断言其中不含密钥内容。检查 sidecar 启动参数，断言只包含可执行程序，不含 JSON 或密钥。

- [ ] **步骤 5：验证并提交**

运行： `npm run test:security && npm run test:rust && npm run test:native`

预期：通过。

```bash
git add desktop/scripts desktop/package.json desktop/src-tauri/capabilities desktop/src-tauri/src
git commit -m "test: prevent desktop secret leakage"
```

### 任务 5: 生成应用图标并验证 bundle 配置

**文件：**
- 创建： `desktop/scripts/generate-icon.swift`
- 创建： `desktop/src-tauri/icons/`
- 创建： `desktop/scripts/verify-app-bundle.sh`
- 修改： `desktop/src-tauri/tauri.conf.json`

- [ ] **步骤 1：生成一致的 IdeaCard 图标**

Swift 脚本在不透明的 1024x1024 位图上绘制现有深绿色方形标志和居中的白色 `I`，不使用渐变或装饰效果。生成所需 PNG 尺寸，再通过 `iconutil` 生成 `.icns`。生成的图标文件一并提交，使构建不依赖运行时字体。

- [ ] **步骤 2：配置 bundle**

设置产品名 `IdeaCard`、标识符 `com.niedawu.ideacard`、版本 `0.1.0`、最低 macOS 14、应用图标、仅 `.app` 构建目标和原生 sidecar，不配置签名身份。不得加入公开分发、自动更新、DMG、App Sandbox、麦克风、摄像头或辅助功能权限。

- [ ] **步骤 3：编写 bundle 验证脚本**

`verify-app-bundle.sh` asserts `.app` path, main executable, `Info.plist` identifier/version/minimum system, icon, sidecar executable, no updater configuration, no unexpected entitlements, and ad-hoc/local code signature validity with `codesign --verify --deep --strict`.

- [ ] **步骤 4：构建并验证**

运行：

```bash
npm run tauri build -- --bundles app
npm run verify:app
```

预期： both PASS and `IdeaCard.app` exists.

- [ ] **步骤 5：提交 bundle 资产**

```bash
git add desktop/scripts desktop/src-tauri/icons desktop/src-tauri/tauri.conf.json desktop/package.json
git commit -m "build: prepare local IdeaCard app bundle"
```

### 任务 6: 完成真实 `.app` 验收矩阵

**文件：**
- 创建： `docs/verification/2026-07-14-ideacard-mac-mvp-acceptance.md`
- 修改： `desktop/README.md`

- [ ] **步骤 1：从构建目录启动真实应用**

运行：

```bash
open desktop/src-tauri/target/release/bundle/macos/IdeaCard.app
```

预期：应用不依赖终端环境即可启动，并显示主窗口和菜单栏图标。

- [ ] **步骤 2：验收全部正常流程**

记录 date, app commit, macOS version, and pass/fail evidence for: tray, three shortcuts, five-second quick record, clipboard text/URL/image, screenshot OCR, every approved file class, five source filters, detail edits, persistence restart, AI offline-search toggle, real web search, citations, follow-up, cancel, retry, and immutable insight.

- [ ] **步骤 3：验收全部失败流程**

记录 evidence for: shortcut conflict, clipboard empty, screenshot cancel, screen permission denial, OCR empty, corrupt/oversize/unsupported files, partial batch failure, missing attachment, database open/migration failure test, missing/invalid/quota/rate/timeout API errors, stream interruption, cancel, and 200001-character context.

- [ ] **步骤 4：验证密钥边界**

先用非密钥哨兵值完成自动存储边界测试，扫描仓库、数据目录、日志、运行进程参数、SQLite 和 `.app`，断言只有 Keychain 测试项可包含哨兵值。真实 API Key 只能由用户在应用设置界面输入，不传给脚本或命令行；完成一次真实请求后移除该 Key，验证 Keychain 项已消失且业务数据仍保留。安全扫描只匹配常见 OpenAI Key 前缀和固定测试哨兵，不输出匹配内容。

- [ ] **步骤 5：对照十四条规格验收标准**

验收文档包含 `编号`、`要求`、`自动证据`、`真实应用证据`、`结果` 五列。第 1 至 14 行都必须关联命令结果、测试名称、截图路径或人工观察记录；缺少任一证据即判定失败，不得写成“默认通过”。

- [ ] **步骤 6：更新运行说明并提交**

README 使用中文记录本地安装路径、启动方式、数据路径、权限、API Key 设置、测试及构建命令、当前 MVP 范围和明确不做的能力。

```bash
git add docs/verification/2026-07-14-ideacard-mac-mvp-acceptance.md desktop/README.md
git commit -m "docs: verify IdeaCard Mac MVP"
```

### 任务 7: 最终全量验证与完成审计

**文件：**
- 仅当验证发现缺陷时修改对应文件。

- [ ] **步骤 1：运行所有自动验证**

运行：

```bash
cd desktop
npm test
npm run lint
npm run build
npm run test:rust
npm run test:native
npm run test:e2e
npm run test:security
npm run tauri build -- --bundles app
npm run verify:app
```

预期：每条命令退出码均为 0，失败测试数为 0。

- [ ] **步骤 2：检查工作树和差异**

运行：

```bash
git status --short
git diff --check
git log --oneline --decorate -20
```

预期：工作树干净、没有空白字符错误，各阶段提交边界清晰。

- [ ] **步骤 3：独立审查规格覆盖**

重新阅读 `docs/superpowers/specs/2026-07-14-ideacard-mac-mvp-design.md` 和验收矩阵。对每项明确功能、失败状态、隐私规则、非目标和验收标准，定位对应代码与证据；完成前修正所有矛盾或缺失项。

- [ ] **步骤 4：完成代码审查**

执行最终代码审查，重点检查数据丢失、密钥暴露、不安全路径、部分事务、取消竞态、状态持久化、系统权限恢复、打包和缺失测试。解决所有阻断项和高严重度问题后，重新执行步骤 1。

- [ ] **步骤 5：提交最终修复**

仅当步骤 1 至 4 确实发现并修复缺陷时：

```bash
git status --short
git add --all
git commit -m "fix: complete Mac MVP verification"
```

预期：没有缺陷时不创建空提交；存在修复时，提交只包含逐项核对过的文件。
