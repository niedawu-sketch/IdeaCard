# IdeaCard Mac 桌面基础实施计划

> **供智能执行者使用：** 必须使用 `superpowers:subagent-driven-development`（推荐）或 `superpowers:executing-plans`，逐项执行本计划。步骤使用复选框（`- [ ]`）跟踪。

**目标：** 建立可启动的 Tauri 桌面应用、SQLite 初始迁移、附件仓库、真实卡片读写、三栏收集箱和快速记录闭环。

**架构：** 新建独立 `desktop/` 工程。React 只通过 `DesktopService` 调用 Tauri 命令；Rust 持有数据库、仓库和业务规则，Tauri 命令保持为薄适配层。

**技术栈：** Tauri 2.11、React 19、TypeScript 5.9、Rust stable、rusqlite、sha2、Zustand、Vitest、Testing Library

---

## 文件结构

```text
desktop/
├── package.json                         # 前端、Tauri 和验证命令
├── vite.config.ts                       # Vite 与 Vitest 配置
├── src/
│   ├── app/App.tsx                      # 主窗口布局和路由
│   ├── entities/card/model.ts           # 前端卡片领域类型
│   ├── features/capture/QuickCapture.tsx# 快速记录对话框
│   ├── features/inbox/CardDetail.tsx    # 卡片详情编辑
│   ├── features/inbox/CardList.tsx      # 卡片列表和选择
│   ├── pages/InboxPage.tsx              # 三栏收集箱编排
│   ├── services/desktop/contracts.ts    # 桌面能力接口
│   ├── services/desktop/tauriClient.ts  # invoke 实现
│   ├── services/desktop/context.tsx     # 测试可替换服务
│   ├── store/useDesktopStore.ts         # 异步 UI 状态
│   └── shared/                          # UI、样式、错误
├── src-tauri/
│   ├── migrations/0001_initial.sql      # 初始数据结构
│   ├── src/app_error.rs                 # 稳定错误码
│   ├── src/domain/card.rs               # Rust 卡片领域类型和规则
│   ├── src/database.rs                  # 连接和迁移
│   ├── src/repository/card_repository.rs# 卡片 SQL
│   ├── src/repository/attachment_repository.rs # 附件原子复制
│   ├── src/commands/card_commands.rs     # 卡片 Tauri 命令
│   └── src/lib.rs                        # 应用初始化
└── tests/                               # Playwright 后续阶段使用
```

### 任务 1: 安装工具链并建立 Tauri 工程

**文件：**
- 创建： `desktop/package.json`
- 创建： `desktop/vite.config.ts`
- 创建： `desktop/src/main.tsx`
- 创建： `desktop/src/app/App.tsx`
- 创建： `desktop/src/test/setup.ts`
- 创建： `desktop/src-tauri/`

- [x] **步骤 1：安装已批准的用户级 Rust 工具链**

运行：

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
```

运行：

```bash
source "$HOME/.cargo/env"
rustc --version
cargo --version
```

预期： 两个版本命令退出码为 0；不要修改系统级 Rust 或 Xcode 配置。

- [x] **步骤 2：生成 React TypeScript 工程并固定依赖**

运行：

```bash
npm create vite@8.3.0 desktop -- --template react-ts
```

将 `desktop/package.json` 改为：

```json
{
  "name": "ideacard-desktop",
  "private": true,
  "version": "0.1.0",
  "type": "module",
  "scripts": {
    "dev": "vite --port 1420",
    "build": "tsc -b && vite build",
    "lint": "eslint .",
    "test": "vitest run",
    "test:watch": "vitest",
    "test:e2e": "playwright test",
    "test:rust": "cargo test --manifest-path src-tauri/Cargo.toml",
    "tauri": "tauri"
  },
  "dependencies": {
    "@tauri-apps/api": "2.11.1",
    "lucide-react": "1.17.0",
    "react": "19.2.0",
    "react-dom": "19.2.0",
    "react-router-dom": "7.17.0",
    "zustand": "5.0.14"
  },
  "devDependencies": {
    "@eslint/js": "9.39.1",
    "@playwright/test": "1.61.1",
    "@tauri-apps/cli": "2.11.4",
    "@testing-library/jest-dom": "6.9.1",
    "@testing-library/react": "16.3.2",
    "@testing-library/user-event": "14.6.1",
    "@types/node": "24.10.1",
    "@types/react": "19.2.7",
    "@types/react-dom": "19.2.3",
    "@vitejs/plugin-react": "5.1.1",
    "eslint": "9.39.1",
    "eslint-plugin-react-hooks": "7.0.1",
    "eslint-plugin-react-refresh": "0.4.24",
    "globals": "16.5.0",
    "jsdom": "29.1.1",
    "typescript": "5.9.3",
    "typescript-eslint": "8.48.0",
    "vite": "8.0.16",
    "vitest": "4.1.8"
  }
}
```

运行： `cd desktop && npm install`

预期： 生成 `package-lock.json`，退出码为 0。

- [x] **步骤 3：初始化 Tauri**

在 `desktop/` 目录运行：

```bash
npm run tauri init -- --ci --app-name IdeaCard --window-title IdeaCard --frontend-dist ../dist --dev-url http://localhost:1420 --before-dev-command "npm run dev" --before-build-command "npm run build"
```

将 `desktop/src-tauri/tauri.conf.json` 的产品配置固定为：

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "IdeaCard",
  "version": "0.1.0",
  "identifier": "com.niedawu.ideacard",
  "build": {
    "beforeDevCommand": "npm run dev",
    "devUrl": "http://localhost:1420",
    "beforeBuildCommand": "npm run build",
    "frontendDist": "../dist"
  },
  "app": {
    "windows": [
      {
        "label": "main",
        "title": "IdeaCard",
        "width": 1440,
        "height": 900,
        "minWidth": 1100,
        "minHeight": 700,
        "resizable": true
      }
    ],
    "security": { "csp": "default-src 'self'; img-src 'self' asset: data:; style-src 'self' 'unsafe-inline'" }
  },
  "bundle": {
    "active": true,
    "targets": ["app"],
    "macOS": { "minimumSystemVersion": "14.0" }
  }
}
```

