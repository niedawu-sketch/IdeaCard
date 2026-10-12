# 来源分类与标签简化 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在第一阶段原型中启用五类固定来源筛选，并从所有用户界面和新建流程中移除标签能力，同时兼容已有浏览器本地数据。

**Architecture:** 为卡片新增用户可见的 `sourceType`，现有 `type` 继续表达捕获技术类型，避免把剪贴板和 OCR 当成来源分类。来源类型、中文名称和旧类型映射集中在卡片实体目录；持久化数据在读取时补齐缺失字段，不删除或覆盖旧标签。Inbox 只按固定来源类型筛选，卡片详情展示来源类型与具体来源名称。

**Tech Stack:** React 19、TypeScript 5.9、Zustand 5、Vitest、Testing Library、Vite

---

## 文件结构

- `docs/superpowers/specs/2026-06-10-ideacard-mvp-design.md`：同步第一阶段主规格中的来源与标签表述。
- `prototype/src/entities/card/model.ts`：声明 `SourceType`，将 `sourceType` 纳入卡片和新建输入；`tags` 仅保留在持久化卡片结构中。
- `prototype/src/entities/card/source.ts`：集中维护五类来源、中文名称和旧卡片类型映射。
- `prototype/src/entities/card/source.test.ts`：验证来源类型固定为五类，并验证旧卡片映射。
- `prototype/src/store/seed.ts`：为示例卡片补充来源类型，标签数组仅用于旧数据兼容。
- `prototype/src/store/useIdeaCardStore.ts`：新卡片固定写入空标签；读取旧数据时补齐 `sourceType` 并保留已有标签。
- `prototype/src/store/useIdeaCardStore.test.ts`：验证新卡片来源类型及旧本地数据兼容。
- `prototype/src/features/capture/captureAdapter.ts`：让三种模拟捕获自动产生来源类型，不再处理标签输入。
- `prototype/src/features/capture/captureAdapter.test.ts`：验证捕获类型到来源类型的自动映射。
- `prototype/src/features/capture/CaptureDialog.tsx`：构建新卡片时不再复制标签。
- `prototype/src/features/capture/CaptureDialog.test.tsx`：验证保存卡片的来源类型。
- `prototype/src/features/inbox/CardDetail.tsx`：移除标签编辑，展示来源类型和具体来源。
- `prototype/src/pages/InboxPage.tsx`：移除标签筛选，改为按固定来源类型筛选。
- `prototype/src/pages/InboxPage.test.tsx`：验证五类来源、分类筛选和标签完全不可见。
- `prototype/src/features/analysis/AnalysisResult.test.tsx`：为手工构造的卡片测试数据补充 `sourceType`。

## 实现约束

- 不新增依赖。
- 不删除 `Card.tags`，不迁移、不清空已有标签数据。
- `NewCardInput` 和 `CardUpdateInput` 不再暴露标签写入能力。
- `sourceType` 使用稳定英文值：`personal`、`web`、`document`、`image`、`media`。
- 来源筛选显示全部五类，即使某一类当前没有示例卡片。
- `sourceName` 继续保存文件名、网页标题或捕获入口名称；它不再决定筛选分类。

### Task 1：同步第一阶段主规格

**Files:**
- Modify: `docs/superpowers/specs/2026-06-10-ideacard-mvp-design.md:70-170`
- Test: `docs/superpowers/specs/2026-06-10-ideacard-mvp-design.md`

- [ ] **Step 1: 更新卡片字段与 Inbox 行为**

将卡片字段中的“标签”替换为“来源类型”，并明确来源类型为个人记录、网页文章、文档资料、图片材料、音视频。将 Inbox 描述改为“搜索、按来源筛选、补充想法、修改状态、批量选择和归档”。

- [ ] **Step 2: 更新信息架构与关键流程**

将左栏改为只包含来源筛选，将右栏改为“原始内容、来源信息、我的想法、状态和操作”；从 Inbox 处理流程和验收标准中删除“添加标签”。将“跨来源、标签和时间选择卡片”改为“跨来源和时间选择卡片”。

- [ ] **Step 3: 验证主规格与专项设计一致**

Run:

```bash
rg -n "标签|个人记录|网页文章|文档资料|图片材料|音视频" docs/superpowers/specs/2026-06-10-ideacard-mvp-design.md docs/superpowers/specs/2026-07-14-source-and-tag-simplification-design.md
git diff --check -- docs/superpowers/specs
```

