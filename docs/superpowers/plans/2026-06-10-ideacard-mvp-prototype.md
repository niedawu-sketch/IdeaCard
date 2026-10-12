# IdeaCard 第一阶段交互原型实施计划

> **供智能执行者使用：** 必须使用 `superpowers:subagent-driven-development`（推荐）或 `superpowers:executing-plans`，按任务逐项实施。所有步骤使用复选框跟踪。

**目标：** 构建一个可在浏览器中完整体验的 Mac 桌面风格原型，验证“统一 Inbox 捕获、卡片处理、多卡片 AI 分析、保存结论”的核心闭环。

**架构：** 原型位于独立的 `prototype/` Vite 应用中，使用 React Router 组织 Inbox、分析和结论页面，使用 Zustand 持有领域状态并持久化到 `localStorage`。系统捕获和 AI 能力通过可控的模拟适配器提供，使正常、失败、取消和重试流程都能稳定复现。

**技术栈：** React 19、TypeScript、Vite 8、React Router 7、Zustand 5、Lucide React、Vitest 4、Testing Library、Playwright。

---

## 文件结构

```text
prototype/
├── package.json
├── vite.config.ts
├── playwright.config.ts
├── src/
│   ├── app/
│   │   ├── App.tsx
│   │   ├── App.test.tsx
│   │   └── router.tsx
│   ├── entities/
│   │   ├── analysis/model.ts
│   │   ├── card/model.ts
│   │   └── insight/model.ts
│   ├── features/
│   │   ├── analysis/
│   │   │   ├── AnalysisResult.tsx
│   │   │   ├── AnalysisResult.test.tsx
│   │   │   ├── mockAnalysisService.ts
│   │   │   └── mockAnalysisService.test.ts
│   │   ├── capture/
│   │   │   ├── CaptureDialog.tsx
│   │   │   ├── CaptureDialog.test.tsx
│   │   │   ├── captureAdapter.ts
│   │   │   └── captureAdapter.test.ts
│   │   └── inbox/
│   │       ├── CardDetail.tsx
│   │       ├── CardList.tsx
│   │       ├── CardList.test.tsx
│   │       └── InboxToolbar.tsx
│   ├── pages/
│   │   ├── AnalysisPage.tsx
│   │   ├── AnalysisPage.test.tsx
│   │   ├── InboxPage.tsx
│   │   ├── InboxPage.test.tsx
│   │   ├── InsightPage.tsx
│   │   └── InsightPage.test.tsx
│   ├── shared/
│   │   ├── lib/id.ts
│   │   ├── styles/global.css
│   │   └── ui/
│   │       ├── Button.tsx
│   │       ├── Dialog.tsx
│   │       ├── EmptyState.tsx
│   │       └── StatusBadge.tsx
│   ├── store/
│   │   ├── seed.ts
│   │   ├── useIdeaCardStore.ts
│   │   └── useIdeaCardStore.test.ts
│   ├── test/setup.ts
│   └── main.tsx
└── tests/
    ├── analysis-flow.spec.ts
    ├── capture-flow.spec.ts
    └── inbox-flow.spec.ts
```

文件边界：

- `entities/*/model.ts` 只定义领域类型，不包含 React 或浏览器 API。
- `store/useIdeaCardStore.ts` 负责所有领域变更和持久化，不在页面组件中直接写 `localStorage`。
- `features/capture/captureAdapter.ts` 负责模拟剪贴板和 OCR。
- `features/analysis/mockAnalysisService.ts` 负责可取消、可失败的模拟 AI。
- `pages/` 只编排功能组件和路由参数，不承载可复用领域逻辑。
- `shared/ui/` 只放无业务含义的基础组件。

### Task 1：建立前端与测试工程

**文件：**
- 创建：`prototype/` Vite 应用
- 修改：`prototype/package.json`
- 修改：`prototype/vite.config.ts`
- 创建：`prototype/src/test/setup.ts`
- 创建：`prototype/playwright.config.ts`

- [ ] **Step 1：生成 React TypeScript 工程**

运行：

```bash
npm create vite@8.3.0 prototype -- --template react-ts
cd prototype
npm install
npm install -D vite@8.0.16
```

预期：生成 `prototype/package.json`，安装过程成功并产生 `package-lock.json`。

- [ ] **Step 2：安装运行和测试依赖**

运行：

```bash
npm install react-router-dom@7.17.0 zustand@5.0.14 lucide-react@1.17.0
npm install -D vitest@4.1.8 jsdom@29.1.1 @testing-library/react@16.3.2 @testing-library/user-event@14.6.1 @testing-library/jest-dom@6.9.1 @playwright/test@1.60.0
npx playwright install chromium
```

预期：所有命令退出码为 0，依赖写入 `prototype/package.json`。

- [ ] **Step 3：配置测试脚本**

将 `prototype/package.json` 的 `scripts` 设为：

```json
{
  "dev": "vite",
  "build": "tsc -b && vite build",
  "lint": "eslint .",
  "test": "vitest run",
  "test:watch": "vitest",
  "test:e2e": "playwright test"
}
```

在 `prototype/vite.config.ts` 配置：

```ts
/// <reference types="vitest/config" />
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  test: {
    environment: "jsdom",
    setupFiles: "./src/test/setup.ts",
    css: true,
  },
});
```

创建 `prototype/src/test/setup.ts`：

```ts
import "@testing-library/jest-dom/vitest";
```

- [ ] **Step 4：配置 Playwright**

创建 `prototype/playwright.config.ts`：

```ts
import { defineConfig, devices } from "@playwright/test";

export default defineConfig({
  testDir: "./tests",
  use: {
    baseURL: "http://127.0.0.1:4173",
    screenshot: "only-on-failure",
    trace: "retain-on-failure",
    viewport: { width: 1440, height: 900 },
  },
  webServer: {
    command: "npm run dev -- --host 127.0.0.1 --port 4173",
    url: "http://127.0.0.1:4173",
    reuseExistingServer: true,
  },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],
});
```

