# IdeaCard 文本右键菜单精简实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 用应用内中文基础菜单替换 macOS WebView 的完整文本右键菜单。

**Architecture:** `TextContextMenu` 在应用根部统一接管 `contextmenu` 事件并维护选区快照。纯文本剪贴板通过新增 Tauri 命令读写，输入框修改继续触发现有 React `input` 事件。

**Tech Stack:** React 19、TypeScript、Vitest、Testing Library、Tauri 2、Rust、arboard、Playwright

---

### Task 1: 系统纯文本剪贴板桥接

**Files:**
- Modify: `desktop/src-tauri/src/capture/clipboard.rs`
- Create: `desktop/src-tauri/src/commands/clipboard_commands.rs`
- Modify: `desktop/src-tauri/src/commands/mod.rs`
- Modify: `desktop/src-tauri/src/lib.rs`
- Modify: `desktop/src/services/desktop/contracts.ts`
- Modify: `desktop/src/services/desktop/tauriClient.ts`

- [x] **Step 1: Write the failing Rust tests**

为`SystemClipboard`提取可替换的纯文本读写边界，并在`clipboard_commands.rs`中使用测试双验证：

```rust
#[test]
fn plain_text_commands_read_write_and_preserve_content() {
    let clipboard = FakeTextClipboard::new(Some("原文字".to_owned()));
    assert_eq!(read_clipboard_text_with(&clipboard).unwrap(), Some("原文字".to_owned()));
    write_clipboard_text_with(&clipboard, "新文字".to_owned()).unwrap();
    assert_eq!(clipboard.written(), Some("新文字".to_owned()));
}
```

同时验证无文字返回`None`，读写失败返回稳定错误码`clipboard_read_failed`和`clipboard_write_failed`。

- [x] **Step 2: Run the Rust test and verify RED**

Run:

```bash
cd desktop
npm run test:rust -- clipboard_commands
```

Expected: FAIL，因为纯文本命令和写入接口尚不存在。

- [x] **Step 3: Implement the clipboard commands**

在`capture/clipboard.rs`中增加：

```rust
pub trait PlainTextClipboard {
    fn read_plain_text(&mut self) -> Result<Option<String>, AppError>;
    fn write_plain_text(&mut self, text: String) -> Result<(), AppError>;
}
```

`SystemClipboard`使用`arboard::Clipboard::get_text`和`set_text`实现。新增两个 Tauri 命令：

```rust
#[tauri::command]
pub(crate) fn read_clipboard_text() -> Result<Option<String>, AppError>;

#[tauri::command]
pub(crate) fn write_clipboard_text(text: String) -> Result<(), AppError>;
```

在前端服务增加：

```ts
readClipboardText(): Promise<string | null>
writeClipboardText(text: string): Promise<void>
```

- [x] **Step 4: Run focused verification**

Run:

```bash
cd desktop
npm run test:rust -- clipboard_commands
npm test -- src/services/desktop
npm run lint
```

Expected: 新增测试和现有服务测试通过。

### Task 2: 文本选区与编辑操作

**Files:**
- Create: `desktop/src/features/context-menu/textSelection.ts`
- Create: `desktop/src/features/context-menu/textSelection.test.ts`

- [x] **Step 1: Write failing selection tests**

覆盖以下行为：

```ts
it('cuts the selected textarea text and dispatches input', async () => {
  const textarea = document.createElement('textarea')
  textarea.value = '保留删除保留'
  textarea.setSelectionRange(2, 4)
  const selection = createTextControlSelection(textarea)
  selection.cut()
  expect(textarea.value).toBe('保留保留')
})

it('pastes text at the captured selection', () => {
  const input = document.createElement('input')
  input.value = '前后'
  input.setSelectionRange(1, 1)
  createTextControlSelection(input).paste('中')
  expect(input.value).toBe('前中后')
})
```

同时测试复制文本、全选、只读控件、普通页面文字选区和不支持的元素。