- [x] **步骤 4：建立测试入口并写启动失败测试**

创建 `desktop/src/app/App.test.tsx`：

```tsx
import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { App } from './App'

describe('App', () => {
  it('renders the IdeaCard workspace', () => {
    render(<App />)
    expect(screen.getByRole('heading', { name: '收集箱' })).toBeInTheDocument()
  })
})
```

在 `desktop/vite.config.ts` 中配置 `environment: 'jsdom'` 和 `setupFiles: './src/test/setup.ts'`。测试初始化文件导入 `@testing-library/jest-dom/vitest`，并在每个测试后清理 mock。

- [x] **步骤 5：运行测试并确认失败**

运行： `cd desktop && npm test -- src/app/App.test.tsx`

预期：失败，因为 `App` 尚未导出所需工作区。

- [x] **步骤 6：实现最小启动界面**

创建 `desktop/src/app/App.tsx`：

```tsx
export function App() {
  return (
    <main>
      <h1>收集箱</h1>
    </main>
  )
}
```

更新 `desktop/src/main.tsx`，在 `StrictMode` 内渲染 `<App />`。

- [x] **步骤 7：验证并提交工程**

运行：

```bash
npm test -- src/app/App.test.tsx
npm run build
npm run test:rust
```

预期： 全部退出码为 0。

```bash
git add desktop
git commit -m "chore: scaffold IdeaCard desktop app"
```

### 任务 2: 定义跨前后端领域契约

**文件：**
- 创建： `desktop/src/entities/card/model.ts`
- 创建： `desktop/src/entities/card/model.test.ts`
- 创建： `desktop/src/services/desktop/contracts.ts`
- 创建： `desktop/src-tauri/src/domain/mod.rs`
- 创建： `desktop/src-tauri/src/domain/card.rs`

- [x] **步骤 1：编写标题、来源和字符限制失败测试**

创建 `desktop/src/entities/card/model.test.ts`：

```ts
import { describe, expect, it } from 'vitest'
import { createFallbackTitle, sourceTypeOptions, totalContextCharacters } from './model'

describe('card model', () => {
  it('creates a 30-character title from the first normalized line', () => {
    expect(createFallbackTitle('  第一行   有空格\n第二行  ')).toBe('第一行 有空格')
    expect(createFallbackTitle('一'.repeat(35))).toBe('一'.repeat(30))
  })

  it('defines the five approved source types', () => {
    expect(sourceTypeOptions.map((item) => item.value)).toEqual([
      'personal', 'web', 'document', 'image', 'media',
    ])
  })

  it('counts title, original content, and note for AI context', () => {
    expect(totalContextCharacters([{ title: '题', originalContent: '正文', note: '想法' }])).toBe(5)
  })
})
```

- [x] **步骤 2：运行并确认失败**

运行： `npm test -- src/entities/card/model.test.ts`

预期：失败，因为 `model.ts` 尚不存在。

- [x] **步骤 3：实现前端类型和纯函数**

创建 `desktop/src/entities/card/model.ts`，使用以下固定联合类型：