- [ ] **Step 5：运行基础验证**

运行：

```bash
npm run build
npm run lint
npm test
```

预期：三个命令均退出码为 0。

- [ ] **Step 6：提交基础工程**

```bash
git add prototype
git commit -m "chore: scaffold IdeaCard prototype"
```

### Task 2：定义领域模型和持久化 Store

**文件：**
- 创建：`prototype/src/entities/card/model.ts`
- 创建：`prototype/src/entities/analysis/model.ts`
- 创建：`prototype/src/entities/insight/model.ts`
- 创建：`prototype/src/shared/lib/id.ts`
- 创建：`prototype/src/store/seed.ts`
- 创建：`prototype/src/store/useIdeaCardStore.ts`
- 创建：`prototype/src/store/useIdeaCardStore.test.ts`

- [ ] **Step 1：编写 Store 失败测试**

创建 `prototype/src/store/useIdeaCardStore.test.ts`：

```ts
import { beforeEach, describe, expect, it } from "vitest";
import { useIdeaCardStore } from "./useIdeaCardStore";

describe("useIdeaCardStore", () => {
  beforeEach(() => {
    localStorage.clear();
    useIdeaCardStore.getState().reset();
  });

  it("creates a card in inbox and preserves original content separately", () => {
    const id = useIdeaCardStore.getState().createCard({
      title: "临时问题",
      content: "如何缩短记录动作？",
      note: "",
      type: "thought",
      sourceName: "快速记录",
      sourceUrl: "",
      tags: [],
    });

    const card = useIdeaCardStore.getState().cards.find((item) => item.id === id);
    expect(card).toMatchObject({
      content: "如何缩短记录动作？",
      note: "",
      status: "unprocessed",
    });
  });

  it("updates note and marks the card as annotated", () => {
    const id = useIdeaCardStore.getState().cards[0].id;
    useIdeaCardStore.getState().updateCard(id, { note: "需要让入口随时可达" });
    expect(useIdeaCardStore.getState().cards[0].status).toBe("annotated");
  });

  it("creates an analysis set from selected cards", () => {
    const cardIds = useIdeaCardStore.getState().cards.slice(0, 2).map((card) => card.id);
    const analysisId = useIdeaCardStore.getState().createAnalysisSet(cardIds);
    expect(useIdeaCardStore.getState().analyses[0]).toMatchObject({
      id: analysisId,
      cardIds,
      status: "draft",
    });
  });
});
```

- [ ] **Step 2：运行测试并确认失败**

运行：

```bash
npm test -- src/store/useIdeaCardStore.test.ts
```

预期：FAIL，提示无法找到 `./useIdeaCardStore`。

- [ ] **Step 3：定义领域类型**

创建 `prototype/src/entities/card/model.ts`：

```ts
export type CardType = "thought" | "article" | "clipboard" | "screenshot";
export type CardStatus =
  | "unprocessed"
  | "annotated"
  | "pending-analysis"
  | "concluded"
  | "archived";

export interface Card {
  id: string;
  title: string;
  content: string;
  note: string;
  type: CardType;
  sourceName: string;
  sourceUrl: string;
  tags: string[];
  status: CardStatus;
  createdAt: string;
  updatedAt: string;
}

export type NewCardInput = Pick<
  Card,
  "title" | "content" | "note" | "type" | "sourceName" | "sourceUrl" | "tags"
>;
```

创建 `prototype/src/entities/analysis/model.ts`：

```ts
export type AnalysisMode = "themes" | "conflicts" | "actions" | "custom";
export type AnalysisStatus = "draft" | "running" | "completed" | "failed" | "cancelled";

export interface CardReference {
  cardId: string;
  label: string;
}

export interface AnalysisOutput {
  summary: string;
  themes: Array<{ title: string; detail: string; references: CardReference[] }>;
  conflicts: string[];
  actions: string[];
  openQuestions: string[];
}

export interface FollowUp {
  id: string;
  question: string;
  answer: string;
  cardId?: string;
  createdAt: string;
}

export interface AnalysisSet {
  id: string;
  title: string;
  goal: string;
  mode: AnalysisMode;
  cardIds: string[];
  status: AnalysisStatus;
  output?: AnalysisOutput;
  followUps: FollowUp[];
  createdAt: string;
  updatedAt: string;
}
```

创建 `prototype/src/entities/insight/model.ts`：

```ts
import type { AnalysisOutput, FollowUp } from "../analysis/model";

export interface Insight {
  id: string;
  analysisId: string;
  title: string;
  goal: string;
  cardIds: string[];
  output: AnalysisOutput;
  followUps: FollowUp[];
  createdAt: string;
}
```

- [ ] **Step 4：实现 Store 最小行为**

创建 `prototype/src/shared/lib/id.ts`：

```ts
export const createId = (prefix: string) =>
  `${prefix}-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
```

创建 `prototype/src/store/seed.ts`，导出至少 6 张中文演示卡片，覆盖四种 `CardType` 和前三种处理状态：

```ts
import type { Card } from "../entities/card/model";

