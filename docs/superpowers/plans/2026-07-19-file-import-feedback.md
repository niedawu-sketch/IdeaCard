# 文件导入反馈 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让成功导入提示在 1 秒后自动消失，异常结果可手动关闭，并在导入入口展示支持格式，同时删除剪贴板图片确认弹窗中的“已附原图”提示。

**Architecture:** `FileImport` 继续作为导入状态的唯一所有者，通过一个基于 `results` 的 effect 定时移除成功项，异常项保留到用户关闭。`CaptureMenu` 只负责入口文案，`CaptureDialog` 只删除重复提示，不改变附件数据和保存流程。

**Tech Stack:** React、TypeScript、Vitest、Testing Library、Playwright、Tauri

---

### Task 1: 文件导入结果生命周期

**Files:**
- Modify: `desktop/src/features/capture/FileImport.tsx`
- Test: `desktop/src/features/capture/FileImport.test.tsx`
- Modify: `desktop/src/shared/styles/global.css`

- [ ] **Step 1: 编写成功自动消失的失败测试**

使用 Vitest 假计时器导入一个成功文件，确认 999ms 时仍存在，1000ms 时消失：

```tsx
it('removes successful imports after one second', async () => {
  vi.useFakeTimers()
  const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime })
  renderFileImport(vi.fn().mockResolvedValue([
    { originalName: '方案.pdf', status: 'saved', card, errorCode: null },
  ]))

  await user.click(screen.getByRole('button', { name: '导入本地文件' }))
  expect(await screen.findByText('导入成功')).toBeInTheDocument()

  await act(async () => vi.advanceTimersByTime(999))
  expect(screen.getByText('导入成功')).toBeInTheDocument()
  await act(async () => vi.advanceTimersByTime(1))
  expect(screen.queryByText('导入成功')).not.toBeInTheDocument()
  vi.useRealTimers()
})
```

- [ ] **Step 2: 编写混合结果和关闭异常的失败测试**

```tsx
it('keeps failed results after successes expire and allows dismissing them', async () => {
  vi.useFakeTimers()
  const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime })
  renderFileImport(vi.fn().mockResolvedValue([
    { originalName: '方案.pdf', status: 'saved', card, errorCode: null },
    { originalName: '超限.mp4', status: 'failed', card: null, errorCode: 'attachment_too_large' },
  ]))

  await user.click(screen.getByRole('button', { name: '导入本地文件' }))
  expect(await screen.findByText('方案.pdf')).toBeInTheDocument()
  expect(screen.getByText('超限.mp4')).toBeInTheDocument()

  await act(async () => vi.advanceTimersByTime(1000))
  expect(screen.queryByText('方案.pdf')).not.toBeInTheDocument()
  expect(screen.getByText('超限.mp4')).toBeInTheDocument()

  await user.click(screen.getByRole('button', { name: '关闭导入结果' }))
  expect(screen.queryByLabelText('文件导入结果')).not.toBeInTheDocument()
  vi.useRealTimers()
})
```

- [ ] **Step 3: 运行测试确认失败**

Run:

```bash
cd desktop
npm test -- src/features/capture/FileImport.test.tsx
```

Expected: FAIL，成功结果不会自动消失且不存在关闭按钮。

- [ ] **Step 4: 实现结果计时与关闭**

在 `FileImport.tsx` 中：

```tsx
useEffect(() => {
  if (!results.some((result) => result.status === 'saved')) return
  const timer = window.setTimeout(() => {
    setResults((current) => current.filter((result) => result.status !== 'saved'))
  }, 1000)
  return () => window.clearTimeout(timer)
}, [results])
```

开始新一轮导入前调用 `setResults([])` 和 `setError(null)`。存在非成功结果时显示关闭图标按钮：

```tsx
const hasPersistentResults = results.some((result) => result.status !== 'saved')

<button
  aria-label="关闭导入结果"
  className="icon-button subtle file-import-results-close"
  title="关闭导入结果"
  type="button"
  onClick={() => setResults([])}
>
  <X aria-hidden="true" size={14} />
</button>
```