```ts
export type SourceType = 'personal' | 'web' | 'document' | 'image' | 'media'
export type CaptureMethod = 'quick' | 'clipboard' | 'screenshot' | 'file'
export type CardStatus = 'unprocessed' | 'annotated' | 'pending_analysis' | 'concluded' | 'archived'
export type ExtractionStatus = 'pending' | 'completed' | 'empty' | 'failed' | 'not_applicable'

export interface Attachment {
  id: string
  cardId: string
  kind: 'document' | 'image' | 'audio' | 'video'
  originalName: string
  relativePath: string
  mimeType: string
  byteSize: number
  extractionStatus: ExtractionStatus
  extractionError: string
  extractedText: string
  createdAt: string
}

export interface Card {
  id: string
  title: string
  originalContent: string
  note: string
  sourceType: SourceType
  captureMethod: CaptureMethod
  sourceName: string
  sourceUri: string
  status: CardStatus
  attachments: Attachment[]
  createdAt: string
  updatedAt: string
  archivedAt: string | null
}

export interface NewCardInput {
  title: string
  originalContent: string
  note: string
  sourceType: SourceType
  captureMethod: CaptureMethod
  sourceName: string
  sourceUri: string
}

export const sourceTypeOptions = [
  { value: 'personal', label: '个人记录' },
  { value: 'web', label: '网页文章' },
  { value: 'document', label: '文档资料' },
  { value: 'image', label: '图片材料' },
  { value: 'media', label: '音视频' },
] as const satisfies ReadonlyArray<{ value: SourceType; label: string }>

export function createFallbackTitle(content: string) {
  return content.split(/\r?\n/, 1)[0].trim().replace(/\s+/g, ' ').slice(0, 30)
}

export function totalContextCharacters(cards: Array<Pick<Card, 'title' | 'originalContent' | 'note'>>) {
  return cards.reduce((total, card) => total + card.title.length + card.originalContent.length + card.note.length, 0)
}
```

- [x] **步骤 4：定义桌面服务接口**

创建 `desktop/src/services/desktop/contracts.ts`：

```ts
import type { Card, NewCardInput, SourceType, CardStatus } from '../../entities/card/model'

export interface CardPatch {
  title?: string
  originalContent?: string
  note?: string
  sourceType?: SourceType
  sourceName?: string
  sourceUri?: string
  status?: CardStatus
}

export interface CreateCardResult {
  card: Card
  duplicateCardIds: string[]
}

export interface DesktopService {
  listCards(): Promise<Card[]>
  getCard(id: string): Promise<Card>
  createCard(input: NewCardInput): Promise<CreateCardResult>
  updateCard(id: string, patch: CardPatch): Promise<Card>
}
```

创建结果必须保留重复卡片 ID，供后续界面提示；附件打开能力在阶段 2 写入真实附件记录后再扩展，避免当前阶段暴露无法完成的数据链路。

- [x] **步骤 5：镜像 Rust 类型并测试序列化**

创建 `desktop/src-tauri/src/domain/card.rs`，定义与 TypeScript `snake_case` 值一致的 serde 枚举，以及 `Card`、`Attachment`、`NewCardInput` 和 `CardPatch`。增加以下测试：

```rust
#[test]
fn source_type_serializes_to_stable_values() {
    assert_eq!(serde_json::to_string(&SourceType::Web).unwrap(), "\"web\"");
}

#[test]
fn fallback_title_uses_first_normalized_line() {
    assert_eq!(fallback_title("  第一行   有空格\n第二行  "), "第一行 有空格");
}
```

用 `split_once('\n')`、`split_whitespace`、`join(" ")` 和 `.chars().take(30)` 实现 `fallback_title`，确保中文字符不会按字节截断。

- [x] **步骤 6：验证并提交领域契约**

运行：

```bash
npm test -- src/entities/card/model.test.ts
npm run test:rust
```

预期：通过。

```bash
git add desktop/src/entities desktop/src/services/desktop/contracts.ts desktop/src-tauri/src/domain
git commit -m "feat: define desktop card contracts"
```

### 任务 3: 建立统一错误契约

**文件：**
- 创建： `desktop/src/shared/errors/DesktopError.ts`
- 创建： `desktop/src/shared/errors/DesktopError.test.ts`
- 创建： `desktop/src-tauri/src/app_error.rs`

- [x] **步骤 1：编写错误转换失败测试**

创建 `DesktopError.test.ts`：

```ts
import { describe, expect, it } from 'vitest'
import { DesktopError } from './DesktopError'

it('preserves stable code and recovery detail', () => {
  const error = DesktopError.fromUnknown({ code: 'database_open_failed', message: '无法打开数据库', recovery: '检查数据目录' })
  expect(error.code).toBe('database_open_failed')
  expect(error.recovery).toBe('检查数据目录')
})
```

- [x] **步骤 2：运行并确认失败**

运行： `npm test -- src/shared/errors/DesktopError.test.ts`

预期：失败，因为模块尚不存在。

- [x] **步骤 3：实现前端错误类**

创建 `DesktopError.ts`：

```ts
export class DesktopError extends Error {
  constructor(
    readonly code: string,
    message: string,
    readonly recovery = '',
  ) {
    super(message)
  }

  static fromUnknown(value: unknown) {
    try {
      if (value instanceof DesktopError) return value
      if (value instanceof Error) return new DesktopError('unexpected', '发生未知错误')
      if (value && typeof value === 'object' && 'code' in value && 'message' in value) {
        const item = value as { code: unknown; message: unknown; recovery?: unknown }
        if (
          typeof item.code === 'string' &&
          typeof item.message === 'string' &&
          (item.recovery === undefined || typeof item.recovery === 'string')
        ) {
          return new DesktopError(item.code, item.message, item.recovery ?? '')
        }
      }
    } catch {
      // 任何不可信对象都在下方统一降级。
    }
    return new DesktopError('unexpected', '发生未知错误')
  }
}
```