export const seedCards: Card[] = [
  {
    id: "card-reading-distance",
    title: "从信息到判断的距离",
    content: "产品的核心不是记录更多，而是缩短从信息到判断的距离。",
    note: "需要把记录后的加工动作放在主流程里。",
    type: "article",
    sourceName: "公众号文章",
    sourceUrl: "https://example.com/reading",
    tags: ["产品", "阅读"],
    status: "pending-analysis",
    createdAt: "2026-06-10T01:00:00.000Z",
    updatedAt: "2026-06-10T01:00:00.000Z"
  },
  {
    id: "card-capture-friction",
    title: "捕获动作的摩擦",
    content: "记录入口必须随时可达，但保存时不应要求用户完成分类。",
    note: "菜单栏和全局快捷键都服务于这个目标。",
    type: "thought",
    sourceName: "快速记录",
    sourceUrl: "",
    tags: ["捕获", "交互"],
    status: "annotated",
    createdAt: "2026-06-10T02:00:00.000Z",
    updatedAt: "2026-06-10T02:00:00.000Z"
  },
  {
    id: "card-citation",
    title: "AI 结论需要来源",
    content: "AI 总结需要引用原始卡片，不能只给无来源结论。",
    note: "",
    type: "clipboard",
    sourceName: "系统剪贴板",
    sourceUrl: "",
    tags: ["AI", "可信度"],
    status: "unprocessed",
    createdAt: "2026-06-10T03:00:00.000Z",
    updatedAt: "2026-06-10T03:00:00.000Z"
  },
  {
    id: "card-inbox-principle",
    title: "先收集再整理",
    content: "捕获阶段不应要求用户立即决定内容最终归属。",
    note: "Inbox 应该是待处理区，而不是永久分类。",
    type: "screenshot",
    sourceName: "屏幕截图 OCR",
    sourceUrl: "",
    tags: ["Inbox", "整理"],
    status: "pending-analysis",
    createdAt: "2026-06-10T04:00:00.000Z",
    updatedAt: "2026-06-10T04:00:00.000Z"
  },
  {
    id: "card-reflection-question",
    title: "阅读后的思考沉淀",
    content: "如何让用户愿意在阅读当下补充自己的想法？",
    note: "",
    type: "thought",
    sourceName: "快速记录",
    sourceUrl: "",
    tags: ["阅读", "待讨论"],
    status: "unprocessed",
    createdAt: "2026-06-10T05:00:00.000Z",
    updatedAt: "2026-06-10T05:00:00.000Z"
  },
  {
    id: "card-context-selection",
    title: "用户决定分析上下文",
    content: "相比默认分析整个资料库，主动选择材料更容易控制噪声。",
    note: "分析集是用户意图的显式表达。",
    type: "article",
    sourceName: "产品研究笔记",
    sourceUrl: "https://example.com/context",
    tags: ["分析集", "AI"],
    status: "pending-analysis",
    createdAt: "2026-06-10T06:00:00.000Z",
    updatedAt: "2026-06-10T06:00:00.000Z"
  }
];
```

创建 `prototype/src/store/useIdeaCardStore.ts`，使用 Zustand `persist` 中间件实现：

```ts
import { create } from "zustand";
import { persist } from "zustand/middleware";
import type { AnalysisSet } from "../entities/analysis/model";
import type { Card, NewCardInput } from "../entities/card/model";
import type { Insight } from "../entities/insight/model";
import { createId } from "../shared/lib/id";
import { seedCards } from "./seed";

interface IdeaCardState {
  cards: Card[];
  analyses: AnalysisSet[];
  insights: Insight[];
  selectedCardIds: string[];
  createCard: (input: NewCardInput) => string;
  updateCard: (id: string, patch: Partial<Card>) => void;
  toggleCardSelection: (id: string) => void;
  clearSelection: () => void;
  createAnalysisSet: (cardIds: string[]) => string;
  reset: () => void;
}

const initialState = {
  cards: seedCards,
  analyses: [],
  insights: [],
  selectedCardIds: [],
};

export const useIdeaCardStore = create<IdeaCardState>()(
  persist(
    (set, get) => ({
      ...initialState,
      createCard: (input) => {
        const now = new Date().toISOString();
        const id = createId("card");
        set({ cards: [{ ...input, id, status: "unprocessed", createdAt: now, updatedAt: now }, ...get().cards] });
        return id;
      },
      updateCard: (id, patch) =>
        set({
          cards: get().cards.map((card) => {
            if (card.id !== id) return card;
            const status = patch.note?.trim() && card.status === "unprocessed" ? "annotated" : patch.status ?? card.status;
            return { ...card, ...patch, status, updatedAt: new Date().toISOString() };
          }),
        }),
      toggleCardSelection: (id) =>
        set({
          selectedCardIds: get().selectedCardIds.includes(id)
            ? get().selectedCardIds.filter((cardId) => cardId !== id)
            : [...get().selectedCardIds, id],
        }),
      clearSelection: () => set({ selectedCardIds: [] }),
      createAnalysisSet: (cardIds) => {
        const now = new Date().toISOString();
        const id = createId("analysis");
        const analysis: AnalysisSet = {
          id,
          title: "未命名分析",
          goal: "",
          mode: "themes",
          cardIds,
          status: "draft",
          followUps: [],
          createdAt: now,
          updatedAt: now,
        };
        set({ analyses: [analysis, ...get().analyses], selectedCardIds: [] });
        return id;
      },
      reset: () => set(initialState),
    }),
    { name: "ideacard-prototype-v1" },
  ),
);
```

- [ ] **Step 5：运行测试并确认通过**

运行：

```bash
npm test -- src/store/useIdeaCardStore.test.ts
```

预期：3 个测试全部 PASS。

- [ ] **Step 6：提交领域基础**

```bash
git add prototype/src/entities prototype/src/shared/lib prototype/src/store
git commit -m "feat: add persisted IdeaCard domain store"
```

### Task 3：建立桌面应用壳层和路由

**文件：**
- 修改：`prototype/src/main.tsx`
- 创建：`prototype/src/app/router.tsx`
- 创建：`prototype/src/app/App.tsx`
- 创建：`prototype/src/app/App.test.tsx`
- 创建：`prototype/src/shared/styles/global.css`
- 创建：`prototype/src/shared/ui/Button.tsx`
- 创建：`prototype/src/shared/ui/StatusBadge.tsx`
- 创建：`prototype/src/shared/ui/EmptyState.tsx`
- 创建：`prototype/src/pages/InboxPage.tsx`
- 创建：`prototype/src/pages/AnalysisPage.tsx`
- 创建：`prototype/src/pages/InsightPage.tsx`

- [ ] **Step 1：编写壳层失败测试**

创建 `prototype/src/app/App.test.tsx`：

```tsx
import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { App } from "./App";

