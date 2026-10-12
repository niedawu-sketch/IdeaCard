# 卡片整理状态 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将卡片状态界面收敛为自动变化的`待整理 / 已整理`，移除用户手动设置状态的入口，同时兼容现有历史状态数据。

**Architecture:** 在卡片领域模型中增加唯一的整理状态映射函数，列表和详情共同使用，避免各自维护状态文案。底层 `CardStatus`、数据库字段和 Rust 自动标注逻辑保持不变；历史 AI 状态映射为`已整理`，归档卡片继续由现有列表查询隐藏。

**Tech Stack:** React、TypeScript、Vitest、Testing Library、Tauri、Rust

---

### Task 1: 建立统一整理状态映射

**Files:**
- Modify: `desktop/src/entities/card/model.ts`
- Test: `desktop/src/entities/card/model.test.ts`

- [ ] **Step 1: 编写失败测试**

在 `model.test.ts` 中导入 `getCardOrganizationStatus`，验证底层状态映射：

```ts
it.each([
  ['unprocessed', { value: 'unprocessed', label: '待整理' }],
  ['annotated', { value: 'organized', label: '已整理' }],
  ['pending_analysis', { value: 'organized', label: '已整理' }],
  ['concluded', { value: 'organized', label: '已整理' }],
  ['archived', { value: 'organized', label: '已整理' }],
] as const)('maps %s to its organization status', (status, expected) => {
  expect(getCardOrganizationStatus(status)).toEqual(expected)
})
```

- [ ] **Step 2: 运行测试确认失败**

Run:

```bash
cd desktop
npm test -- src/entities/card/model.test.ts
```

Expected: FAIL，提示 `getCardOrganizationStatus` 尚未导出。

- [ ] **Step 3: 实现最小映射函数**

在 `model.ts` 中增加：

```ts
export type CardOrganizationStatus = 'unprocessed' | 'organized'

export function getCardOrganizationStatus(status: CardStatus): {
  value: CardOrganizationStatus
  label: '待整理' | '已整理'
} {
  return status === 'unprocessed'
    ? { value: 'unprocessed', label: '待整理' }
    : { value: 'organized', label: '已整理' }
}
```

- [ ] **Step 4: 运行模型测试**

Run:

```bash
cd desktop
npm test -- src/entities/card/model.test.ts
```

Expected: PASS。

### Task 2: 移除手动状态设置并统一界面显示

**Files:**
- Modify: `desktop/src/features/inbox/CardList.tsx`
- Modify: `desktop/src/features/inbox/CardDetail.tsx`
- Modify: `desktop/src/shared/styles/global.css`
- Test: `desktop/src/pages/InboxPage.test.tsx`

- [ ] **Step 1: 编写失败的界面测试**

在 `InboxPage.test.tsx` 增加历史 AI 状态卡片，并验证：

```ts
it('shows only automatic organization statuses and has no status editor', async () => {
  const legacyCards: Card[] = [
    personalCard,
    { ...personalCard, id: 'card-annotated', title: '已整理卡片', status: 'annotated' },
    { ...personalCard, id: 'card-pending', title: '历史待分析卡片', status: 'pending_analysis' },
    { ...personalCard, id: 'card-concluded', title: '历史结论卡片', status: 'concluded' },
  ]
  renderInbox(createService({ listCards: vi.fn().mockResolvedValue(legacyCards) }))
  const cardList = within(screen.getByLabelText('卡片列表'))

  expect(await cardList.findByText('待整理')).toBeInTheDocument()
  expect(cardList.getAllByText('已整理')).toHaveLength(3)
  expect(screen.queryByLabelText('状态')).not.toBeInTheDocument()
  expect(screen.queryByText('待分析')).not.toBeInTheDocument()
  expect(screen.queryByText('已生成结论')).not.toBeInTheDocument()
  expect(screen.queryByText('已归档')).not.toBeInTheDocument()
})
```

并加强已有编辑测试，确认提交补丁不包含用户设置的 `status`：

```ts
expect(service.updateCard).toHaveBeenCalledWith(
  'card-1',
  expect.not.objectContaining({ status: expect.anything() }),
)
```

- [ ] **Step 2: 运行界面测试确认失败**

Run:

```bash
cd desktop
npm test -- src/pages/InboxPage.test.tsx
```

Expected: FAIL，界面仍显示旧状态文案和状态下拉框。

- [ ] **Step 3: 修改列表和详情组件**

在 `CardList.tsx` 中删除本地 `statusLabels`，使用共享映射：

```tsx
const organizationStatus = getCardOrganizationStatus(card.status)

<span className={`status-dot is-${organizationStatus.value}`}>
  {organizationStatus.label}
</span>
```

在 `CardDetail.tsx` 中：

- 删除 `CardStatus` 导入、`statusOptions`、`status` state 和 `patch.status`。
- 使用 `getCardOrganizationStatus(card.status)` 渲染页头只读状态。
- 删除标题旁的状态 `<select>`，标题字段占满整行。

```tsx
const organizationStatus = getCardOrganizationStatus(card.status)

<span className={`status-chip is-${organizationStatus.value}`}>
  {organizationStatus.label}
</span>
```

- [ ] **Step 4: 收敛状态样式**

在 `global.css` 中：

- 把 `.is-annotated` 改为 `.is-organized`。
- 删除 `.is-pending_analysis`、`.is-concluded`、`.is-archived` 的状态点和状态胶囊样式。
- 为详情标题字段添加 `grid-column: 1 / -1`，保证删除下拉框后标题占满整行。

```css
.status-dot.is-organized,
.status-chip.is-organized {
  color: var(--st-annotated);
}

.status-chip.is-organized {
  background: color-mix(in oklch, var(--st-annotated) 12%, transparent);
}

.detail-title-field {
  grid-column: 1 / -1;
}
```

- [ ] **Step 5: 运行界面测试**

Run:

```bash
cd desktop
npm test -- src/pages/InboxPage.test.tsx
```

Expected: PASS。

### Task 3: 验证自动变化和完整回归

**Files:**
- Verify: `desktop/src-tauri/src/repository/card_repository.rs`
- Verify: `desktop/e2e/inbox-layout.spec.ts`

- [ ] **Step 1: 验证后端现有自动变化规则**

Run:

```bash
cd desktop
npm run test:rust -- note_changes_unprocessed_card_to_annotated
```

Expected: PASS，待整理卡片保存非空“我的想法”后自动写入 `annotated`。

- [ ] **Step 2: 运行前端完整验证**

Run:

```bash
cd desktop
npm run lint
npm run build
npm test -- --run
npm run test:e2e -- e2e/inbox-layout.spec.ts
```

Expected: lint、build、全部单元测试和 3 个界面测试通过。

- [ ] **Step 3: 检查标准窗口截图**

查看 `/tmp/ideacard-desktop-1280.png`，确认：

- 列表只出现`待整理 / 已整理`。
- 详情页没有状态下拉框。
- 标题输入框占满原来的整行。
- 状态胶囊和列表状态点未重叠或截断。

- [ ] **Step 4: 重建 Mac App**

Run:

```bash
cd desktop
npm run tauri build -- --debug
```

Expected: 生成 `desktop/src-tauri/target/debug/bundle/macos/IdeaCard.app`。