- [x] **步骤 4：实现 Rust 错误类型**

创建 `app_error.rs`：

```rust
use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub code: &'static str,
    pub message: String,
    pub recovery: String,
}

impl AppError {
    pub fn new(code: &'static str, message: impl Into<String>, recovery: impl Into<String>) -> Self {
        Self { code, message: message.into(), recovery: recovery.into() }
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(_error: rusqlite::Error) -> Self {
        Self::new("database_operation_failed", "数据库操作失败", "重新打开应用；问题持续时检查数据目录")
    }
}
```

增加序列化测试，断言 JSON 中存在 `code`、`message` 和 `recovery` 字段。用包含 SQL 与内部路径的哨兵错误验证 `Display`、`Debug` 和 JSON 均不泄露原始错误文本。

- [x] **步骤 5：验证并提交**

运行： `npm test -- src/shared/errors/DesktopError.test.ts && npm run test:rust`

预期：通过。

```bash
git add desktop/src/shared/errors desktop/src-tauri/src/app_error.rs
git commit -m "feat: add stable desktop errors"
```

### 任务 4: 创建 SQLite 初始迁移与数据库门禁

**文件：**
- 创建： `desktop/src-tauri/migrations/0001_initial.sql`
- 创建： `desktop/src-tauri/src/database.rs`
- 修改： `desktop/src-tauri/Cargo.toml`

- [x] **步骤 1：添加 Rust 数据依赖**

在 `desktop/src-tauri/` 目录运行：

```bash
cargo add rusqlite --features bundled,chrono
cargo add chrono --features serde
cargo add uuid --features v4,serde
cargo add sha2 hex mime_guess tempfile thiserror
```

预期： `Cargo.toml` 与 `Cargo.lock` 更新，退出码为 0。

- [x] **步骤 2：编写空库迁移失败测试**

在 `database.rs` 测试模块中加入：

```rust
#[test]
fn creates_all_initial_tables_and_enables_foreign_keys() {
    let database = Database::open_in_memory().unwrap();
    let tables = database.table_names().unwrap();
    for table in [
        "cards", "attachments", "analysis_sessions", "analysis_session_cards",
        "analysis_messages", "message_card_citations", "message_web_citations",
        "insights", "insight_messages", "app_settings", "schema_migrations",
    ] {
        assert!(tables.contains(&table.to_string()), "missing {table}");
    }
    assert!(database.foreign_keys_enabled().unwrap());
}
```

- [x] **步骤 3：运行并确认失败**

运行： `npm run test:rust -- database::tests::creates_all_initial_tables_and_enables_foreign_keys`

预期：失败，因为 `Database` 和迁移尚不存在。

- [x] **步骤 4：编写完整初始迁移**

创建 `0001_initial.sql`，内容固定如下。迁移由 Rust 外层事务执行，因此 SQL 文件内不写 `BEGIN` 或 `COMMIT`；最后一条语句才登记版本号。