it("renders the desktop navigation and inbox outlet", () => {
  render(
    <MemoryRouter>
      <App />
    </MemoryRouter>,
  );
  expect(screen.getByText("IdeaCard")).toBeInTheDocument();
  expect(screen.getByRole("link", { name: "收集箱" })).toBeInTheDocument();
  expect(screen.getByRole("main")).toBeInTheDocument();
});
```

- [ ] **Step 2：运行测试并确认失败**

运行：

```bash
npm test -- src/app/App.test.tsx
```

预期：FAIL，提示无法找到 `./App`。

- [ ] **Step 3：实现基础组件和应用壳层**

`App.tsx` 使用 `NavLink`、`Outlet` 和 Lucide 图标，结构固定为：

```tsx
export function App() {
  return (
    <div className="app-shell">
      <aside className="sidebar" aria-label="主导航">
        <div className="brand">IdeaCard</div>
        <nav>
          <NavLink to="/">收集箱</NavLink>
          <NavLink to="/?status=unprocessed">未处理</NavLink>
          <NavLink to="/?status=pending-analysis">待分析</NavLink>
          <NavLink to="/insights">结论</NavLink>
          <NavLink to="/?status=archived">已归档</NavLink>
        </nav>
      </aside>
      <main>
        <Outlet />
      </main>
    </div>
  );
}
```

`global.css` 定义：

- 1440×900 作为主要设计视口。
- 左栏固定 `220px`，内容区占剩余宽度。
- 字体使用 `Inter, "PingFang SC", "Microsoft YaHei", sans-serif`。
- 背景使用中性浅灰，主文字近黑，强调色使用绿色，状态色同时依赖文字和图标，不只依赖颜色。
- 卡片圆角不超过 `8px`。
- 所有按钮和输入框都有清晰的 `:focus-visible`。

- [ ] **Step 4：建立路由**

创建 `prototype/src/app/router.tsx`：

```tsx
import { createBrowserRouter } from "react-router-dom";
import { App } from "./App";
import { AnalysisPage } from "../pages/AnalysisPage";
import { InboxPage } from "../pages/InboxPage";
import { InsightPage } from "../pages/InsightPage";

export const router = createBrowserRouter([
  {
    path: "/",
    element: <App />,
    children: [
      { index: true, element: <InboxPage /> },
      { path: "analysis/:analysisId", element: <AnalysisPage /> },
      { path: "insights", element: <InsightPage /> },
      { path: "insights/:insightId", element: <InsightPage /> },
    ],
  },
]);
```

将 `main.tsx` 改为渲染 `<RouterProvider router={router} />` 并导入 `global.css`。三个页面先返回带正确标题的 `<section>` 占位结构，不加入业务逻辑。

- [ ] **Step 5：运行验证**

运行：

```bash
npm test -- src/app/App.test.tsx
npm run build
```

预期：测试 PASS，构建成功。

- [ ] **Step 6：提交应用壳层**

```bash
git add prototype/src/app prototype/src/pages prototype/src/shared prototype/src/main.tsx
git commit -m "feat: add desktop shell and application routes"
```

### Task 4：实现 Inbox 列表、筛选和卡片详情

**文件：**
- 创建：`prototype/src/features/inbox/CardList.tsx`
- 创建：`prototype/src/features/inbox/CardList.test.tsx`
- 创建：`prototype/src/features/inbox/CardDetail.tsx`
- 创建：`prototype/src/features/inbox/InboxToolbar.tsx`
- 修改：`prototype/src/pages/InboxPage.tsx`
- 创建：`prototype/src/pages/InboxPage.test.tsx`

- [ ] **Step 1：编写 Inbox 失败测试**

创建 `prototype/src/pages/InboxPage.test.tsx`，覆盖：

```tsx
it("filters cards by search text and opens card detail", async () => {
  const user = userEvent.setup();
  render(<InboxPage />);

  await user.type(screen.getByPlaceholderText("搜索标题、内容或我的想法"), "判断");
  expect(screen.getByText("从信息到判断的距离")).toBeInTheDocument();

  await user.click(screen.getByText("从信息到判断的距离"));
  expect(screen.getByLabelText("我的想法")).toHaveValue("需要把记录后的加工动作放在主流程里。");
});

it("updates note, tags and status from detail panel", async () => {
  const user = userEvent.setup();
  render(<InboxPage />);
  await user.click(screen.getByText("从信息到判断的距离"));
  await user.clear(screen.getByLabelText("我的想法"));
  await user.type(screen.getByLabelText("我的想法"), "引用和判断必须分开");
  await user.click(screen.getByRole("button", { name: "标记待分析" }));
  expect(screen.getByText("待分析")).toBeInTheDocument();
});
```

测试前调用 `useIdeaCardStore.getState().reset()`，避免本地状态串扰。

- [ ] **Step 2：运行测试并确认失败**

运行：

```bash
npm test -- src/pages/InboxPage.test.tsx
```

预期：FAIL，页面中不存在搜索框和卡片详情控件。

- [ ] **Step 3：实现 Inbox 三栏内容区**

`InboxPage.tsx` 在应用左侧主导航之外再提供：

- 中栏顶部搜索框和来源、标签、状态筛选。
- 中栏卡片列表，每项包含复选框、标题、内容摘要、来源、状态和时间。
- 右栏卡片详情，原始内容只读，“我的想法”、标签可编辑。
- 未选中卡片时，右栏显示“选择一张卡片开始处理”。

筛选规则：

```ts
const matchesQuery = [card.title, card.content, card.note]
  .join(" ")
  .toLocaleLowerCase()
  .includes(query.trim().toLocaleLowerCase());