Expected: 主规格不再把标签描述为第一阶段能力；五类来源与专项设计一致；`git diff --check` 退出码为 0。

- [ ] **Step 4: 提交文档同步**

```bash
git add docs/superpowers/specs/2026-06-10-ideacard-mvp-design.md
git commit -m "docs: align MVP with source taxonomy"
```

### Task 2：建立来源类型与旧数据兼容

**Files:**
- Create: `prototype/src/entities/card/source.ts`
- Create: `prototype/src/entities/card/source.test.ts`
- Modify: `prototype/src/entities/card/model.ts:1-41`
- Modify: `prototype/src/store/seed.ts:1-82`
- Modify: `prototype/src/store/useIdeaCardStore.ts:8-88,235-239`
- Modify: `prototype/src/store/useIdeaCardStore.test.ts:17-119,460-520`
- Modify: `prototype/src/features/analysis/AnalysisResult.test.tsx:8-35`

- [ ] **Step 1: 编写来源类型和旧类型映射测试**

创建 `prototype/src/entities/card/source.test.ts`：

```ts
import { describe, expect, it } from 'vitest'
import {
  getSourceTypeLabel,
  inferLegacySourceType,
  sourceTypeOptions,
} from './source'

describe('card source taxonomy', () => {
  it('defines exactly the five approved source types', () => {
    expect(sourceTypeOptions).toEqual([
      { value: 'personal', label: '个人记录' },
      { value: 'web', label: '网页文章' },
      { value: 'document', label: '文档资料' },
      { value: 'image', label: '图片材料' },
      { value: 'media', label: '音视频' },
    ])
    expect(getSourceTypeLabel('document')).toBe('文档资料')
  })

  it.each([
    ['thought', 'personal'],
    ['clipboard', 'personal'],
    ['article', 'web'],
    ['screenshot', 'image'],
  ] as const)('maps legacy %s cards to %s', (cardType, sourceType) => {
    expect(inferLegacySourceType(cardType)).toBe(sourceType)
  })
})
```

- [ ] **Step 2: 运行测试并确认它因模块缺失而失败**

Run:

```bash
cd prototype
npm test -- src/entities/card/source.test.ts
```

Expected: FAIL，提示无法解析 `./source`。

- [ ] **Step 3: 增加来源类型和集中映射**

在 `prototype/src/entities/card/model.ts` 增加：

```ts
export type SourceType =
  | 'personal'
  | 'web'
  | 'document'
  | 'image'
  | 'media'
```

将 `sourceType: SourceType` 加入 `Card`。`NewCardInput` 包含 `sourceType`，但不再包含 `tags`；`CardUpdateInput` 不再包含 `tags`。`Card.tags` 保留为 `string[]`，仅用于旧数据兼容。

创建 `prototype/src/entities/card/source.ts`：

```ts
import type { CardType, SourceType } from './model'

export const sourceTypeOptions = [
  { value: 'personal', label: '个人记录' },
  { value: 'web', label: '网页文章' },
  { value: 'document', label: '文档资料' },
  { value: 'image', label: '图片材料' },
  { value: 'media', label: '音视频' },
] as const satisfies ReadonlyArray<{
  value: SourceType
  label: string
}>

export function getSourceTypeLabel(sourceType: SourceType) {
  return sourceTypeOptions.find((option) => option.value === sourceType)!.label
}

export function inferLegacySourceType(cardType: CardType): SourceType {
  if (cardType === 'article') return 'web'
  if (cardType === 'screenshot') return 'image'
  return 'personal'
}
```

- [ ] **Step 4: 为示例卡片和手工测试数据补充来源类型**

在 `prototype/src/store/seed.ts` 为六张示例卡片补充：

```ts
// card-reading-distance
sourceType: 'web',

// card-capture-friction
sourceType: 'personal',

// card-citation
sourceType: 'personal',

// card-inbox-principle
sourceType: 'image',

// card-reflection-question
sourceType: 'personal',

// card-context-selection
sourceType: 'document',
```

在 `prototype/src/features/analysis/AnalysisResult.test.tsx` 中，来源为“阅读”的文章卡片增加 `sourceType: 'web'`，来源为“快速记录”的想法卡片增加 `sourceType: 'personal'`。

- [ ] **Step 5: 编写旧本地数据兼容测试**

在 `prototype/src/store/useIdeaCardStore.test.ts` 中：