- [x] **Step 2: Run tests and verify RED**

Run:

```bash
cd desktop
npm test -- src/features/context-menu/textSelection.test.ts
```

Expected: FAIL，因为选区模块尚不存在。

- [x] **Step 3: Implement the selection model**

导出：

```ts
export interface TextSelectionSnapshot {
  kind: 'control' | 'document'
  selectedText: string
  canCut: boolean
  canCopy: boolean
  canPaste: boolean
  canSelectAll: boolean
  cut(): void
  paste(text: string): void
  selectAll(): void
}

export function captureTextSelection(target: EventTarget | null): TextSelectionSnapshot | null
```

输入框使用`selectionStart`、`selectionEnd`和`setRangeText`，操作后派发`new InputEvent('input', { bubbles: true, inputType: ... })`。普通文字选区只允许复制。

- [x] **Step 4: Run focused tests**

Run:

```bash
cd desktop
npm test -- src/features/context-menu/textSelection.test.ts
```

Expected: PASS。

### Task 3: 自定义右键菜单组件

**Files:**
- Create: `desktop/src/features/context-menu/TextContextMenu.tsx`
- Create: `desktop/src/features/context-menu/TextContextMenu.test.tsx`
- Modify: `desktop/src/app/App.tsx`
- Modify: `desktop/src/shared/styles/global.css`
- Modify: `DESIGN.md`

- [x] **Step 1: Write failing component tests**

测试必须覆盖：

```ts
it('shows only cut copy paste and select all for editable text')
it('shows only copy for a document selection')
it('disables cut and copy without a selection')
it('closes after an action, outside click, Escape, scroll and blur')
it('keeps the menu inside the viewport')
it('reports clipboard failures without showing the native menu')
```

断言菜单中不存在“翻译”“搜索”“检查元素”“服务”等文字。

- [x] **Step 2: Run component tests and verify RED**

Run:

```bash
cd desktop
npm test -- src/features/context-menu/TextContextMenu.test.tsx
```

Expected: FAIL，因为组件尚不存在。

- [x] **Step 3: Implement and mount the component**

组件在`document`上注册`contextmenu`监听器，始终调用`preventDefault()`，仅在`captureTextSelection`返回有效目标时展示菜单。菜单使用：

```tsx
<div className="text-context-menu" role="menu">
  <button role="menuitem">剪切</button>
  <button role="menuitem">复制</button>
  <button role="menuitem">粘贴</button>
  <button role="menuitem">全选</button>
</div>
```

根据目标类型过滤项目，根据能力设置`disabled`。位置通过`Math.min(pointer, viewport - menuSize - 8)`约束。样式复用`--surface`、`--line-strong`、`--r-md`和现有弹层阴影。

- [x] **Step 4: Run focused and frontend verification**

Run:

```bash
cd desktop
npm test -- src/features/context-menu
npm run lint
npm run build
```

Expected: 全部通过。

### Task 4: 桌面运行态验证

**Files:**
- Modify: `desktop/e2e/inbox-layout.spec.ts`
- Modify: `desktop/README.md`

- [x] **Step 1: Add a failing E2E check**

在卡片正文输入框中选择文字并触发右键，断言：

```ts
await expect(page.getByRole('menuitem', { name: '复制' })).toBeVisible()
await expect(page.getByText('Inspect Element')).toHaveCount(0)
```

检查菜单边界完整位于视口内。

- [x] **Step 2: Run E2E and verify RED or missing coverage**

Run:

```bash
cd desktop
npm run test:e2e -- e2e/inbox-layout.spec.ts
```

Expected: 新测试在组件接入前失败；接入后通过。

- [x] **Step 3: Run full verification**

Run:

```bash
cd desktop
npm test -- --run
npm run lint
npm run build
npm run test:rust
npm run test:e2e
npm run tauri build -- --debug
```

Expected: 全部退出码为 0，并生成`desktop/src-tauri/target/debug/bundle/macos/IdeaCard.app`。