```

状态筛选读取 URL 查询参数 `status`；来源和标签筛选使用页面本地状态。

- [ ] **Step 4：实现编辑和状态动作**

`CardDetail.tsx`：

- 文本框失焦时调用 `updateCard`。
- 标签输入使用中文顿号或英文逗号分割，保存前去空白和去重。
- “标记待分析”写入 `pending-analysis`。
- “归档”写入 `archived`，并清空当前详情选择。
- 原始内容和来源地址不可编辑。

- [ ] **Step 5：运行测试和构建**

运行：

```bash
npm test -- src/pages/InboxPage.test.tsx src/features/inbox/CardList.test.tsx
npm run build
```

预期：Inbox 测试全部 PASS，构建成功。

- [ ] **Step 6：提交 Inbox**

```bash
git add prototype/src/features/inbox prototype/src/pages/InboxPage.tsx prototype/src/pages/InboxPage.test.tsx
git commit -m "feat: build inbox card processing workspace"
```

### Task 5：实现三种模拟捕获流程

**文件：**
- 创建：`prototype/src/features/capture/captureAdapter.ts`
- 创建：`prototype/src/features/capture/captureAdapter.test.ts`
- 创建：`prototype/src/features/capture/CaptureDialog.tsx`
- 创建：`prototype/src/features/capture/CaptureDialog.test.tsx`
- 修改：`prototype/src/features/inbox/InboxToolbar.tsx`
- 修改：`prototype/src/pages/InboxPage.tsx`

- [ ] **Step 1：编写捕获适配器失败测试**

创建 `captureAdapter.test.ts`：

```ts
import { expect, it } from "vitest";
import { simulateCapture } from "./captureAdapter";

it("returns deterministic clipboard content", async () => {
  await expect(simulateCapture("clipboard", "success")).resolves.toMatchObject({
    type: "clipboard",
    sourceName: "系统剪贴板",
  });
});

it("returns a controlled OCR error", async () => {
  await expect(simulateCapture("screenshot", "failure")).rejects.toThrow("未识别到清晰文字");
});
```

- [ ] **Step 2：运行测试并确认失败**

运行：

```bash
npm test -- src/features/capture/captureAdapter.test.ts
```

预期：FAIL，提示无法找到 `simulateCapture`。

- [ ] **Step 3：实现可控捕获适配器**

`captureAdapter.ts` 导出：

```ts
export type CaptureKind = "quick" | "clipboard" | "screenshot";
export type DemoOutcome = "success" | "failure";

export async function simulateCapture(kind: CaptureKind, outcome: DemoOutcome) {
  await new Promise((resolve) => setTimeout(resolve, 450));
  if (kind === "screenshot" && outcome === "failure") {
    throw new Error("未识别到清晰文字");
  }
  if (kind === "clipboard") {
    return {
      title: "剪贴板摘录",
      content: "AI 总结需要引用原始卡片，不能只给无来源结论。",
      note: "",
      type: "clipboard" as const,
      sourceName: "系统剪贴板",
      sourceUrl: "",
      tags: [],
    };
  }
  if (kind === "screenshot") {
    return {
      title: "截图中的产品观点",
      content: "捕获阶段不应要求用户立即决定内容最终归属。",
      note: "",
      type: "screenshot" as const,
      sourceName: "屏幕截图 OCR",
      sourceUrl: "",
      tags: [],
    };
  }
  return {
    title: "",
    content: "",
    note: "",
    type: "thought" as const,
    sourceName: "快速记录",
    sourceUrl: "",
    tags: [],
  };
}
```

- [ ] **Step 4：编写对话框交互测试**

`CaptureDialog.test.tsx` 覆盖：

- 快速记录只填写内容即可保存，标题为空时取内容前 20 个字符。
- 新卡片保存后出现在 Inbox 顶部。
- 来源地址或去除首尾空白后的内容与已有卡片一致时展示重复提示。
- 重复提示提供“返回修改”和“仍然保存”。
- OCR 失败展示错误信息和“重新识别”。
- 对话框关闭后不创建卡片。

- [ ] **Step 5：实现捕获对话框**

`CaptureDialog.tsx` 使用统一表单，根据 `kind` 调整标题和 loading 文案：

- 快速记录：打开后立即聚焦内容输入框。
- 剪贴板：打开后调用成功适配器并填充表单。
- 截图：提供“正常识别”和“演示识别失败”两个演示入口。
- 保存前调用 `isDuplicateCard(cards, draft)`：

```ts
export const isDuplicateCard = (cards: Card[], draft: NewCardInput) =>
  cards.some(
    (card) =>
      Boolean(draft.sourceUrl && card.sourceUrl === draft.sourceUrl) ||
      card.content.trim() === draft.content.trim(),
  );
```

- [ ] **Step 6：运行捕获测试**

运行：

```bash
npm test -- src/features/capture
```

预期：适配器和对话框测试全部 PASS。

- [ ] **Step 7：提交捕获流程**

```bash
git add prototype/src/features/capture prototype/src/features/inbox/InboxToolbar.tsx prototype/src/pages/InboxPage.tsx
git commit -m "feat: add simulated capture workflows"
```

### Task 6：实现卡片多选和分析集创建

**文件：**
- 修改：`prototype/src/store/useIdeaCardStore.ts`
- 修改：`prototype/src/store/useIdeaCardStore.test.ts`
- 修改：`prototype/src/features/inbox/CardList.tsx`
- 修改：`prototype/src/pages/InboxPage.tsx`
- 修改：`prototype/src/pages/InboxPage.test.tsx`

- [ ] **Step 1：增加失败测试**

在 Store 测试中增加：

```ts
it("rejects analysis sets with more than 20 cards", () => {
  const cardIds = Array.from({ length: 21 }, (_, index) => `card-${index}`);
  expect(() => useIdeaCardStore.getState().createAnalysisSet(cardIds)).toThrow(
    "单次分析最多选择 20 张卡片",
  );
});
```

在 Inbox 测试中增加：

```tsx
const mockNavigate = vi.fn();