- 所有 `createCard` 输入增加 `sourceType: 'personal'` 并移除 `tags`。
- 删除“copies updated tags before storing them”测试，因为标签不再是更新能力。
- 将持久化测试扩展为以下核心断言：

```ts
it('rehydrates legacy cards without sourceType and preserves old tags', async () => {
  const legacyCard = {
    ...useIdeaCardStore.getState().cards[0],
    tags: ['旧标签'],
  } as Partial<Card>
  delete legacyCard.sourceType

  localStorage.setItem(storageKey, JSON.stringify({
    state: {
      cards: [legacyCard],
      analyses: [],
      insights: [],
      selectedCardIds: [],
    },
    version: 0,
  }))

  await useIdeaCardStore.persist.rehydrate()

  expect(useIdeaCardStore.getState().cards[0]).toMatchObject({
    sourceType: 'web',
    tags: ['旧标签'],
  })
})
```

同时从 `prototype/src/store/useIdeaCardStore.test.ts` 导入 `Card` 类型。

- [ ] **Step 6: 运行测试并确认类型与兼容逻辑尚未实现**

Run:

```bash
cd prototype
npm test -- src/entities/card/source.test.ts src/store/useIdeaCardStore.test.ts src/features/analysis/AnalysisResult.test.tsx
```

Expected: FAIL，失败原因包括 `sourceType` 尚未由 Store 创建或旧数据读取后仍缺少该字段。

- [ ] **Step 7: 实现新卡片默认标签和旧卡片读取归一化**

在 `prototype/src/store/useIdeaCardStore.ts` 中增加：

```ts
import type { Card, CardUpdateInput, NewCardInput, SourceType } from '../entities/card/model'
import { inferLegacySourceType } from '../entities/card/source'

type StoredCard = Omit<Card, 'sourceType' | 'tags'> & {
  sourceType?: SourceType
  tags?: string[]
}

function normalizeStoredCard(card: StoredCard): Card {
  return {
    ...card,
    sourceType: card.sourceType ?? inferLegacySourceType(card.type),
    tags: Array.isArray(card.tags) ? [...card.tags] : [],
  }
}
```

创建卡片时由 Store 写入空标签：

```ts
const card: Card = {
  ...input,
  tags: [],
  id,
  status: 'unprocessed',
  createdAt: now,
  updatedAt: now,
}
```

删除 `updateCard` 中针对 `patch.tags` 的复制分支。在 `persist` 配置中增加读取归一化：

```ts
merge: (persistedState, currentState) => {
  const persisted = persistedState as Partial<IdeaCardState>
  return {
    ...currentState,
    ...persisted,
    cards: (persisted.cards ?? currentState.cards).map((card) =>
      normalizeStoredCard(card as StoredCard),
    ),
  }
},
```

- [ ] **Step 8: 运行来源与 Store 测试**

Run:

```bash
cd prototype
npm test -- src/entities/card/source.test.ts src/store/useIdeaCardStore.test.ts src/features/analysis/AnalysisResult.test.tsx
```

Expected: 3 个测试文件全部 PASS；旧标签保持 `['旧标签']`，旧网页卡片补为 `sourceType: 'web'`。

- [ ] **Step 9: 提交来源模型和兼容逻辑**

```bash
git add prototype/src/entities/card prototype/src/store/seed.ts prototype/src/store/useIdeaCardStore.ts prototype/src/store/useIdeaCardStore.test.ts prototype/src/features/analysis/AnalysisResult.test.tsx
git commit -m "feat: add stable card source types"
```

### Task 3：让捕获流程自动设置来源类型

**Files:**
- Modify: `prototype/src/features/capture/captureAdapter.ts:1-63`
- Modify: `prototype/src/features/capture/captureAdapter.test.ts:1-41`
- Modify: `prototype/src/features/capture/CaptureDialog.tsx:130-155`
- Modify: `prototype/src/features/capture/CaptureDialog.test.tsx:25-180`

- [ ] **Step 1: 为三种捕获方式增加来源类型断言**

在 `prototype/src/features/capture/captureAdapter.test.ts` 中分别断言：

```ts
expect(createEmptyCaptureDraft('quick')).toMatchObject({
  sourceType: 'personal',
})

await expect(simulateCapture('clipboard')).resolves.toMatchObject({
  sourceType: 'personal',
})

expect(createEmptyCaptureDraft('screenshot')).toMatchObject({
  sourceType: 'image',
})
```

补充导入 `createEmptyCaptureDraft`。在 `CaptureDialog.test.tsx` 的快速记录、剪贴板和截图保存断言中分别加入相同的 `sourceType` 期望。

