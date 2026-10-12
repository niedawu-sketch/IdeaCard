# IdeaCard 卡片归档与删除实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 增加收集箱与已归档工作区、卡片归档和恢复，以及带附件一致性保护的永久删除。

**Architecture:** 继续使用现有`archived`状态和`archived_at`字段，不新增迁移。React Store 一次加载全部卡片并按工作区筛选；Rust`CardLifecycleService`协调数据库事务与附件临时删除区，确保失败时恢复文件。

**Tech Stack:** React 19、Zustand、TypeScript、Vitest、Tauri 2、Rust、rusqlite、Playwright

---

### Task 1: 仓库层归档、恢复与删除规则

**Files:**
- Modify: `desktop/src-tauri/src/repository/card_repository.rs`

- [x] **Step 1: Write failing repository tests**

增加以下测试：

```rust
#[test]
fn list_includes_archived_only_when_requested()

#[test]
fn restore_archived_card_derives_status_from_note()

#[test]
fn delete_rejects_unarchived_card()

#[test]
fn delete_rejects_cards_referenced_by_analysis()

#[test]
fn delete_removes_attachment_rows_and_card_in_one_transaction()
```

分别断言空想法恢复为`Unprocessed`、非空想法和历史 AI 状态恢复为`Annotated`，错误码为`card_not_archived`与`card_delete_referenced`。

- [x] **Step 2: Run repository tests and verify RED**

Run:

```bash
cd desktop
npm run test:rust -- card_repository
```

Expected: FAIL，因为恢复和删除方法尚不存在。

- [x] **Step 3: Implement repository methods**

增加：

```rust
pub fn restore(&self, id: &str) -> Result<Card, AppError>;
pub fn delete_archived(&self, id: &str) -> Result<Vec<String>, AppError>;
```

`restore`只接受`Archived`，根据`note.trim().is_empty()`写入`Unprocessed`或`Annotated`并清空`archived_at`。`delete_archived`先检查 AI 引用，再在同一事务中删除`attachments`和`cards`，返回待清理附件相对路径。

- [x] **Step 4: Run repository tests**

Run:

```bash
cd desktop
npm run test:rust -- card_repository
```

Expected: PASS。

### Task 2: 附件临时删除与恢复

**Files:**
- Modify: `desktop/src-tauri/src/repository/attachment_repository.rs`
- Create: `desktop/src-tauri/src/card_lifecycle.rs`
- Modify: `desktop/src-tauri/src/lib.rs`

- [x] **Step 1: Write failing attachment lifecycle tests**

覆盖：

```rust
#[test]
fn staged_files_are_restored_when_database_delete_fails()

#[test]
fn committed_delete_removes_original_and_staged_files()

#[test]
fn startup_cleans_abandoned_staged_files()
```

使用临时目录创建真实附件文件，验证失败时原文件仍存在。

- [x] **Step 2: Run tests and verify RED**

Run:

```bash
cd desktop
npm run test:rust -- attachment_repository
npm run test:rust -- card_lifecycle
```

Expected: FAIL，因为临时删除区和生命周期服务尚不存在。

- [x] **Step 3: Implement transactional staging**

`AttachmentRepository`增加：

```rust
pub fn stage_removal(&self, relative_paths: &[String]) -> Result<StagedRemoval, AppError>;
pub fn cleanup_staged_removals(&self) -> Result<(), AppError>;
```

文件原子移动到附件根目录下`.deleting/<operation-id>/`。`StagedRemoval`未提交时`Drop`恢复文件；`commit()`删除临时目录。应用启动时调用`cleanup_staged_removals`。

`CardLifecycleService`增加：

```rust
pub fn archive(&self, id: &str) -> Result<Card, AppError>;
pub fn restore(&self, id: &str) -> Result<Card, AppError>;
pub fn delete(&self, id: &str) -> Result<(), AppError>;
```

删除顺序为读取卡片并验证归档、暂存附件、提交数据库删除、提交附件清理。

- [x] **Step 4: Run lifecycle tests**

Run:

```bash
cd desktop
npm run test:rust -- attachment_repository
npm run test:rust -- card_lifecycle
```

Expected: PASS。

### Task 3: Tauri 命令与前端服务契约

**Files:**
- Modify: `desktop/src-tauri/src/commands/card_commands.rs`
- Modify: `desktop/src-tauri/src/lib.rs`
- Modify: `desktop/src/services/desktop/contracts.ts`
- Modify: `desktop/src/services/desktop/tauriClient.ts`

- [x] **Step 1: Write failing command and service tests**

Rust 命令测试覆盖：

```rust
list_cards(true)
archive_card(id)
restore_card(id)
delete_card(id)
```

前端服务契约固定为：

```ts
listCards(includeArchived?: boolean): Promise<Card[]>
archiveCard(id: string): Promise<Card>
restoreCard(id: string): Promise<Card>
deleteCard(id: string): Promise<void>
```

- [x] **Step 2: Run focused tests and verify RED**

Run:

```bash
cd desktop
npm run test:rust -- card_commands
npm test -- src/services/desktop
```

Expected: FAIL，因为命令和契约尚不存在。

- [x] **Step 3: Implement command delegation**