vi.mock("react-router-dom", async () => {
  const actual = await vi.importActual<typeof import("react-router-dom")>(
    "react-router-dom",
  );
  return {
    ...actual,
    useNavigate: () => mockNavigate,
    useSearchParams: () => [new URLSearchParams(), vi.fn()],
  };
});

it("creates an analysis set from selected cards and navigates to it", async () => {
  const user = userEvent.setup();
  render(<InboxPage />);
  const checkboxes = screen.getAllByRole("checkbox", { name: /选择卡片/ });
  await user.click(checkboxes[0]);
  await user.click(checkboxes[1]);
  expect(screen.getByText("已选择 2 张卡片")).toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: "创建分析集" }));
  expect(mockNavigate).toHaveBeenCalledWith(expect.stringMatching(/^\/analysis\//));
});
```

- [ ] **Step 2：运行测试并确认失败**

运行：

```bash
npm test -- src/store/useIdeaCardStore.test.ts src/pages/InboxPage.test.tsx
```

预期：超过 20 张卡片没有报错，批量操作栏不存在。

- [ ] **Step 3：实现选择和批量操作栏**

- 卡片复选框点击不切换详情选中状态。
- 有选择时，在内容区底部固定显示批量栏。
- 批量栏包含选择数量、“清除选择”和“创建分析集”。
- `createAnalysisSet` 在 `cardIds.length > 20` 时抛出固定错误。
- 成功后导航到 `/analysis/:analysisId`。
- 单卡片也允许创建分析集，但按钮旁显示“单卡片分析”说明。

- [ ] **Step 4：运行测试**

运行：

```bash
npm test -- src/store/useIdeaCardStore.test.ts src/pages/InboxPage.test.tsx
```

预期：新增测试全部 PASS。

- [ ] **Step 5：提交分析集入口**

```bash
git add prototype/src/store prototype/src/features/inbox/CardList.tsx prototype/src/pages/InboxPage.tsx prototype/src/pages/InboxPage.test.tsx
git commit -m "feat: create analysis sets from selected cards"
```

### Task 7：实现 AI 模拟生成和引用结果

**文件：**
- 创建：`prototype/src/features/analysis/mockAnalysisService.ts`
- 创建：`prototype/src/features/analysis/mockAnalysisService.test.ts`
- 创建：`prototype/src/features/analysis/AnalysisResult.tsx`
- 创建：`prototype/src/features/analysis/AnalysisResult.test.tsx`
- 修改：`prototype/src/store/useIdeaCardStore.ts`
- 创建：`prototype/src/pages/AnalysisPage.test.tsx`
- 修改：`prototype/src/pages/AnalysisPage.tsx`

- [ ] **Step 1：编写模拟服务失败测试**

```ts
const cards = seedCards.slice(0, 3);

const createInput = (
  outcome: "success" | "failure",
  signal = new AbortController().signal,
) => ({
  cards,
  goal: "归纳阅读沉淀的关键因素",
  mode: "themes" as const,
  outcome,
  signal,
});

it("returns structured output with valid card references", async () => {
  const output = await runMockAnalysis(createInput("success"));
  expect(output.themes[0].references[0].cardId).toBe(cards[0].id);
});

