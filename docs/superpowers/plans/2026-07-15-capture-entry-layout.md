# 捕获入口布局调整实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**目标：** 把剪贴板捕获和文件导入从来源筛选栏迁移到顶部“更多捕获方式”菜单，同时保留现有捕获流程和导入结果展示。

**架构：** `CaptureMenu` 只负责菜单开关、关闭方式和动作分发；`FileImport` 继续拥有导入状态、错误和逐文件结果，但通过渲染函数把触发入口交给菜单。`InboxPage` 只组装现有桌面服务与两个界面组件，不新增后端命令。

**技术栈：** React 19、TypeScript、Testing Library、Vitest、Playwright、Tauri 桌面服务接口、现有 CSS 变量。

---

## 文件结构

- 创建 `desktop/src/features/capture/CaptureMenu.tsx`：顶部捕获菜单及其关闭交互。
- 创建 `desktop/src/features/capture/CaptureMenu.test.tsx`：菜单、剪贴板和文件导入行为测试。
- 修改 `desktop/src/features/capture/FileImport.tsx`：把固定按钮改为可注入触发器，保留状态与结果浮层。
- 修改 `desktop/src/features/capture/FileImport.test.tsx`：验证自定义触发器与结果持久显示。
- 修改 `desktop/src/pages/InboxPage.tsx`：来源栏移除捕获按钮，工具栏接入 `CaptureMenu`。
- 修改 `desktop/src/pages/InboxPage.test.tsx`：验证来源栏纯筛选和顶部捕获入口。
- 修改 `desktop/src/shared/styles/global.css`：新增捕获菜单样式并约束最小宽度。
- 修改 `desktop/e2e/inbox-layout.spec.ts`：覆盖 1280×800 和 1100×700 的菜单布局。

### 任务 1：让文件导入支持菜单触发器

**文件：**
- 修改：`desktop/src/features/capture/FileImport.tsx`
- 测试：`desktop/src/features/capture/FileImport.test.tsx`

- [x] **步骤 1：编写失败测试**

把现有固定“导入文件”按钮测试改成渲染函数测试，断言自定义菜单项可以启动导入，触发器消失后结果仍保留：

```tsx
render(
  <FileImport onImport={onImport}>
    {({ startImport, isImporting }) => (
      <button disabled={isImporting} type="button" onClick={() => void startImport()}>
        导入本地文件
      </button>
    )}
  </FileImport>,
)

await user.click(screen.getByRole('button', { name: '导入本地文件' }))
expect(await screen.findByText('方案.pdf')).toBeInTheDocument()
expect(screen.getByText('导入成功')).toBeInTheDocument()
```

- [x] **步骤 2：运行测试并确认失败**

运行：

```bash
npm test -- src/features/capture/FileImport.test.tsx
```

预期：TypeScript 或渲染断言失败，因为 `FileImport` 尚不接受 `children` 渲染函数。

- [x] **步骤 3：实现最小渲染函数接口**

在 `FileImport.tsx` 中定义控制接口并替换固定按钮：

```tsx
import { AlertCircle, Check } from 'lucide-react'
import { useState, type ReactNode } from 'react'

interface FileImportControl {
  isImporting: boolean
  startImport(): Promise<void>
}

interface FileImportProps {
  onImport(): Promise<FileImportResult[]>
  children(control: FileImportControl): ReactNode
}

export function FileImport({ children, onImport }: FileImportProps) {
  const [results, setResults] = useState<FileImportResult[]>([])
  const [isImporting, setIsImporting] = useState(false)
  const [error, setError] = useState<DesktopError | null>(null)

  async function startImport() {
    if (isImporting) return
    setError(null)
    setIsImporting(true)
    try {
      setResults(await onImport())
    } catch (nextError) {
      setError(DesktopError.fromUnknown(nextError))
    } finally {
      setIsImporting(false)
    }
  }

  return (
    <div className="file-import-control">
      {children({ isImporting, startImport })}
      {error ? (
        <div className="inline-alert is-error" role="alert">
          <AlertCircle aria-hidden="true" size={15} />
          <strong>{error.message}</strong>
        </div>
      ) : null}
      {results.length > 0 ? (
        <div aria-label="文件导入结果" className="file-import-results" role="status">
          {results.map((result, index) => (
            <div className={`file-import-result is-${result.status}`} key={`${result.originalName}-${index}`}>
              {result.status === 'saved' ? <Check aria-hidden="true" size={14} /> : <AlertCircle aria-hidden="true" size={14} />}
              <strong>{result.originalName}</strong>
              <span>{resultLabel(result)}</span>
            </div>
          ))}
        </div>
      ) : null}
    </div>
  )
}
```

结果浮层仍放在 `.file-import-control` 内，不能放入捕获菜单条件分支。

- [x] **步骤 4：运行测试并确认通过**

运行：

```bash
npm test -- src/features/capture/FileImport.test.tsx
```

预期：文件导入测试全部通过。

- [x] **步骤 5：提交文件导入接口调整**

```bash
git add desktop/src/features/capture/FileImport.tsx desktop/src/features/capture/FileImport.test.tsx
git commit -m "refactor: expose file import trigger"
```

