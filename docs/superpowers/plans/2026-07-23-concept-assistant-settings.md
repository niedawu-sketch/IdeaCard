# AI 概念助手设置 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 增加概念助手 System Prompt 的独立设置入口，并让概念解释只携带必要语境。

**Architecture:** Prompt 以独立键保存在现有 `app_settings` 表，通过 Tauri 命令暴露读取和保存接口。前端设置图标打开双入口菜单，概念助手使用独立弹窗。解释请求由后端组合用户规则、标题和选中文字附近原文。

**Tech Stack:** React、TypeScript、Vitest、Rust、Tauri、rusqlite

---

### Task 1: Prompt 领域规则与存储

**Files:**
- Modify: `desktop/src-tauri/src/domain/ai.rs`
- Modify: `desktop/src-tauri/src/repository/ai_repository.rs`

- [ ] 先添加默认值、字符限制、存取和错误路径测试。
- [ ] 运行定向 Rust 测试并确认因实现缺失而失败。
- [ ] 实现 `ConceptAssistantSettings`、校验函数和独立仓储键。
- [ ] 再次运行定向测试并确认通过。

### Task 2: 概念解释请求

**Files:**
- Modify: `desktop/src-tauri/src/ai/service.rs`
- Modify: `desktop/src-tauri/src/commands/ai_commands.rs`
- Modify: `desktop/src-tauri/src/lib.rs`

- [ ] 先添加测试，断言请求只含标题、附近原文和自定义 Prompt。
- [ ] 运行测试并确认失败。
- [ ] 实现附近原文截取、Prompt 组装及读写命令。
- [ ] 运行相关 Rust 测试并确认通过。

### Task 3: 前端模型与桌面接口

**Files:**
- Modify: `desktop/src/entities/ai/model.ts`
- Modify: `desktop/src/services/desktop/contracts.ts`
- Modify: `desktop/src/services/desktop/tauriClient.ts`
- Modify: `desktop/src/services/desktop/tauriClient.test.ts`

- [ ] 先添加命令映射测试并确认失败。
- [ ] 增加概念助手设置类型和桌面服务方法。
- [ ] 运行服务测试并确认通过。

### Task 4: 双入口设置界面

**Files:**
- Create: `desktop/src/features/settings/ConceptAssistantSettingsDialog.tsx`
- Create: `desktop/src/features/settings/ConceptAssistantSettingsDialog.test.tsx`
- Modify: `desktop/src/app/App.tsx`
- Modify: `desktop/src/app/App.test.tsx`
- Modify: `desktop/src/shared/styles/global.css`

- [ ] 先添加设置菜单、加载、恢复默认和保存测试并确认失败。
- [ ] 实现设置菜单和概念助手弹窗。
- [ ] 运行相关 React 测试并确认通过。

### Task 5: 文档与完整验证

**Files:**
- Modify: `PRODUCT.md`
- Modify: `DESIGN.md`

- [ ] 同步产品边界与设置组件规则。
- [ ] 运行完整 Rust 和 React 测试。
- [ ] 构建并验证使用 `IdeaCard Local Signing` 签名的 App。