it("supports controlled failure and cancellation", async () => {
  await expect(runMockAnalysis(createInput("failure"))).rejects.toThrow(
    "分析服务暂时不可用",
  );
  const controller = new AbortController();
  const result = runMockAnalysis(createInput("success", controller.signal));
  controller.abort();
  await expect(result).rejects.toMatchObject({ name: "AbortError" });
});
```

- [ ] **Step 2：运行测试并确认失败**

运行：

```bash
npm test -- src/features/analysis/mockAnalysisService.test.ts
```

预期：FAIL，提示无法找到 `runMockAnalysis`。

- [ ] **Step 3：实现模拟服务**

`runMockAnalysis`：

- 固定等待 1200 毫秒。
- 每 50 毫秒检查一次 `signal.aborted`。
- `outcome === "failure"` 时抛出 `分析服务暂时不可用`。
- 成功结果包含五个固定区块。
- 所有引用的 `cardId` 必须来自输入卡片。
- 不根据随机数改变结果。

- [ ] **Step 4：扩展 Store 的分析动作**

加入：

```ts
updateAnalysis: (id: string, patch: Partial<AnalysisSet>) => void;
setAnalysisOutput: (id: string, output: AnalysisOutput) => void;
```

`setAnalysisOutput` 同时将分析状态写为 `completed`。失败和取消分别写为 `failed`、`cancelled`，但保留 `goal`、`mode` 和 `cardIds`。

- [ ] **Step 5：实现分析工作台**

`AnalysisPage.tsx`：

- 路由参数不存在时显示“未找到分析集”，提供返回 Inbox 的按钮。
- 左栏显示材料卡片，可查看和移除，数量显示在标题中。
- 右栏包含分析标题、分析目标、四种分析模式。
- 目标为空时禁止发起分析。
- 提供“正常生成”和“演示分析失败”。
- running 状态展示进度、取消按钮并禁用材料编辑。
- completed 状态渲染 `AnalysisResult`。
- 点击结果引用，在左栏高亮并滚动到对应卡片。

- [ ] **Step 6：实现结果组件测试**

测试 `AnalysisResult`：

- 展示核心结论、主题、共识与分歧、行动建议和待验证问题。
- 每个引用都是可聚焦按钮，名称来自对应卡片标题。
- 点击引用调用 `onReferenceClick(cardId)`。

- [ ] **Step 7：运行分析测试**

运行：

```bash
npm test -- src/features/analysis src/pages/AnalysisPage.test.tsx
npm run build
```

预期：全部 PASS，构建成功。

- [ ] **Step 8：提交 AI 模拟分析**

```bash
git add prototype/src/features/analysis prototype/src/pages/AnalysisPage.tsx prototype/src/pages/AnalysisPage.test.tsx prototype/src/store
git commit -m "feat: add controllable AI analysis workspace"
```

### Task 8：实现追问、保存结论和来源回链

**文件：**
- 修改：`prototype/src/store/useIdeaCardStore.ts`
- 修改：`prototype/src/store/useIdeaCardStore.test.ts`
- 修改：`prototype/src/pages/AnalysisPage.tsx`
- 修改：`prototype/src/pages/AnalysisPage.test.tsx`
- 创建：`prototype/src/pages/InsightPage.test.tsx`
- 修改：`prototype/src/pages/InsightPage.tsx`

- [ ] **Step 1：编写结论领域失败测试**

```ts
it("adds a follow-up and saves a completed analysis as an insight", () => {
  const cards = seedCards.slice(0, 2);
  const analysis: AnalysisSet = {
    id: "analysis-completed",
    title: "阅读后的思考沉淀",
    goal: "归纳最值得验证的产品机会",
    mode: "actions",
    cardIds: cards.map((card) => card.id),
    status: "completed",
    output: {
      summary: "低摩擦捕获和后续回访缺一不可。",
      themes: [
        {
          title: "捕获与加工",
          detail: "捕获应快，判断应可追溯。",
          references: [{ cardId: cards[0].id, label: cards[0].title }],
        },
      ],
      conflicts: ["即时整理与稍后整理存在操作成本冲突。"],
      actions: ["验证用户在记录后 24 小时内的回访率。"],
      openQuestions: ["什么提醒方式不会造成打扰？"],
    },
    followUps: [],
    createdAt: "2026-06-10T08:00:00.000Z",
    updatedAt: "2026-06-10T08:00:00.000Z",
  };
  useIdeaCardStore.setState({ cards, analyses: [analysis], insights: [] });
  useIdeaCardStore.getState().addFollowUp(analysis.id, {
    question: "最值得优先验证的建议是什么？",
    answer: "优先验证快速记录后的回访处理率。",
  });
  const insightId = useIdeaCardStore.getState().saveInsight(analysis.id);
  expect(useIdeaCardStore.getState().insights[0].id).toBe(insightId);
  expect(
    useIdeaCardStore.getState().cards.every((card) => card.status === "concluded"),
  ).toBe(true);
});
```

- [ ] **Step 2：运行测试并确认失败**

运行：

```bash
npm test -- src/store/useIdeaCardStore.test.ts
```

预期：FAIL，`addFollowUp` 和 `saveInsight` 不存在。

- [ ] **Step 3：实现追问与保存动作**

Store 新增：

```ts
addFollowUp: (
  analysisId: string,
  input: { question: string; answer: string; cardId?: string },
) => void;
saveInsight: (analysisId: string) => string;
```

保存规则：

- 只有 `completed` 且存在 `output` 的分析可保存。
- Insight 复制分析目标、卡片 ID、输出和追问，避免之后编辑分析导致历史结论变化。
- 参与分析的卡片状态统一改为 `concluded`。
- 重复点击保存返回已有 Insight ID，不重复创建。

- [ ] **Step 4：实现追问交互**

分析页完成态底部提供：

- 追问输入框。
- 可选“引用某张材料卡片”菜单。
- 提交后等待 500 毫秒，追加固定回答。
- 空问题不可提交。
- “保存为结论”成功后导航至 `/insights/:insightId`。

- [ ] **Step 5：实现结论页**

列表路由 `/insights`：

- 显示所有结论标题、分析目标、来源卡片数量和创建时间。
- 无结论时显示明确空状态和返回 Inbox 的操作。

详情路由 `/insights/:insightId`：

- 使用与 `AnalysisResult` 相同的结果结构。
- 展示追问记录。
- 展示来源卡片列表。
- 点击来源卡片导航到 `/?card=:cardId`，Inbox 根据 URL 自动打开对应详情。
- 提供“基于原分析集继续分析”，导航回原分析页。

- [ ] **Step 6：运行结论测试**

运行：

```bash
npm test -- src/store/useIdeaCardStore.test.ts src/pages/AnalysisPage.test.tsx src/pages/InsightPage.test.tsx
```

预期：全部 PASS。

- [ ] **Step 7：提交结论闭环**

```bash
git add prototype/src/store prototype/src/pages
git commit -m "feat: save cited analysis as reusable insights"
```

### Task 9：补齐空状态、失败状态和可访问性

**文件：**
- 修改：`prototype/src/shared/ui/EmptyState.tsx`
- 创建：`prototype/src/shared/ui/Dialog.tsx`
- 修改：`prototype/src/shared/styles/global.css`
- 修改：`prototype/src/pages/InboxPage.test.tsx`
- 修改：`prototype/src/pages/AnalysisPage.test.tsx`
- 修改：`prototype/src/pages/InsightPage.test.tsx`
- 修改：涉及状态展示的页面和功能组件

- [ ] **Step 1：增加异常状态失败测试**

覆盖以下可重复状态：

- cards 为空时 Inbox 显示“收集箱还是空的”和“快速记录”。
- 搜索无结果时显示“没有匹配的卡片”，保留清除筛选操作。
- OCR 失败后对话框不关闭。
- 21 张材料阻止分析并显示固定限制文案。
- 分析失败后显示“重试”，点击后回到可发起状态。
- 分析取消后显示“已取消，本次材料和目标已保留”。
- 找不到分析或结论 ID 时显示恢复入口。

- [ ] **Step 2：运行测试并确认失败**

运行：

```bash
npm test
```

预期：新增异常状态断言至少有一项 FAIL。

- [ ] **Step 3：实现统一反馈模式**

- `EmptyState` 接受 `title`、`description` 和一个主操作。
- `Dialog` 使用原生 `<dialog>`，打开时聚焦标题后的第一个可交互元素，关闭后恢复触发按钮焦点。
- 错误信息使用 `role="alert"`。
- 生成状态使用 `aria-live="polite"`。
- 图标按钮必须有 `aria-label` 和 tooltip。
- 所有复选框、输入框、按钮具备可访问名称。
- 键盘可完成卡片选择、打开详情、捕获、发起分析和保存结论。

- [ ] **Step 4：执行测试和静态检查**

运行：

```bash
npm test
npm run lint
npm run build
```

预期：全部退出码为 0。

- [ ] **Step 5：提交异常状态和可访问性**

```bash
git add prototype/src
git commit -m "feat: complete prototype states and accessibility"
```

### Task 10：完成端到端验证和视觉验收

**文件：**
- 创建：`prototype/tests/capture-flow.spec.ts`
- 创建：`prototype/tests/inbox-flow.spec.ts`
- 创建：`prototype/tests/analysis-flow.spec.ts`
- 修改：`prototype/README.md`
- 修改：`AGENTS.md`

- [ ] **Step 1：编写捕获端到端测试**

`capture-flow.spec.ts`：

```ts
import { expect, test } from "@playwright/test";