```sql
CREATE TABLE schema_migrations (
  version INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  applied_at TEXT NOT NULL
);

CREATE TABLE cards (
  id TEXT PRIMARY KEY,
  title TEXT NOT NULL,
  original_content TEXT NOT NULL,
  note TEXT NOT NULL DEFAULT '',
  source_type TEXT NOT NULL CHECK (source_type IN ('personal','web','document','image','media')),
  capture_method TEXT NOT NULL CHECK (capture_method IN ('quick','clipboard','screenshot','file')),
  source_name TEXT NOT NULL DEFAULT '',
  source_uri TEXT NOT NULL DEFAULT '',
  status TEXT NOT NULL CHECK (status IN ('unprocessed','annotated','pending_analysis','concluded','archived')),
  content_hash TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE TABLE attachments (
  id TEXT PRIMARY KEY,
  card_id TEXT NOT NULL REFERENCES cards(id) ON DELETE RESTRICT,
  kind TEXT NOT NULL CHECK (kind IN ('document','image','audio','video')),
  original_name TEXT NOT NULL,
  relative_path TEXT NOT NULL UNIQUE,
  mime_type TEXT NOT NULL,
  byte_size INTEGER NOT NULL CHECK (byte_size >= 0),
  checksum TEXT NOT NULL,
  extraction_status TEXT NOT NULL CHECK (extraction_status IN ('pending','completed','empty','failed','not_applicable')),
  extraction_error TEXT NOT NULL DEFAULT '',
  extracted_text TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL
);

CREATE TABLE analysis_sessions (
  id TEXT PRIMARY KEY,
  title TEXT NOT NULL,
  question TEXT NOT NULL,
  web_search_enabled INTEGER NOT NULL CHECK (web_search_enabled IN (0, 1)),
  status TEXT NOT NULL CHECK (status IN ('draft','running','completed','failed','cancelled')),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE analysis_session_cards (
  analysis_session_id TEXT NOT NULL REFERENCES analysis_sessions(id) ON DELETE RESTRICT,
  card_id TEXT NOT NULL REFERENCES cards(id) ON DELETE RESTRICT,
  position INTEGER NOT NULL CHECK (position >= 0),
  PRIMARY KEY (analysis_session_id, card_id),
  UNIQUE (analysis_session_id, position)
);

CREATE TABLE analysis_messages (
  id TEXT PRIMARY KEY,
  analysis_session_id TEXT NOT NULL REFERENCES analysis_sessions(id) ON DELETE RESTRICT,
  role TEXT NOT NULL CHECK (role IN ('user','assistant')),
  content TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status = 'completed'),
  created_at TEXT NOT NULL
);

CREATE TABLE message_card_citations (
  message_id TEXT NOT NULL REFERENCES analysis_messages(id) ON DELETE RESTRICT,
  card_id TEXT NOT NULL REFERENCES cards(id) ON DELETE RESTRICT,
  label TEXT NOT NULL,
  position INTEGER NOT NULL CHECK (position >= 0),
  PRIMARY KEY (message_id, card_id),
  UNIQUE (message_id, position)
);

CREATE TABLE message_web_citations (
  message_id TEXT NOT NULL REFERENCES analysis_messages(id) ON DELETE RESTRICT,
  url TEXT NOT NULL,
  title TEXT NOT NULL,
  start_index INTEGER NOT NULL CHECK (start_index >= 0),
  end_index INTEGER NOT NULL CHECK (end_index >= start_index),
  PRIMARY KEY (message_id, url, start_index)
);

CREATE TABLE insights (
  id TEXT PRIMARY KEY,
  analysis_session_id TEXT NOT NULL REFERENCES analysis_sessions(id) ON DELETE RESTRICT,
  saved_through_message_id TEXT NOT NULL REFERENCES analysis_messages(id) ON DELETE RESTRICT,
  title TEXT NOT NULL,
  question TEXT NOT NULL,
  content TEXT NOT NULL,
  created_at TEXT NOT NULL
);

CREATE TABLE insight_messages (
  insight_id TEXT NOT NULL REFERENCES insights(id) ON DELETE RESTRICT,
  message_id TEXT NOT NULL REFERENCES analysis_messages(id) ON DELETE RESTRICT,
  position INTEGER NOT NULL CHECK (position >= 0),
  PRIMARY KEY (insight_id, message_id),
  UNIQUE (insight_id, position)
);

CREATE TABLE app_settings (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX cards_status_updated_at_idx ON cards(status, updated_at DESC);
CREATE INDEX cards_source_type_updated_at_idx ON cards(source_type, updated_at DESC);
CREATE INDEX cards_content_hash_idx ON cards(content_hash);
CREATE INDEX cards_source_uri_idx ON cards(source_uri) WHERE source_uri <> '';
CREATE INDEX attachments_card_id_idx ON attachments(card_id);
CREATE INDEX attachments_checksum_idx ON attachments(checksum);
CREATE INDEX analysis_session_cards_card_id_idx ON analysis_session_cards(card_id);
CREATE INDEX analysis_messages_session_id_idx ON analysis_messages(analysis_session_id);
CREATE INDEX message_card_citations_card_id_idx ON message_card_citations(card_id);
CREATE INDEX message_web_citations_message_id_idx ON message_web_citations(message_id);
CREATE INDEX insights_session_id_idx ON insights(analysis_session_id);
CREATE INDEX insights_saved_message_id_idx ON insights(saved_through_message_id);
CREATE INDEX insight_messages_message_id_idx ON insight_messages(message_id);

INSERT INTO schema_migrations (version, name, applied_at)
VALUES (1, 'initial', strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
```

所有业务外键均使用 `ON DELETE RESTRICT`，任何表都不使用 `ON DELETE CASCADE`。

- [x] **步骤 5：实现数据库打开与事务迁移**

实现：

```rust
pub struct Database {
    connection: std::sync::Mutex<rusqlite::Connection>,
}

impl Database {
    pub fn open(path: &std::path::Path) -> Result<Self, AppError>;
    pub fn open_in_memory() -> Result<Self, AppError>;
    pub fn with_connection<T>(&self, operation: impl FnOnce(&rusqlite::Connection) -> Result<T, AppError>) -> Result<T, AppError>;
    fn migrate(connection: &mut rusqlite::Connection) -> Result<(), AppError>;
}
```

两个打开方法都执行 `PRAGMA foreign_keys = ON`，在同一事务中运行迁移，并且失败后绝不创建备用数据库。`table_names` 与 `foreign_keys_enabled` 仅供测试使用。

- [x] **步骤 6：验证迁移回滚**

增加迁移回滚测试：在创建哨兵表后执行故意无效的 SQL，断言迁移返回错误，并验证任何部分结构都未保留。

运行： `npm run test:rust -- database::tests`

预期：通过。