### 任务 2：实现顶部捕获菜单

**文件：**
- 创建：`desktop/src/features/capture/CaptureMenu.tsx`
- 创建：`desktop/src/features/capture/CaptureMenu.test.tsx`

- [x] **步骤 1：编写菜单失败测试**

覆盖打开、动作分发、`Esc`、点击外部和导入结果五种行为：

```tsx
it('opens capture actions and closes after clipboard capture', async () => {
  const user = userEvent.setup()
  const onClipboard = vi.fn().mockResolvedValue(undefined)
  render(<CaptureMenu onClipboard={onClipboard} onImport={vi.fn().mockResolvedValue([])} />)

  await user.click(screen.getByRole('button', { name: '更多捕获方式' }))
  await user.click(screen.getByRole('menuitem', { name: '从剪贴板捕获' }))

  expect(onClipboard).toHaveBeenCalledTimes(1)
  expect(screen.queryByRole('menu')).not.toBeInTheDocument()
})

it('closes with Escape and an outside click', async () => {
  const user = userEvent.setup()
  render(
    <div>
      <CaptureMenu onClipboard={vi.fn()} onImport={vi.fn().mockResolvedValue([])} />
      <button type="button">外部区域</button>
    </div>,
  )

  await user.click(screen.getByRole('button', { name: '更多捕获方式' }))
  await user.keyboard('{Escape}')
  expect(screen.queryByRole('menu')).not.toBeInTheDocument()

  await user.click(screen.getByRole('button', { name: '更多捕获方式' }))
  await user.click(screen.getByRole('button', { name: '外部区域' }))
  expect(screen.queryByRole('menu')).not.toBeInTheDocument()
})
```

- [x] **步骤 2：运行测试并确认失败**

运行：

```bash
npm test -- src/features/capture/CaptureMenu.test.tsx
```

预期：失败，因为 `CaptureMenu` 尚不存在。

- [x] **步骤 3：实现菜单组件**

组件使用现有 Lucide 图标，不引入新依赖：

```tsx
import { ClipboardPaste, FileUp, LoaderCircle, Plus } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'

import type { FileImportResult } from '../../services/desktop/contracts'
import { FileImport } from './FileImport'

interface CaptureMenuProps {
  disabled?: boolean
  onClipboard(): Promise<void> | void
  onImport(): Promise<FileImportResult[]>
}

export function CaptureMenu({ disabled = false, onClipboard, onImport }: CaptureMenuProps) {
  const [isOpen, setIsOpen] = useState(false)
  const rootRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (disabled) setIsOpen(false)
  }, [disabled])

  useEffect(() => {
    if (!isOpen) return
    const closeOnKey = (event: KeyboardEvent) => {
      if (event.key === 'Escape') setIsOpen(false)
    }
    const closeOutside = (event: MouseEvent) => {
      if (!rootRef.current?.contains(event.target as Node)) setIsOpen(false)
    }
    document.addEventListener('keydown', closeOnKey)
    document.addEventListener('mousedown', closeOutside)
    return () => {
      document.removeEventListener('keydown', closeOnKey)
      document.removeEventListener('mousedown', closeOutside)
    }
  }, [isOpen])

  return (
    <div className="capture-menu-control" ref={rootRef}>
      <FileImport onImport={onImport}>
        {({ isImporting, startImport }) => (
          <>
            <button
              aria-expanded={isOpen}
              aria-haspopup="menu"
              aria-label="更多捕获方式"
              className="icon-button"
              disabled={disabled || isImporting}
              title="更多捕获方式"
              type="button"
              onClick={() => setIsOpen((current) => !current)}
            >
              {isImporting ? <LoaderCircle className="is-spinning" size={16} /> : <Plus size={16} />}
            </button>
            {isOpen ? (
              <div className="capture-menu-popover" role="menu">
                <button
                  className="capture-menu-item"
                  role="menuitem"
                  type="button"
                  onClick={() => {
                    setIsOpen(false)
                    void onClipboard()
                  }}
                >
                  <ClipboardPaste aria-hidden="true" size={15} />
                  从剪贴板捕获
                </button>
                <button
                  className="capture-menu-item"
                  role="menuitem"
                  type="button"
                  onClick={() => {
                    setIsOpen(false)
                    void startImport()
                  }}
                >
                  <FileUp aria-hidden="true" size={15} />
                  导入本地文件
                </button>
              </div>
            ) : null}
          </>
        )}
      </FileImport>
    </div>
  )
}
```

剪贴板菜单项先 `setIsOpen(false)` 再调用 `onClipboard()`；文件菜单项先收起菜单再调用 `void startImport()`。

- [x] **步骤 4：运行测试并确认通过**

运行：

```bash
npm test -- src/features/capture/CaptureMenu.test.tsx src/features/capture/FileImport.test.tsx
```

预期：两个测试文件全部通过，无未处理 Promise 错误。

- [x] **步骤 5：提交捕获菜单**

```bash
git add desktop/src/features/capture/CaptureMenu.tsx desktop/src/features/capture/CaptureMenu.test.tsx
git commit -m "feat: add compact capture menu"
```