test.beforeEach(async ({ page }) => {
  await page.goto("/");
  await page.evaluate(() => localStorage.clear());
  await page.reload();
});

test("quick capture adds a card to inbox", async ({ page }) => {
  await page.getByRole("button", { name: "快速记录" }).click();
  await page.getByLabel("记录内容").fill("验证五秒内完成一次记录");
  await page.getByRole("button", { name: "保存到收集箱" }).click();
  await expect(page.getByText("验证五秒内完成一次记录")).toBeVisible();
});
```

- [ ] **Step 2：编写 Inbox 端到端测试**

覆盖：

- 搜索和状态筛选。
- 打开卡片详情。
- 编辑“我的想法”和标签。
- 标记待分析。
- 刷新后修改仍存在。

- [ ] **Step 3：编写分析闭环端到端测试**

覆盖：

```ts
test("selected cards become a cited saved insight", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("checkbox", { name: /选择卡片/ }).nth(0).check();
  await page.getByRole("checkbox", { name: /选择卡片/ }).nth(1).check();
  await page.getByRole("button", { name: "创建分析集" }).click();
  await page.getByLabel("分析目标").fill("归纳提升思考沉淀的产品机会");
  await page.getByRole("button", { name: "开始分析" }).click();
  await expect(page.getByText("核心结论")).toBeVisible();
  await expect(page.getByRole("button", { name: /来源卡片/ }).first()).toBeVisible();
  await page.getByRole("button", { name: "保存为结论" }).click();
  await expect(page).toHaveURL(/\/insights\//);
  await expect(page.getByText("来源卡片")).toBeVisible();
});
```

- [ ] **Step 4：运行完整自动化验证**

运行：

```bash
npm test
npm run lint
npm run build
npm run test:e2e
```

预期：

- Vitest 无失败测试。
- ESLint 无 error。
- Vite 构建成功。
- Playwright 三组主流程全部通过。

- [ ] **Step 5：进行桌面视觉检查**

启动：

```bash
npm run dev -- --host 127.0.0.1 --port 4173
```

使用 Codex in-app Browser 检查 `http://127.0.0.1:4173`：

- `1280×800`、`1440×900`、`1728×1117` 三种视口。
- Inbox 三栏无横向溢出，详情面板不遮挡列表。
- 批量操作栏不遮挡最后一张卡片。
- 捕获对话框、错误提示和分析 loading 不导致布局跳动。
- 分析引用、追问和结论页可完整操作。
- 页面没有大面积营销式标题、嵌套卡片或无法识别的图标按钮。

若发现视觉问题，修复后重新运行 `npm run build && npm run test:e2e`。

- [ ] **Step 6：补充运行说明和项目验证命令**

`prototype/README.md` 使用中文说明：

```markdown
# IdeaCard 交互原型

## 启动

`npm install`
`npm run dev`

## 验证

`npm test`
`npm run lint`
`npm run build`
`npm run test:e2e`

原型中的剪贴板、截图 OCR 和 AI 分析均为可控模拟能力，不会读取真实系统数据或调用外部模型。
```

在根目录 `AGENTS.md` 增加验证约定：

```markdown
## 原型验证命令
- 在 `prototype/` 下运行 `npm test`。
- 在 `prototype/` 下运行 `npm run lint`。
- 在 `prototype/` 下运行 `npm run build`。
- 核心流程变更还需运行 `npm run test:e2e`。
```

- [ ] **Step 7：提交端到端验收**

```bash
git add AGENTS.md prototype
git commit -m "test: verify IdeaCard prototype workflows"
```

## 最终验收清单

- [ ] 快速记录可在单个对话框中完成，不要求选择分类。
- [ ] 剪贴板和截图 OCR 有正常与失败演示路径。
- [ ] 原始内容和“我的想法”在数据与界面中分离。
- [ ] Inbox 支持搜索、筛选、编辑、状态修改、归档和多选。
- [ ] 单次分析不超过 20 张卡片。
- [ ] 分析目标为空时不能发起分析。
- [ ] AI 结果包含五个固定区块并引用真实输入卡片。
- [ ] 失败、取消和重试可稳定复现。
- [ ] 追问只附着于当前分析集。
- [ ] 保存结论后可查看来源卡片并回到对应详情。
- [ ] 刷新后卡片、分析集和结论仍存在。
- [ ] 单元测试、静态检查、构建和端到端测试全部通过。
- [ ] 1280 像素及以上桌面视口无内容遮挡和不可操作区域。