`list_cards`接收`include_archived: Option<bool>`并传给仓库。归档、恢复和删除命令只转换参数，业务规则全部委托给`CardLifecycleService`。

- [x] **Step 4: Run focused tests**

Run:

```bash
cd desktop
npm run test:rust -- card_commands
npm test -- src/services/desktop
```

Expected: PASS。

### Task 4: Store 中的卡片生命周期

**Files:**
- Modify: `desktop/src/store/useDesktopStore.ts`
- Modify: `desktop/src/store/useDesktopStore.test.ts`

- [x] **Step 1: Write failing Store tests**

覆盖：

```ts
it('loads active and archived cards together')
it('archives a card and keeps it in the local collection')
it('restores an archived card')
it('permanently deletes a card and selects its next sibling')
it('keeps the card when a lifecycle operation fails')
```

- [x] **Step 2: Run Store tests and verify RED**

Run:

```bash
cd desktop
npm test -- src/store/useDesktopStore.test.ts
```

Expected: FAIL，因为 Store 没有生命周期方法。

- [x] **Step 3: Implement Store actions**

增加：

```ts
archiveCard(service, id): Promise<Card>
restoreCard(service, id): Promise<Card>
deleteCard(service, id): Promise<void>
```

`loadCards`调用`service.listCards(true)`。归档和恢复用返回卡片替换本地对象，删除成功后从数组移除；失败统一转换为`DesktopError`并保留原列表。

- [x] **Step 4: Run Store tests**

Run:

```bash
cd desktop
npm test -- src/store/useDesktopStore.test.ts
```

Expected: PASS。

### Task 5: 工作区导航与筛选

**Files:**
- Modify: `desktop/src/app/App.tsx`
- Modify: `desktop/src/pages/InboxPage.tsx`
- Modify: `desktop/src/pages/InboxPage.test.tsx`
- Modify: `desktop/src/features/inbox/CardList.tsx`

- [x] **Step 1: Write failing workspace tests**

断言：

```ts
it('switches between inbox and archived workspaces')
it('shows independent counts and source filters')
it('selects the first visible card after switching')
it('shows the approved empty states')
```

- [x] **Step 2: Run page tests and verify RED**

Run:

```bash
cd desktop
npm test -- src/pages/InboxPage.test.tsx
```

Expected: FAIL，因为只有收集箱入口。

- [x] **Step 3: Implement workspace navigation**

定义：

```ts
export type CardWorkspace = 'inbox' | 'archived'
```

`App`渲染“收集箱”和“已归档”两个导航按钮并维护当前工作区。`InboxPage`按`card.status === 'archived'`筛选，来源计数和搜索基于当前工作区卡片。切换后选择第一张可见卡片。

- [x] **Step 4: Run page tests**

Run:

```bash
cd desktop
npm test -- src/pages/InboxPage.test.tsx
```

Expected: PASS。

### Task 6: 卡片详情操作与删除确认

**Files:**
- Modify: `desktop/src/features/inbox/CardDetail.tsx`
- Modify: `desktop/src/features/inbox/CardDetail.test.tsx`
- Create: `desktop/src/features/inbox/DeleteCardDialog.tsx`
- Create: `desktop/src/features/inbox/DeleteCardDialog.test.tsx`
- Modify: `desktop/src/pages/InboxPage.tsx`
- Modify: `desktop/src/shared/styles/global.css`
- Modify: `DESIGN.md`

- [x] **Step 1: Write failing detail and dialog tests**

覆盖：

```ts
it('shows archive only for active cards')
it('disables archive while edits are unsaved')
it('renders archived cards read-only with restore and permanent delete')
it('requires confirmation before permanent deletion')
it('keeps the dialog open and shows an error when deletion fails')
```

- [x] **Step 2: Run component tests and verify RED**

Run:

```bash
cd desktop
npm test -- src/features/inbox
```

Expected: FAIL，因为操作入口和确认弹窗尚不存在。

- [x] **Step 3: Implement detail actions**

未归档详情底部使用`Archive`图标按钮“归档”。已归档详情字段设置为只读，底部使用`RotateCcw`“恢复”和`Trash2`“永久删除”。确认弹窗文案严格使用规格中的警示，危险按钮使用`--danger`，不新增色值。

- [x] **Step 4: Run component tests**

Run:

```bash
cd desktop
npm test -- src/features/inbox
```

Expected: PASS。

### Task 7: E2E、文档与 Mac 验收

**Files:**
- Modify: `desktop/e2e/inbox-layout.spec.ts`
- Modify: `desktop/README.md`

- [x] **Step 1: Add lifecycle E2E coverage**

模拟服务命令后验证归档、切换已归档、恢复、打开删除确认和删除后的列表状态。检查 1100×700 下导航、详情按钮和弹窗无重叠。

- [x] **Step 2: Run E2E**

Run:

```bash
cd desktop
npm run test:e2e
```

Expected: PASS。

- [x] **Step 3: Run full verification**

Run:

```bash
cd desktop
npm test -- --run
npm run lint
npm run build
npm run test:rust
npm run test:native
npm run test:e2e
npm run tauri build -- --debug
```

Expected: 全部退出码为 0；生成的`IdeaCard.app`可启动，归档、恢复、永久删除和附件清理行为符合规格。