- [x] **步骤 7：提交数据库基础**

```bash
git add desktop/src-tauri/Cargo.toml desktop/src-tauri/Cargo.lock desktop/src-tauri/migrations desktop/src-tauri/src/database.rs
git commit -m "feat: create local database schema"
```

### 任务 5: 实现卡片仓库与重复提示

**文件：**
- 创建： `desktop/src-tauri/src/repository/mod.rs`
- 创建： `desktop/src-tauri/src/repository/card_repository.rs`
- 修改： `desktop/src-tauri/src/domain/card.rs`

- [x] **步骤 1：编写仓库失败测试**

在 `card_repository.rs` 的测试模块中创建内存数据库，并分别实现以下测试：

- `creates_and_lists_a_quick_card`：以空标题、内容 ` 第一行\n第二行 `、个人来源和快速记录方式创建卡片；断言标题为 `第一行`、内容保留输入正文、来源为 `personal`、状态为 `unprocessed`、创建与更新时间非空，并且 `list(false)` 只返回该卡片。
- `note_changes_unprocessed_card_to_annotated`：创建无笔记卡片后提交 `CardPatch { note: Some("新的想法") }`；断言笔记已更新、状态变为 `annotated`、`updated_at` 不早于原值。
- `duplicate_content_is_reported_but_second_card_can_be_saved`：依次保存 `重复正文` 和 `  重复正文  `；断言第二次创建仍成功、两个 ID 不同，且第二次结果的 `duplicate_card_ids` 只包含第一张卡片 ID。
- `invalid_source_value_is_rejected_by_the_database`：绕过仓库直接执行来源为 `forum` 的完整 `INSERT`；断言 SQLite 返回 `ConstraintViolation`，并且卡片计数仍为 0。

- [x] **步骤 2：运行并确认失败**

运行： `npm run test:rust -- repository::card_repository::tests`

预期：失败，因为 `CardRepository` 尚不存在。

- [x] **步骤 3：实现仓库接口**

实现以下固定方法：

```rust
pub struct CreateCardResult {
    pub card: Card,
    pub duplicate_card_ids: Vec<String>,
}

pub struct CardRepository<'a> { database: &'a Database }

impl<'a> CardRepository<'a> {
    pub fn new(database: &'a Database) -> Self;
    pub fn list(&self, include_archived: bool) -> Result<Vec<Card>, AppError>;
    pub fn get(&self, id: &str) -> Result<Card, AppError>;
    pub fn create(&self, input: NewCardInput) -> Result<CreateCardResult, AppError>;
    pub fn update(&self, id: &str, patch: CardPatch) -> Result<Card, AppError>;
}
```

用 `trim` 规范化内容，对规范化 UTF-8 内容计算 SHA-256；插入前查询重复项，但仍保存新卡片。卡片及附件写入共用一个事务；找不到卡片时返回 `card_not_found`，不得返回空结果。

- [x] **步骤 4：验证全部仓库行为**

运行： `npm run test:rust -- repository::card_repository::tests`

预期：全部仓库测试通过。

- [x] **步骤 5：提交卡片仓库**

```bash
git add desktop/src-tauri/src/domain/card.rs desktop/src-tauri/src/repository
git commit -m "feat: persist desktop cards"
```

### 任务 6: 实现附件原子仓库

**文件：**
- 创建： `desktop/src-tauri/src/repository/attachment_repository.rs`

- [x] **步骤 1：编写附件失败测试**

使用 `tempfile::TempDir` 建立独立附件目录和缓存目录，并分别实现以下测试：

- `copies_to_temp_then_atomically_moves_and_hashes_file`：保存内容为 `hello` 的 `.txt`；断言最终文件存在且内容一致，SHA-256 为 `2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824`，相对路径不含原始基名，缓存目录为空。
- `rejects_document_over_50_megabytes_before_copying`：用 `set_len(50 * 1024 * 1024 + 1)` 创建稀疏 `.pdf`；断言错误码为 `attachment_too_large`，附件目录和缓存目录均为空。
- `rejects_media_over_500_megabytes_before_copying`：用 `set_len(500 * 1024 * 1024 + 1)` 创建稀疏 `.mp4`；断言错误码为 `attachment_too_large`，且没有目标文件。
- `removes_temp_file_when_copy_fails`：通过测试专用 `store_from_reader` 注入读取到第二块时失败的 Reader；断言错误码为 `attachment_copy_failed`，缓存和附件目录均为空。

- [x] **步骤 2：运行并确认失败**

运行： `npm run test:rust -- repository::attachment_repository::tests`

预期：失败，因为附件仓库尚不存在。

- [x] **步骤 3：实现附件仓库**

实现：

```rust
pub enum AttachmentClass { DocumentOrImage, Media }

pub struct StoredAttachment {
    pub relative_path: String,
    pub byte_size: u64,
    pub checksum: String,
    pub mime_type: String,
}

pub struct AttachmentRepository { root: PathBuf, cache: PathBuf }

impl AttachmentRepository {
    pub fn store(&self, source: &Path, class: AttachmentClass) -> Result<StoredAttachment, AppError>;
    pub fn resolve(&self, relative_path: &str) -> Result<PathBuf, AppError>;
}
```

