# IdeaCard Mac MVP 实施计划

> **供智能执行者使用：** 必须使用 `superpowers:subagent-driven-development`（推荐）或 `superpowers:executing-plans`，逐项执行本计划。步骤使用复选框（`- [ ]`）跟踪。

**长期目标：** 在当前 Mac 上交付一个可安装运行的 IdeaCard `.app`，实现真实本地捕获、SQLite 持久化、AI 延伸与联网查询、结论保存和完整异常恢复。

**当前执行范围：** 优先完成并稳定非 AI 主流程。AI 相关模块只保留界面，真实模型、联网搜索、Keychain 和结论保存暂缓开发，恢复时间以新的确认规格为准。

**架构：** MVP 拆成四个连续且可独立验收的子计划。React 负责界面，Tauri/Rust 负责业务、SQLite、附件、系统能力和 OpenAI，随应用打包的 Swift 辅助程序负责 Vision、截图和 Keychain。

**技术栈：** Tauri 2.11、React 19、TypeScript 5.9、Rust stable、rusqlite、reqwest、Swift、Vision、Security、Vitest、Rust tests、Playwright

---

## 基线

- 分支：`codex/ideacard-mac-mvp`
- 工作树：`/Users/niedawu/coding/IdeaCard/.worktrees/ideacard-mac-mvp`
- 原型合并基线：`6cd94cc`
- 设计规格：`docs/superpowers/specs/2026-07-14-ideacard-mac-mvp-design.md`
- 原型验证：11 个测试文件、98 个测试通过，`npm run lint` 与 `npm run build` 通过。

## 子计划与执行顺序

- [x] **阶段 1：桌面基础与本地数据**

  执行 `docs/superpowers/plans/2026-07-14-ideacard-mac-foundation.md`。阶段结束时必须有可启动的 Tauri 主窗口、SQLite 初始迁移、附件仓库、真实卡片 CRUD 和快速记录。

- [x] **阶段 2：真实捕获能力**

  执行 `docs/superpowers/plans/2026-07-14-ideacard-mac-capture.md`。阶段结束时必须有菜单栏、全局快捷键、剪贴板、文件导入、区域截图和 Vision OCR。

- [ ] **阶段 3：AI 延伸、查询与结论（暂缓）**

  当前不执行真实能力开发，只保留界面。后续重新确认 AI 是“想法延伸与即时查询”而非“材料深度分析”后，再修订并执行 `docs/superpowers/plans/2026-07-14-ideacard-mac-ai.md`。

- [ ] **阶段 4：恢复、安全与 `.app` 验收**

  执行 `docs/superpowers/plans/2026-07-14-ideacard-mac-release.md`。阶段结束时必须完成异常恢复、安全扫描、真实系统能力验收和 `.app` 构建。

## 跨阶段固定约束

- `prototype/` 保留为交互参照，不在桌面运行时使用。
- 正式产品只写入 `desktop/`。
- 前端不能直接访问 SQLite、Keychain、任意文件系统或 OpenAI。
- Tauri 命令只做参数转换，业务逻辑放在 Rust 服务和仓库中。
- API Key 只能存在于 Keychain 和请求期间的 Rust 内存中。
- 所有迁移按顺序新增，已提交迁移不可改写。
- 附件先写临时文件、校验后原子移动。
- 不静默截断 AI 上下文；超过 200,000 字符时阻止请求。
- 不实现标签、云同步、多模型、音视频转写或公开分发。
- 卡片永久删除只允许从已归档工作区发起，并同时清理应用管理的本地附件。
- 每个功能遵循失败测试、最小实现、通过测试、独立提交的顺序。

## 十四条验收标准映射

| 编号 | 规格验收要求 | 实施与证据位置 |
| --- | --- | --- |
| 1 | 主窗口、菜单栏和重启持久化 | 基础计划任务 10；捕获计划任务 8；发布计划任务 3、6 |
| 2 | 快速记录 | 基础计划任务 9、10；发布计划任务 2、6 |
| 3 | 剪贴板文本与链接捕获 | 捕获计划任务 6、9、10；发布计划任务 2、6 |
| 4 | 区域截图与本地 OCR | 捕获计划任务 3、4、5、8、10；发布计划任务 6 |
| 5 | 文件与媒体导入 | 捕获计划任务 1、7、9、10；发布计划任务 2、6 |
| 6 | 五类来源与无标签界面 | 基础计划任务 2、9；发布计划任务 2、6 |
| 7 | 卡片编辑与状态流转 | 基础计划任务 5、7、8、9；发布计划任务 2、6 |
| 8 | 多卡片进入 AI 工作台 | AI 计划任务 1、5、7、8；发布计划任务 2、6 |
| 9 | AI 延伸、连续追问和取消 | AI 计划任务 4、6、7、8、9；发布计划任务 2、6 |
| 10 | 联网查询与网页引用 | AI 计划任务 5、6、8、9；发布计划任务 2、6 |
| 11 | 不可变结论快照 | AI 计划任务 1、4、8、9；发布计划任务 2、6 |
| 12 | API Key 仅存 Keychain | AI 计划任务 2、3；发布计划任务 4、6 |
| 13 | 超限、失败、权限与部分成功恢复 | 四份子计划中的失败测试；发布计划任务 1、2、3、6 |
| 14 | 可构建、可独立启动的 `.app` | 基础计划任务 10；发布计划任务 5、6、7 |

## 每阶段统一验证

在 `desktop/` 运行：

```bash
npm test
npm run lint
npm run build
npm run test:rust
```

预期：全部退出码为 0。

阶段 2 起增加：

```bash
npm run test:native
```

阶段 4 增加：

```bash
npm run test:e2e
npm run tauri build -- --bundles app
```

预期：端到端测试通过，并生成可启动的 `IdeaCard.app`。

## 最终完成证据

- 四份子计划中的复选框全部完成。
- 自动测试、代码检查、前端构建、Rust 测试、Swift 测试和桌面端到端测试全部通过。
- 真实 `.app` 完成快速记录、剪贴板、截图 OCR、文件导入、AI 延伸、联网查询、追问和结论保存。
- 退出并重启 `.app` 后数据仍存在。
- SQLite、日志、前端状态和构建产物中未发现 API Key。
- 设计规格的十四条验收标准逐项有对应证据。