- [ ] **Step 2: 运行捕获测试并确认失败**

Run:

```bash
cd prototype
npm test -- src/features/capture/captureAdapter.test.ts src/features/capture/CaptureDialog.test.tsx
```

Expected: FAIL，草稿或保存卡片中缺少 `sourceType`。

- [ ] **Step 3: 在模拟草稿中自动设置来源类型**

将 `demoDrafts` 完整改为：

```ts
const demoDrafts = {
  quick: {
    title: '',
    content: '',
    note: '',
    type: 'thought',
    sourceType: 'personal',
    sourceName: '快速记录',
    sourceUrl: '',
  },
  clipboard: {
    title: '剪贴板摘录',
    content: 'AI 总结需要引用原始卡片，不能只给无来源结论。',
    note: '',
    type: 'clipboard',
    sourceType: 'personal',
    sourceName: '系统剪贴板',
    sourceUrl: 'ideacard://demo/clipboard',
  },
  screenshot: {
    title: '截图中的产品观点',
    content: '捕获阶段不应要求用户立即决定内容最终归属。',
    note: '',
    type: 'screenshot',
    sourceType: 'image',
    sourceName: '屏幕截图 OCR',
    sourceUrl: 'ideacard://demo/screenshot',
  },
} satisfies Record<CaptureKind, CaptureDraft>
```

从所有草稿删除 `tags`。`simulateCapture` 和 `createEmptyCaptureDraft` 直接复制草稿对象，不再复制标签数组。`CaptureDialog.buildCardInput` 不再返回 `tags`。

- [ ] **Step 4: 运行捕获测试**

Run:

```bash
cd prototype
npm test -- src/features/capture/captureAdapter.test.ts src/features/capture/CaptureDialog.test.tsx
```

Expected: 2 个测试文件全部 PASS；快速记录与剪贴板为个人记录，截图为图片材料。

- [ ] **Step 5: 提交捕获映射**

```bash
git add prototype/src/features/capture
git commit -m "feat: classify captured materials by source"
```

### Task 4：移除标签界面并启用五类来源筛选

**Files:**
- Modify: `prototype/src/pages/InboxPage.test.tsx:25-171`
- Modify: `prototype/src/pages/InboxPage.tsx:1-291`
- Modify: `prototype/src/features/inbox/CardDetail.tsx:1-170`

- [ ] **Step 1: 将 Inbox 测试改为新产品行为**

删除“splits and deduplicates tags when the tag field blurs”测试。新增：

```ts
it('does not expose tag controls in the inbox or card detail', () => {
  renderInbox('/?card=card-reading-distance')

  expect(screen.queryByText('标签')).not.toBeInTheDocument()
  expect(screen.queryByRole('button', { name: '全部标签' })).not.toBeInTheDocument()
})
```

将原“filters cards by source and tag”测试替换为：

```ts
it('shows the five source types and filters cards by source type', async () => {
  const user = userEvent.setup()
  renderInbox()

  for (const label of ['个人记录', '网页文章', '文档资料', '图片材料', '音视频']) {
    expect(screen.getByRole('button', { name: label })).toBeInTheDocument()
  }

  await user.click(screen.getByRole('button', { name: '个人记录' }))
  expect(screen.getByText('捕获动作的摩擦')).toBeInTheDocument()
  expect(screen.getByText('阅读后的思考沉淀')).toBeInTheDocument()
  expect(screen.queryByText('从信息到判断的距离')).not.toBeInTheDocument()

  await user.click(screen.getByRole('button', { name: '文档资料' }))
  expect(screen.getByText('用户决定分析上下文')).toBeInTheDocument()
  expect(screen.queryByText('捕获动作的摩擦')).not.toBeInTheDocument()
})
```

在打开卡片的测试中增加来源详情断言：

```ts
expect(screen.getByText('网页文章')).toBeInTheDocument()
expect(screen.getByText('公众号文章')).toBeInTheDocument()
```

- [ ] **Step 2: 运行 Inbox 测试并确认旧界面导致失败**

Run:

```bash
cd prototype
npm test -- src/pages/InboxPage.test.tsx
```

Expected: FAIL，页面仍显示标签控件，来源按钮仍按 `sourceName` 生成。

- [ ] **Step 3: 改为固定来源类型筛选**

在 `prototype/src/pages/InboxPage.tsx`：