使用生成的 UUID 文件名，只保留校验通过的扩展名。字节流经 `Sha256` 后调用 `sync_all`，再从缓存目录 `rename` 到附件目录。`resolve` 拒绝绝对路径以及任何不是 `Component::Normal` 的路径组成部分。

- [x] **步骤 4：运行测试并提交**

运行： `npm run test:rust -- repository::attachment_repository::tests`

预期：通过。

```bash
git add desktop/src-tauri/src/repository/attachment_repository.rs
git commit -m "feat: store imported attachments safely"
```

### 任务 7: 暴露卡片命令与类型化 Tauri 客户端

**文件：**
- 创建： `desktop/src-tauri/src/commands/mod.rs`
- 创建： `desktop/src-tauri/src/commands/card_commands.rs`
- 修改： `desktop/src-tauri/src/lib.rs`
- 创建： `desktop/src/services/desktop/tauriClient.ts`
- 创建： `desktop/src/services/desktop/tauriClient.test.ts`

- [x] **步骤 1：编写 invoke 映射失败测试**

mock `@tauri-apps/api/core` 并断言：

```ts
expect(invoke).toHaveBeenCalledWith('create_card', { input })
expect(invoke).toHaveBeenCalledWith('update_card', { id: 'card-1', patch })
```

- [x] **步骤 2：运行并确认失败**

运行： `npm test -- src/services/desktop/tauriClient.test.ts`

预期：失败，因为客户端尚不存在。

- [x] **步骤 3：实现前端客户端**

创建实现 `DesktopService` 全部方法的 `tauriDesktopService`，内部调用 `invoke<T>`，并用 `DesktopError.fromUnknown` 转换拒绝结果。

- [x] **步骤 4：实现薄命令**

只暴露以下命令：

```rust
#[tauri::command] pub fn list_cards(state: State<'_, AppState>) -> Result<Vec<Card>, AppError>;
#[tauri::command] pub fn get_card(id: String, state: State<'_, AppState>) -> Result<Card, AppError>;
#[tauri::command] pub fn create_card(input: NewCardInput, state: State<'_, AppState>) -> Result<CreateCardResult, AppError>;
#[tauri::command] pub fn update_card(id: String, patch: CardPatch, state: State<'_, AppState>) -> Result<Card, AppError>;
```

`AppState` 持有 `Database` 和 `AttachmentRepository`。通过 `app.path().app_data_dir()` 与 `app.path().app_cache_dir()` 初始化应用目录；本阶段在 `generate_handler!` 中只注册上述命令。

- [x] **步骤 5：验证命令与客户端**

运行：

```bash
npm test -- src/services/desktop/tauriClient.test.ts
npm run test:rust
```

预期：通过。

- [x] **步骤 6：提交桌面桥接**

```bash
git add desktop/src/services/desktop desktop/src-tauri/src/commands desktop/src-tauri/src/lib.rs
git commit -m "feat: expose card desktop commands"
```

### 任务 8: 建立可替换服务与异步 Store

**文件：**
- 创建： `desktop/src/services/desktop/context.tsx`
- 创建： `desktop/src/services/desktop/useDesktopService.ts`
- 创建： `desktop/src/store/useDesktopStore.ts`
- 创建： `desktop/src/store/useDesktopStore.test.ts`

- [x] **步骤 1：编写异步状态失败测试**

使用假 `DesktopService` 并断言：

```ts
await store.getState().loadCards(service)
expect(store.getState().cards).toEqual(fakeCards)

await store.getState().createQuickCard(service, { title: '', originalContent: '临时问题', note: '' })
expect(service.createCard).toHaveBeenCalledWith(expect.objectContaining({
  title: '临时问题', sourceType: 'personal', captureMethod: 'quick',
}))
```

另测服务调用失败时仍保留当前卡片，并保存对应 `DesktopError`；创建成功时选中新卡片，并把 `duplicateCardIds` 保存在 Store 中供界面展示重复提示。

- [x] **步骤 2：运行并确认失败**

运行： `npm test -- src/store/useDesktopStore.test.ts`

预期：失败，因为 Store 尚不存在。

- [x] **步骤 3：实现 Store**

暴露状态 `cards`、`selectedCardId`、`duplicateCardIds`、`isLoading`、`error`、`errorOperation`，以及动作 `loadCards`、`selectCard`、`createQuickCard`、`updateCard`、`clearDuplicateCardIds`、`clearError`。`errorOperation` 只允许 `load`、`create`、`update` 或 `null`，界面据此把恢复动作放回发生错误的工作区。不得持久化 Zustand 状态；所有持久化变更都经过 `DesktopService`，并用返回实体替换内存对象。

- [x] **步骤 4：实现 React 服务上下文**