### 任务 3：迁移入口并验证桌面布局

**文件：**
- 修改：`desktop/src/pages/InboxPage.tsx`
- 修改：`desktop/src/pages/InboxPage.test.tsx`
- 修改：`desktop/src/shared/styles/global.css`
- 修改：`desktop/e2e/inbox-layout.spec.ts`

- [x] **步骤 1：编写页面集成失败测试**

给来源区域添加可定位名称，并断言来源栏没有捕获动作、顶部菜单可以调用现有服务：

```tsx
const sourceFilter = within(screen.getByRole('region', { name: '来源' }))
expect(sourceFilter.queryByRole('button', { name: '剪贴板' })).not.toBeInTheDocument()
expect(sourceFilter.queryByRole('button', { name: '导入文件' })).not.toBeInTheDocument()

await user.click(screen.getByRole('button', { name: '更多捕获方式' }))
expect(screen.getByRole('menuitem', { name: '从剪贴板捕获' })).toBeInTheDocument()
expect(screen.getByRole('menuitem', { name: '导入本地文件' })).toBeInTheDocument()
```

剪贴板测试为 `readClipboardDraft` 返回固定草稿，并断言点击菜单项后出现“确认捕获”对话框。

- [x] **步骤 2：运行页面测试并确认失败**

运行：

```bash
npm test -- src/pages/InboxPage.test.tsx
```

预期：失败，因为捕获按钮仍在来源栏，顶部菜单尚未接入。

- [x] **步骤 3：迁移 `InboxPage` 入口**

删除来源栏中的剪贴板按钮和 `FileImport`，在刷新按钮与快速记录按钮之间加入：

```tsx
<CaptureMenu
  disabled={quickCaptureOpen || captureDraft !== null}
  onClipboard={async () => {
    setCaptureDraft(await service.readClipboardDraft())
  }}
  onImport={async () => {
    const results = await service.selectAndImportFiles()
    const selected = results.find((result) => result.card)?.card
    await loadCards(service)
    if (selected) selectCard(selected.id)
    return results
  }}
/>
```

来源 `<section>` 增加 `aria-label="来源"`，保留全部六个筛选按钮原有顺序。

- [x] **步骤 4：实现菜单样式**

在 `global.css` 中增加：

```css
.capture-menu-control,
.file-import-control {
  position: relative;
  flex: 0 0 auto;
}

.capture-menu-popover {
  position: absolute;
  z-index: 30;
  top: calc(100% + 8px);
  right: 0;
  width: 184px;
  padding: 5px;
  border: 1px solid var(--border-strong);
  border-radius: 7px;
  background: var(--surface);
  box-shadow: 0 14px 34px rgba(26, 31, 27, 0.16);
}

.capture-menu-item {
  display: flex;
  width: 100%;
  min-height: 34px;
  align-items: center;
  gap: 9px;
  padding: 0 9px;
  border: 0;
  border-radius: 4px;
  background: transparent;
  color: var(--text);
  font-size: 11px;
  text-align: left;
}

.capture-menu-item:hover {
  background: var(--surface-muted);
}
```

将 `.file-import-results` 保持在 `.file-import-control` 下方并右对齐，确保菜单关闭后仍显示。

- [x] **步骤 5：扩展端到端布局测试**

在 `inbox-layout.spec.ts` 的假 Tauri 服务中允许 `read_clipboard_draft` 和 `import_files` 命令，并新增：

```ts
test('keeps capture actions out of sources and opens the compact menu', async ({ page }) => {
  await page.setViewportSize({ width: 1100, height: 700 })
  await page.goto('/')

  await expect(page.getByRole('button', { name: '更多捕获方式' })).toBeVisible()
  await page.getByRole('button', { name: '更多捕获方式' }).click()
  await expect(page.getByRole('menuitem', { name: '从剪贴板捕获' })).toBeVisible()
  await expect(page.getByRole('menuitem', { name: '导入本地文件' })).toBeVisible()
  await expectNoHorizontalOverflow(page)
})
```

- [x] **步骤 6：运行目标测试**

运行：

```bash
npm test -- src/features/capture/FileImport.test.tsx src/features/capture/CaptureMenu.test.tsx src/pages/InboxPage.test.tsx
npm run test:e2e
```

预期：目标单元测试和三个端到端测试全部通过。

- [x] **步骤 7：运行完整验证**

依次运行：

```bash
npm test
npm run lint
npm run build
npm run test:rust
npm run test:native
npm run test:e2e
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
```

预期：所有命令退出码为 0，不新增警告。

- [x] **步骤 8：提交布局迁移**

```bash
git add desktop/src/pages/InboxPage.tsx desktop/src/pages/InboxPage.test.tsx desktop/src/shared/styles/global.css desktop/e2e/inbox-layout.spec.ts
git commit -m "fix: move capture actions into toolbar menu"
```

- [x] **步骤 9：构建并检查真实应用**

运行：

```bash
npm run tauri build -- --bundles app
```

启动 `desktop/src-tauri/target/release/bundle/macos/IdeaCard.app`，在主窗口确认来源栏纯筛选、加号菜单位置、剪贴板确认框和原生文件选择器均正常。