整体调用错误旁也提供关闭按钮，点击后 `setError(null)`。

- [ ] **Step 5: 补充关闭按钮样式并运行测试**

在 `global.css` 中增加异常结果标题行和错误关闭按钮布局，运行：

```bash
cd desktop
npm test -- src/features/capture/FileImport.test.tsx
```

Expected: PASS。

### Task 2: 导入入口格式提示

**Files:**
- Modify: `desktop/src/features/capture/CaptureMenu.tsx`
- Test: `desktop/src/features/capture/CaptureMenu.test.tsx`
- Modify: `desktop/src/shared/styles/global.css`

- [ ] **Step 1: 编写格式提示失败测试**

```tsx
it('shows the supported file formats below the import action', async () => {
  const user = userEvent.setup()
  render(<CaptureMenu onClipboard={vi.fn()} onImport={vi.fn().mockResolvedValue([])} />)

  await user.click(screen.getByRole('button', { name: '更多捕获方式' }))
  expect(screen.getByText('支持 PDF、DOCX、PPTX、XLSX、MD、TXT、图片及音视频')).toBeInTheDocument()
})
```

- [ ] **Step 2: 运行测试确认失败**

Run:

```bash
cd desktop
npm test -- src/features/capture/CaptureMenu.test.tsx
```

Expected: FAIL，菜单中没有格式提示。

- [ ] **Step 3: 实现格式提示**

在导入按钮中保留 `aria-label="导入本地文件"`，使用两层文案：

```tsx
<span className="capture-menu-item-copy">
  <span>导入本地文件</span>
  <small>支持 PDF、DOCX、PPTX、XLSX、MD、TXT、图片及音视频</small>
</span>
```

将菜单宽度调整为 280px，并让提示使用次级小号文字自然换行。

- [ ] **Step 4: 运行菜单测试**

Run:

```bash
cd desktop
npm test -- src/features/capture/CaptureMenu.test.tsx
```

Expected: PASS。

### Task 3: 删除剪贴板原图提示

**Files:**
- Modify: `desktop/src/features/capture/CaptureDialog.tsx`
- Test: `desktop/src/features/capture/CaptureDialog.test.tsx`
- Modify: `desktop/src/shared/styles/global.css`

- [ ] **Step 1: 编写失败测试**

在现有图片附件保存测试中增加：

```tsx
expect(screen.getByText('图片材料')).toBeInTheDocument()
expect(screen.queryByText('已附原图')).not.toBeInTheDocument()
```

- [ ] **Step 2: 运行测试确认失败**

Run:

```bash
cd desktop
npm test -- src/features/capture/CaptureDialog.test.tsx
```

Expected: FAIL，页面仍显示“已附原图”。

- [ ] **Step 3: 删除提示**

- 删除 `Image` 图标导入。
- 删除基于 `temporaryAttachmentIds.length` 渲染的 `<small>已附原图</small>`。
- 保留 `temporaryAttachmentIds` 对保存条件和 `onSave` 数据的作用。
- 清理 `.capture-source-row small` 样式。

- [ ] **Step 4: 运行捕获弹窗测试**

Run:

```bash
cd desktop
npm test -- src/features/capture/CaptureDialog.test.tsx
```

Expected: PASS。

### Task 4: 完整验证与桌面构建

**Files:**
- Verify: `desktop/e2e/inbox-layout.spec.ts`

- [ ] **Step 1: 运行前端完整验证**

```bash
cd desktop
npm run lint
npm run build
npm test -- --run
npm run test:e2e -- e2e/inbox-layout.spec.ts
```

Expected: lint、build、全部单元测试和 3 个界面测试通过。

- [ ] **Step 2: 检查菜单截图**

查看 `/tmp/ideacard-capture-menu-1100.png`，确认：

- 格式提示完整显示。
- 菜单不遮挡工具栏或越出窗口。
- 图标、标题和提示文字没有重叠。

- [ ] **Step 3: 重建 Mac App**

```bash
cd desktop
npm run tauri build -- --debug
```

Expected: 生成 `desktop/src-tauri/target/debug/bundle/macos/IdeaCard.app`。