- 将 `sourceFilter` 类型改为 `SourceType | null`。
- 删除 `tagFilter`、`sources`、`tags` 两个动态集合及标签筛选条件。
- 使用 `sourceTypeOptions` 作为固定筛选项。
- 将来源条件改为 `card.sourceType !== sourceFilter`。
- 从 `visibleCards` 依赖中删除 `tagFilter`。
- 删除标签 `FilterGroup`。
- 删除传给 `CardDetail` 的 `onSaveTags`。

`FilterGroupProps` 改为：

```ts
interface FilterGroupProps {
  label: string
  allLabel: string
  items: typeof sourceTypeOptions
  selectedItem: SourceType | null
  onSelect: (item: SourceType | null) => void
}
```

渲染选项时使用 `item.value` 和 `item.label`：

```tsx
{items.map((item) => (
  <button
    aria-pressed={selectedItem === item.value}
    className={selectedItem === item.value ? 'is-selected' : ''}
    key={item.value}
    type="button"
    onClick={() => onSelect(item.value)}
  >
    {item.label}
  </button>
))}
```

- [ ] **Step 4: 从卡片详情移除标签并展示来源类型**

在 `prototype/src/features/inbox/CardDetail.tsx`：

- 从 `CardDetailProps` 删除 `onSaveTags`。
- 删除 `parseTags`、标签本地状态、`saveTags` 和标签输入区域。
- 保留“我的想法”的本地状态。
- 导入 `getSourceTypeLabel`，在来源信息中增加类型。

来源区域改为：

```tsx
<div>
  <dt>类型</dt>
  <dd>{getSourceTypeLabel(card.sourceType)}</dd>
</div>
<div>
  <dt>来源</dt>
  <dd>{card.sourceName}</dd>
</div>
```

地址区域保持现有行为。

- [ ] **Step 5: 运行 Inbox 与卡片列表测试**

Run:

```bash
cd prototype
npm test -- src/pages/InboxPage.test.tsx src/features/inbox/CardList.test.tsx
```

Expected: 2 个测试文件全部 PASS；页面仅显示五类来源筛选，不出现任何标签控件。

- [ ] **Step 6: 提交界面简化**

```bash
git add prototype/src/pages/InboxPage.tsx prototype/src/pages/InboxPage.test.tsx prototype/src/features/inbox/CardDetail.tsx
git commit -m "feat: simplify inbox source filtering"
```

### Task 5：全量验证与浏览器验收

**Files:**
- Verify: `prototype/src/**`
- Verify: `docs/superpowers/specs/**`

- [ ] **Step 1: 运行全部单元与交互测试**

Run:

```bash
cd prototype
npm test
```

Expected: 现有 10 个测试文件加新增 `source.test.ts` 全部 PASS，失败数为 0。

- [ ] **Step 2: 运行代码检查和生产构建**

Run:

```bash
cd prototype
npm run lint
npm run build
```

Expected: 两条命令退出码均为 0；Vite 生成 `dist/`。

- [ ] **Step 3: 检查标签残留和来源类型使用点**

Run:

```bash
rg -n "全部标签|onSaveTags|parseTags|tagFilter" prototype/src
rg -n "sourceType|sourceTypeOptions" prototype/src
```

Expected: 第一条命令无匹配；第二条覆盖实体、Store、捕获、Inbox 和测试。`tags` 只允许出现在 `Card` 兼容字段、Seed、读取归一化和旧数据兼容测试中。

- [ ] **Step 4: 在浏览器验证主要状态**

Run:

```bash
cd prototype
npm run dev -- --host 127.0.0.1 --port 4180
```

使用本地浏览器依次验证：

1. Inbox 左栏显示“全部来源”和五类来源，不显示标签。
2. 点击“个人记录”只显示个人记录卡片。
3. 点击“文档资料”只显示“用户决定分析上下文”。
4. 打开“从信息到判断的距离”，右栏显示类型“网页文章”和来源“公众号文章”，不显示标签输入框。
5. 打开快速记录，保存后新卡片进入 Inbox，来源类型为个人记录。
6. 在 1280×800 和 1620×900 下检查无重叠、截断或空白异常。

Expected: 六项全部成立，浏览器控制台无错误。

- [ ] **Step 5: 检查分支状态和提交记录**

Run:

```bash
git status --short
git log --oneline -5
```

Expected: 工作树只允许出现浏览器验证产生的忽略文件；实现文件均已提交，最近提交对应文档同步、来源模型、捕获映射和 Inbox 简化。