创建 `DesktopServiceProvider` 和 `useDesktopService`；默认 Provider 使用 `tauriDesktopService`，测试可注入假实现。在 Provider 外调用 Hook 时抛出稳定的开发错误。

- [x] **步骤 5：验证并提交**

运行： `npm test -- src/store/useDesktopStore.test.ts`

预期：通过。

```bash
git add desktop/src/services/desktop/context.tsx desktop/src/store
git commit -m "feat: coordinate persisted desktop state"
```

### 任务 9: 迁移三栏收集箱和快速记录

**文件：**
- 创建： `desktop/src/features/capture/QuickCapture.tsx`
- 创建： `desktop/src/features/capture/QuickCapture.test.tsx`
- 创建： `desktop/src/features/inbox/CardList.tsx`
- 创建： `desktop/src/features/inbox/CardDetail.tsx`
- 创建： `desktop/src/pages/InboxPage.tsx`
- 创建： `desktop/src/pages/InboxPage.test.tsx`
- 修改： `desktop/src/app/App.tsx`
- 修改： `desktop/src/main.tsx`
- 修改： `desktop/src/store/useDesktopStore.ts`
- 创建： `desktop/src/shared/styles/global.css`
- 创建： `desktop/playwright.config.ts`
- 创建： `desktop/e2e/inbox-layout.spec.ts`
- 修改： `desktop/vite.config.ts`
- 修改： `desktop/.gitignore`

- [x] **步骤 1：编写主流程失败测试**

使用注入的 `DesktopService` 假实现编写四个测试，断言固定如下：

- `loads cards, filters five source types, and hides tags`：等待两张卡片出现；断言五个来源按钮都存在，点击 `文档资料` 后只显示文档卡片，并且页面中不存在文本 `标签`。
- `creates a quick card with an optional note`：打开快速记录，输入正文 `先验证收集流程` 和笔记 `观察一周` 后保存；断言 `createCard` 只调用一次，参数的来源为 `personal`、方式为 `quick`、正文和笔记与输入一致，并选中新返回的卡片。
- `edits note and source information from detail`：选择一张卡片，把笔记改为 `补充判断`、来源改为 `document`、来源名称改为 `方案.pdf`；断言 `updateCard` 收到对应卡片 ID 和这三个字段的补丁。
- `keeps existing cards visible when loading_fails`：先成功加载卡片，再让刷新调用拒绝并点击刷新；断言原卡片仍可见、恢复提示出现，且提示包含重试按钮。

- [x] **步骤 2：运行并确认失败**

运行： `npm test -- src/features/capture/QuickCapture.test.tsx src/pages/InboxPage.test.tsx`

预期：失败，因为界面模块尚不存在。

- [x] **步骤 3：迁移视觉组件**

从 `prototype/src/shared/styles/global.css` 迁移已验证的三栏布局和中性绿色视觉变量。卡片圆角不超过 8px，使用 Lucide 图标，保持桌面布局最小宽度 1100px，并移除全部模拟捕获和分析适配器。

- [x] **步骤 4：实现快速记录**

`QuickCapture` has title, required content, optional note, Cancel and Save. Disable Save for blank trimmed content or while submitting. On success close and select returned card. On failure keep all fields and display the mapped recovery message.

- [x] **步骤 5：实现 Inbox**

挂载时加载卡片；搜索与来源筛选在本地派生；即使没有数据也显示五种来源。`CardDetail` 通过 `updateCard` 编辑标题、原始内容、笔记、来源类型、来源名称、来源 URI 和状态。不得渲染标签或破坏性操作。

- [x] **步骤 6：验证前端和 1280px 布局**

运行：

```bash
npm test
npm run lint
npm run build
```

预期：通过。再以 1280x800 运行 Playwright 组件冒烟测试，断言 `scrollWidth === clientWidth`。

- [x] **步骤 7：提交主窗口闭环**

```bash
git add desktop/src
git commit -m "feat: add persisted desktop inbox"
```

### 任务 10: 验证阶段 1 可运行产物

**文件：**
- 修改： `desktop/README.md`

- [x] **步骤 1：启动 Tauri 开发应用**

运行： `cd desktop && npm run tauri dev`

预期： IdeaCard 主窗口打开；创建快速记录后退出并重新启动，卡片仍存在。

- [x] **步骤 2：运行阶段全量验证**

运行：

```bash
npm test
npm run lint
npm run build
npm run test:rust
npm run tauri build -- --bundles app
```

预期： 全部退出码为 0，并在 `desktop/src-tauri/target/release/bundle/macos/IdeaCard.app` 生成应用。

- [x] **步骤 3：更新中文运行说明**

`desktop/README.md` 必须记录开发启动、自动测试、`.app` 构建命令、数据目录和阶段 1 已实现/未实现能力，不包含 API Key 示例值。

- [x] **步骤 4：最终检查并提交**

运行： `git diff --check && git status --short`

预期： 只有 README 修改。

```bash
git add desktop/README.md
git commit -m "docs: document desktop foundation"
```
