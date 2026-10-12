# 结构化卡片、概念注释与 AI 延伸 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** 将 IdeaCard 从纯文本卡片升级为支持正文贴图、概念注释、概念集合、卡片内 AI 对话和三类模型接入的可运行 Mac 应用。

**Architecture:** React 使用 TipTap/ProseMirror JSON 作为正文编辑事实来源，Rust 负责严格校验 JSON、将临时 Data URL 转换为本地附件并在 SQLite 事务中保存。概念、引用和卡片 AI 对话使用数据库版本 2 的新表；模型请求统一经过 Rust 适配器，非密钥设置进入 `app_settings`，API Key 只进入 macOS Keychain。

**Tech Stack:** React 19、TypeScript、TipTap 3、Zustand、Tauri 2、Rust、rusqlite、reqwest、macOS Security Framework、Vitest、Playwright、Cargo tests、Swift tests。

---

## 文件结构

### Rust

- `desktop/src-tauri/migrations/0002_structured_card_ai.sql`：版本 2 表、索引和迁移记录。
- `desktop/src-tauri/src/database.rs`：顺序迁移和最终 schema 校验。
- `desktop/src-tauri/src/domain/document.rs`：结构化正文类型、校验、纯文本投影和引用收集。
- `desktop/src-tauri/src/domain/concept.rs`：概念、引用和详情 DTO。
- `desktop/src-tauri/src/domain/ai.rs`：模型配置、消息、引用、请求和响应 DTO。
- `desktop/src-tauri/src/repository/document_repository.rs`：正文、正文图片关系和图片事务。
- `desktop/src-tauri/src/repository/concept_repository.rs`：概念及引用事务。
- `desktop/src-tauri/src/repository/ai_repository.rs`：设置、线程、消息和引用持久化。
- `desktop/src-tauri/src/ai/keychain.rs`：Keychain trait 与 macOS 实现。
- `desktop/src-tauri/src/ai/provider.rs`：供应商无关请求、响应和错误。
- `desktop/src-tauri/src/ai/openai.rs`：Responses API 和引用解析。
- `desktop/src-tauri/src/ai/openai_compatible.rs`：Chat Completions 兼容格式。
- `desktop/src-tauri/src/ai/anthropic.rs`：Claude Messages API。
- `desktop/src-tauri/src/ai/service.rs`：提示词、上下文、取消和保存编排。
- `desktop/src-tauri/src/commands/document_commands.rs`：正文与图片命令。
- `desktop/src-tauri/src/commands/concept_commands.rs`：概念命令。
- `desktop/src-tauri/src/commands/ai_commands.rs`：设置、解释和对话命令。

### React

- `desktop/src/entities/document/model.ts`：正文 JSON 类型、纯文本转文档和图片 Data URL 工具。
- `desktop/src/entities/concept/model.ts`：概念前端类型。
- `desktop/src/entities/ai/model.ts`：配置、消息和供应商能力。
- `desktop/src/features/editor/RichTextEditor.tsx`：受控结构化编辑器与粘贴入口。
- `desktop/src/features/editor/extensions/InlineImage.tsx`：本地图片 NodeView。
- `desktop/src/features/editor/extensions/ConceptMark.ts`：概念 mark。
- `desktop/src/features/concepts/ConceptExplainDialog.tsx`：解释预览和保存。
- `desktop/src/features/concepts/ConceptPage.tsx`：概念列表、详情和来源跳转。
- `desktop/src/features/ai/AiConversation.tsx`：卡片内对话。
- `desktop/src/features/settings/AiSettingsDialog.tsx`：供应商、模型、Keychain 状态和连接测试。
- `desktop/src/features/inbox/CardDetail.tsx`：主要内容层级和折叠元信息。
- `desktop/src/features/capture/QuickCapture.tsx`：结构化正文和图片创建。
- `desktop/src/features/context-menu/TextContextMenu.tsx`：正文选区增加 AI 解释动作。
- `desktop/src/services/desktop/contracts.ts`：新增桌面服务契约。
- `desktop/src/services/desktop/tauriClient.ts`：新增 Tauri 命令适配。

## 任务 1：建立版本 2 增量迁移

**Files:**
- Create: `desktop/src-tauri/migrations/0002_structured_card_ai.sql`
- Modify: `desktop/src-tauri/src/database.rs`
- Test: `desktop/src-tauri/src/database.rs`

- [x] **Step 1: 写升级测试**

测试先创建真实版本 1 数据库，插入卡片、附件和旧分析记录，然后重新打开数据库并断言：

```rust
assert_eq!(applied_versions(&connection), vec![1, 2]);
assert_eq!(card_title(&connection, "card-v1"), "旧卡片");
assert_eq!(attachment_count(&connection, "card-v1"), 1);
assert_eq!(analysis_count(&connection), 1);
assert!(table_exists(&connection, "card_documents"));
assert!(table_exists(&connection, "concepts"));
assert!(table_exists(&connection, "card_ai_threads"));
```

另写失败迁移测试，证明版本 2 在事务中回滚且版本仍为 1。

- [x] **Step 2: 运行测试确认 RED**

Run:

```bash
npm --prefix desktop run test:rust -- database
```

Expected: FAIL，原因是只支持版本 1 或缺少版本 2 表。

- [x] **Step 3: 新增迁移 SQL**

建立：

```sql
CREATE TABLE card_documents (...);
CREATE TABLE card_document_images (...);
CREATE TABLE concepts (...);
CREATE TABLE concept_mentions (...);
CREATE TABLE card_ai_threads (...);
CREATE TABLE card_ai_messages (...);
CREATE TABLE card_ai_citations (...);
```

所有业务外键使用 `ON DELETE RESTRICT`，由生命周期服务显式删除。最后插入：

```sql
INSERT INTO schema_migrations (version, name, applied_at)
VALUES (2, 'structured_card_ai', strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
```

- [x] **Step 4: 实现顺序迁移**

`Database::migrate` 按当前版本执行缺失迁移；`expected_schema` 在内存库顺序执行 0001 和 0002；`validate_current_schema` 要求最终版本为 2。禁止删除或重建已有数据库。

- [x] **Step 5: 运行迁移测试确认 GREEN**

Run:

```bash
npm --prefix desktop run test:rust -- database
```

Expected: PASS。

## 任务 2：实现结构化正文校验与附件事务

**Files:**
- Create: `desktop/src-tauri/src/domain/document.rs`
- Create: `desktop/src-tauri/src/repository/document_repository.rs`
- Create: `desktop/src-tauri/src/commands/document_commands.rs`
- Modify: `desktop/src-tauri/src/domain/mod.rs`
- Modify: `desktop/src-tauri/src/repository/mod.rs`
- Modify: `desktop/src-tauri/src/commands/mod.rs`
- Modify: `desktop/src-tauri/src/repository/attachment_repository.rs`
- Modify: `desktop/src-tauri/src/lib.rs`
- Modify: `desktop/src-tauri/src/card_lifecycle.rs`
- Test: matching Rust modules

- [x] **Step 1: 写文档校验 RED 测试**

覆盖：

```rust
assert_eq!(validate_document(valid_text_doc)?.plain_text, "第一段\n第二段");
assert_eq!(validate_document(image_only_doc)?.plain_text, "");
assert_error_code(unexpected_node_doc, "document_invalid");
assert_error_code(image_over_limit_doc, "inline_image_too_large");
assert_error_code(data_url_left_after_save, "document_invalid");
```

- [x] **Step 2: 实现允许列表校验**

递归允许 `doc / paragraph / text / hardBreak / image` 和 `concept` mark；限制 JSON 大小、深度、节点数、单图大小和单次新增图片数。输出：

```rust
pub struct ValidatedDocument {
    pub json: serde_json::Value,
    pub plain_text: String,
    pub persisted_image_ids: Vec<String>,
    pub pending_images: Vec<PendingImage>,
    pub concept_mentions: Vec<DocumentMention>,
}
```

- [x] **Step 3: 写附件保存和回滚 RED 测试**

覆盖新图片写入、节点替换、图片删除、数据库失败时文件回滚、非法附件归属、读取图片 Data URL。

- [x] **Step 4: 扩展附件仓库**

增加受限字节写入：

```rust
pub fn store_bytes(
    &self,
    bytes: &[u8],
    extension: &str,
    class: AttachmentClass,
) -> Result<StoredAttachment, AppError>
```

复用现有临时文件、哈希、大小限制和原子重命名逻辑。

- [x] **Step 5: 实现正文仓库和命令**

提供：

```rust
get_card_document(card_id) -> CardDocument
save_card_document(card_id, document) -> SavedCardDocument
load_inline_image(attachment_id) -> InlineImageData
create_rich_card(input) -> CreateCardResult
```

保存必须同步 `cards.original_content`、`card_documents`、`card_document_images` 和 `attachments`。新图片文件失败或 SQL 失败时回滚；删除图片使用现有 staged removal。

- [x] **Step 6: 扩展永久删除**

按引用顺序删除 AI 引用、AI 消息、AI 线程、概念引用、正文图片关系、正文，再删除附件和卡片。概念实体不删除。

- [x] **Step 7: 运行 Rust 测试**

Run:

```bash
npm --prefix desktop run test:rust
```

Expected: PASS。

## 任务 3：接入 TipTap 并支持快速记录贴图

**Files:**
- Modify: `desktop/package.json`
- Modify: `desktop/package-lock.json`
- Create: `desktop/src/entities/document/model.ts`
- Create: `desktop/src/entities/document/model.test.ts`
- Create: `desktop/src/features/editor/RichTextEditor.tsx`
- Create: `desktop/src/features/editor/RichTextEditor.test.tsx`
- Create: `desktop/src/features/editor/extensions/InlineImage.tsx`
- Modify: `desktop/src/features/capture/QuickCapture.tsx`
- Modify: `desktop/src/features/capture/QuickCapture.test.tsx`
- Modify: `desktop/src/services/desktop/contracts.ts`
- Modify: `desktop/src/services/desktop/tauriClient.ts`
- Modify: `desktop/src/services/desktop/tauriClient.test.ts`

- [x] **Step 1: 安装固定版本编辑器依赖**

Run:

```bash
npm --prefix desktop install @tiptap/core@3.28.0 @tiptap/react@3.28.0 @tiptap/starter-kit@3.28.0 @tiptap/extension-bubble-menu@3.28.0 @tiptap/extension-floating-menu@3.28.0
```

Expected: `package.json` 和 lockfile 只新增项目依赖。

- [x] **Step 2: 写文档工具 RED 测试**

覆盖纯文本转段落、空文档、纯文本投影、Data URL 大小和图片 MIME。

- [x] **Step 3: 实现文档前端类型**

```ts
export interface RichDocument {
  type: 'doc'
  content: RichNode[]
}

export function documentFromPlainText(value: string): RichDocument
export function documentPlainText(document: RichDocument): string
export async function clipboardImageNode(file: File): Promise<RichNode>
```

- [x] **Step 4: 写编辑器 RED 测试**

使用真实 `ClipboardEvent` 数据证明：

- 图片插入当前选区。
- 不支持文件显示错误。
- 只读时不处理粘贴。
- `onChange` 返回 JSON 和纯文本。

- [x] **Step 5: 实现最小结构化编辑器**

TipTap 只启用段落、文字、换行、撤销历史、自定义图片和后续概念 mark。图片 NodeView：

- 临时节点读取 `data`。
- 已保存节点调用 `loadInlineImage(attachmentId)`。
- 显示加载、成功和失败状态。

- [x] **Step 6: 改造快速记录**

标题和“我的想法”保持普通字段；“记录内容”使用 `RichTextEditor`。保存调用 `createRichCard`，只有图片时标题为“图片记录”。

- [x] **Step 7: 运行前端测试**

Run:

```bash
npm --prefix desktop test -- RichTextEditor QuickCapture document tauriClient
```

Expected: PASS。

## 任务 4：实现概念事务与查询

**Files:**
- Create: `desktop/src-tauri/src/domain/concept.rs`
- Create: `desktop/src-tauri/src/repository/concept_repository.rs`
- Create: `desktop/src-tauri/src/commands/concept_commands.rs`
- Modify: Rust module indexes and `lib.rs`
- Test: matching Rust modules

- [x] **Step 1: 写概念 RED 测试**

覆盖：

- 新概念和引用原子保存。
- 同名概念复用且不覆盖解释。
- 文档中缺少对应 mark 时返回 `concept_selection_stale`。
- mark 文字与 `selected_text` 不一致时拒绝。
- 列表引用计数。
- 概念详情包含收集箱和归档来源。

- [x] **Step 2: 实现规范化和 DTO**

```rust
pub fn normalize_concept_name(value: &str) -> Result<String, AppError>;
pub struct ConceptSummary { ... }
pub struct ConceptDetail { pub mentions: Vec<ConceptSource>, ... }
```

使用 Unicode 小写和空白归一化，不进行拼音或同义词合并。

- [x] **Step 3: 实现原子保存**

`save_concept_annotation` 接收已经带临时 UUID mark 的文档，校验后在同一事务内创建/复用概念、插入引用和保存正文。

- [x] **Step 4: 实现查询命令**

```rust
list_concepts() -> Vec<ConceptSummary>
get_concept(id) -> ConceptDetail
list_card_concepts(card_id) -> Vec<CardConcept>
save_concept_annotation(input) -> SavedConceptAnnotation
```

- [x] **Step 5: 运行 Rust 测试**

Run:

```bash
npm --prefix desktop run test:rust -- concept
```

Expected: PASS。

## 任务 5：实现概念界面、右键解释入口和跳转

**Files:**
- Create: `desktop/src/entities/concept/model.ts`
- Create: `desktop/src/features/concepts/ConceptPage.tsx`
- Create: `desktop/src/features/concepts/ConceptPage.test.tsx`
- Create: `desktop/src/features/concepts/ConceptPopover.tsx`
- Create: `desktop/src/features/editor/extensions/ConceptMark.ts`
- Modify: `desktop/src/features/editor/RichTextEditor.tsx`
- Modify: `desktop/src/features/context-menu/TextContextMenu.tsx`
- Modify: `desktop/src/features/context-menu/TextContextMenu.test.tsx`
- Modify: `desktop/src/app/App.tsx`
- Modify: desktop service contract/client tests

- [x] **Step 1: 写右键入口 RED 测试**

正文非空选区显示“AI 解释概念”；普通字段仍只有四个基本动作；已归档正文不显示 AI 动作。

- [x] **Step 2: 扩展选区事件**

`RichTextEditor` 在 `contextmenu` 时保存 `{ from, to, text, revision }`；全局菜单点击后向对应编辑器分发 `ideacard:explain-concept`，不丢失选区。

- [x] **Step 3: 写概念 mark RED 测试**

覆盖悬浮解释、点击打开概念、目标 mention 滚动和只读展示。

- [x] **Step 4: 实现 mark 与悬浮弹层**

`ConceptMark` 输出 `data-concept-id` 和 `data-mention-id`。编辑器通过事件委托读取当前卡片概念映射，悬浮显示摘要，点击调用 `onOpenConcept`。

- [x] **Step 5: 写概念页面 RED 测试**

列表显示引用数；详情列出来源；点击来源回到卡片并传递 mentionId。

- [x] **Step 6: 实现概念工作区**

在侧栏加入“概念”，保持内缩圆角选中态；概念页面使用列表 + 详情两栏，不新增营销式说明。

- [x] **Step 7: 运行前端测试**

Run:

```bash
npm --prefix desktop test -- TextContextMenu RichTextEditor ConceptPage
```

Expected: PASS。

## 任务 6：实现模型设置、Keychain 和三类协议

**Files:**
- Modify: `desktop/src-tauri/Cargo.toml`
- Modify: `desktop/src-tauri/Cargo.lock`
- Create: `desktop/src-tauri/src/domain/ai.rs`
- Create: `desktop/src-tauri/src/repository/ai_repository.rs`
- Create: `desktop/src-tauri/src/ai/mod.rs`
- Create: `desktop/src-tauri/src/ai/keychain.rs`
- Create: `desktop/src-tauri/src/ai/provider.rs`
- Create: `desktop/src-tauri/src/ai/openai.rs`
- Create: `desktop/src-tauri/src/ai/openai_compatible.rs`
- Create: `desktop/src-tauri/src/ai/anthropic.rs`
- Create: `desktop/src-tauri/src/commands/ai_commands.rs`
- Modify: Rust module indexes and `lib.rs`
- Test: matching Rust modules

- [x] **Step 1: 添加本地依赖**

在 `Cargo.toml` 增加 `reqwest`（rustls/json）、`base64`、`tokio-util` 和 macOS `security-framework`；测试依赖使用本地回环模拟服务或 transport fake，不调用真实模型。

- [x] **Step 2: 写设置和 Keychain RED 测试**

覆盖：

- SQLite 只保存 provider/base URL/model/web search。
- API Key 保存、存在检查、更新、移除通过 `SecretStore` fake。
- 任何 DTO、Debug、错误和日志不含 key。
- Base URL 拒绝非 HTTPS，测试环境允许回环 HTTP。

- [x] **Step 3: 实现配置仓库和 Keychain**

```rust
pub trait SecretStore: Send + Sync {
    fn set(&self, account: &str, value: &str) -> Result<(), AppError>;
    fn exists(&self, account: &str) -> Result<bool, AppError>;
    fn get(&self, account: &str) -> Result<String, AppError>;
    fn remove(&self, account: &str) -> Result<(), AppError>;
}
```

系统实现使用 `security_framework::passwords`，service 固定为 `com.niedawu.ideacard.ai`。

- [x] **Step 4: 写三类协议 RED 测试**

对 transport 捕获的 URL、header 和 JSON 做精确断言：

```text
OpenAI: POST {base}/responses, Authorization: Bearer, tools:[{type:web_search}]
Compatible: POST {base}/chat/completions, Authorization: Bearer
Claude: POST {base}/messages, x-api-key, anthropic-version
```

用官方结构 fixture 验证文本和 OpenAI `url_citation` 解析。

- [x] **Step 5: 实现供应商适配器**

统一输出：

```rust
pub struct AiResult {
    pub text: String,
    pub citations: Vec<AiCitation>,
}
```

官方 OpenAI 默认模型 `gpt-5.6`；兼容和 Claude 模型由用户填写。响应正文不进入错误消息。

- [x] **Step 6: 实现设置和连接测试命令**

```rust
get_ai_settings()
save_ai_settings(input)
remove_ai_key()
test_ai_connection(request_id)
cancel_ai_request(request_id)
```

连接测试使用真实适配器请求并要求回复 `OK`。

- [x] **Step 7: 运行 Rust 测试**

Run:

```bash
npm --prefix desktop run test:rust -- ai
```

Expected: PASS。

## 任务 7：实现概念解释和卡片 AI 对话

**Files:**
- Create: `desktop/src-tauri/src/ai/service.rs`
- Modify: `desktop/src-tauri/src/repository/ai_repository.rs`
- Modify: `desktop/src-tauri/src/commands/ai_commands.rs`
- Modify: `desktop/src-tauri/src/lib.rs`
- Test: matching Rust modules

- [x] **Step 1: 写提示上下文 RED 测试**

证明概念解释只包含选区、当前段落和必要上下文；卡片对话包含标题、正文、想法、已引用概念和最近消息，不包含其他卡片。

- [x] **Step 2: 写消息事务 RED 测试**

成功时保存 user + assistant + citations；失败或取消时不保存不完整消息；同一卡片复用一个线程。

- [ ] **Step 3: 实现请求取消**

`AppState` 保存 `request_id -> CancellationToken`。命令开始注册、结束移除；取消命令触发 token，网络 future 使用 `tokio::select!`。

当前实现已让界面立即停止等待，并确保取消后的响应不会写入数据库；底层阻塞网络连接尚未改为 `CancellationToken + tokio::select!` 主动中止。

- [x] **Step 4: 实现解释与对话命令**

```rust
explain_concept(request) -> AiDraft
list_card_ai_messages(card_id) -> Vec<AiMessage>
send_card_ai_message(request) -> AiExchange
```

所有提示词用中文，模型输出按纯文本保存。

- [x] **Step 5: 运行 Rust 测试**

Run:

```bash
npm --prefix desktop run test:rust -- ai
```

Expected: PASS。

## 任务 8：实现模型设置、解释预览和 AI 对话界面

**Files:**
- Create: `desktop/src/entities/ai/model.ts`
- Create: `desktop/src/features/settings/AiSettingsDialog.tsx`
- Create: `desktop/src/features/settings/AiSettingsDialog.test.tsx`
- Create: `desktop/src/features/concepts/ConceptExplainDialog.tsx`
- Create: `desktop/src/features/concepts/ConceptExplainDialog.test.tsx`
- Create: `desktop/src/features/ai/AiConversation.tsx`
- Create: `desktop/src/features/ai/AiConversation.test.tsx`
- Modify: `desktop/src/app/App.tsx`
- Modify: `desktop/src/pages/InboxPage.tsx`
- Modify: `desktop/src/features/inbox/CardDetail.tsx`
- Modify: desktop service contract/client/store tests

- [x] **Step 1: 写设置界面 RED 测试**

覆盖供应商切换默认 URL、密钥不回显、已保存状态、连接测试、移除密钥和不支持联网时禁用开关。

- [x] **Step 2: 实现设置弹窗**

侧栏底部齿轮按钮打开弹窗。API Key 使用非受控密码输入；保存后立即清空 DOM 值，只显示“已保存”。

- [x] **Step 3: 写概念解释 RED 测试**

覆盖未配置引导、加载、取消、失败保留选区、重新生成、保存后 mark 写入和已有概念提示。

- [x] **Step 4: 实现解释预览**

右键动作打开轻量弹窗；生成期间锁定对应编辑器；保存前生成 `conceptId / mentionId` UUID 并将 mark 应用到原选区，再调用原子保存命令。

- [x] **Step 5: 写对话 RED 测试**

覆盖历史加载、发送、联网开关、引用可点击、失败保留输入、取消和归档只读。

- [x] **Step 6: 实现卡片对话**

消息无嵌套卡片；AI 引用在消息下方显示为可点击来源。通过 Tauri opener 打开外部链接。

- [x] **Step 7: 运行前端测试**

Run:

```bash
npm --prefix desktop test -- AiSettingsDialog ConceptExplainDialog AiConversation CardDetail
```

Expected: PASS。

## 任务 9：重排详情页与同步设计系统

**Files:**
- Modify: `desktop/src/features/inbox/CardDetail.tsx`
- Modify: `desktop/src/features/inbox/CardDetail.test.tsx`
- Modify: `desktop/src/shared/styles/global.css`
- Modify: `desktop/e2e/inbox-layout.spec.ts`

- [x] **Step 1: 写层级 RED 测试**

断言 DOM 顺序为标题、原始内容、我的想法、AI 延伸、来源与附件；元信息使用 `<details>` 且默认未展开。

- [x] **Step 2: 重排详情**

标题保留紧凑单行编辑；原始内容使用结构化编辑器；我的想法保持高优先级文本区；AI 区直接位于主流；附件和来源移入一个折叠区。

- [x] **Step 3: 实现样式**

遵守 `DESIGN.md`：

- 无卡片套卡片。
- 图片保持比例。
- 概念浅绿高亮。
- AI 消息无装饰性渐变。
- 宽度 1100 / 1280 / 1440 无横向溢出和文字重叠。

- [x] **Step 4: 运行组件和 E2E**

Run:

```bash
npm --prefix desktop test -- CardDetail
npm --prefix desktop run test:e2e
```

Expected: PASS。

## 任务 10：全量验证、安全审计和 Mac 交付

**Files:**
- Modify: `desktop/e2e/inbox-layout.spec.ts`
- Modify: `desktop/README.md`
- Modify: existing project verification scripts when required

- [ ] **Step 1: 扩展浏览器桥接数据**

E2E 模拟 `get/save document`、概念、AI 设置和消息命令，覆盖贴图、概念、AI 对话、详情折叠和归档只读。

当前 E2E 已覆盖结构化正文读取、概念工作区、AI 设置与消息读取、右键入口和卡片生命周期；真实贴图保存与 AI 发送仍由组件测试和 Rust 测试覆盖。

- [x] **Step 2: 运行全部自动化验证**

Run:

```bash
npm --prefix desktop test
npm --prefix desktop run lint
npm --prefix desktop run build
npm --prefix desktop run test:rust
npm --prefix desktop run test:native
npm --prefix desktop run test:e2e
```

Expected: 全部退出码为 0。

- [x] **Step 3: 运行密钥扫描**

扫描源码、SQLite、日志和 `.app`，确认无 `sk-` 测试哨兵、Authorization header 值或 Keychain 密钥内容。测试 fixture 只使用不符合真实密钥格式的固定占位符。

- [x] **Step 4: 构建并校验 `.app`**

Run:

```bash
npm --prefix desktop run tauri build -- --bundles app
codesign --verify --deep --strict desktop/src-tauri/target/release/bundle/macos/IdeaCard.app
open desktop/src-tauri/target/release/bundle/macos/IdeaCard.app
```

Expected: 构建成功、签名校验成功、应用可启动。

- [ ] **Step 5: Mac 手工验收**

在真实 `.app` 中验证：

1. 版本 1 数据库升级。
2. 快速记录和详情正文粘贴图片。
3. 重启后图片位置不变。
4. 模型设置写入 Keychain 且不回显。
5. 使用用户自己的 OpenAI Key 进行连接测试（若用户提供）。
6. 按住 `Fn` 的豆包语音文字进入结构化正文；悬浮窗不作为失败。

最终 `.app` 已完成构建、签名校验和启动；用户自己的模型 Key 连接，以及豆包语音进入结构化正文，仍需在真实输入环境中手工确认。

> 2026-07-20 复测结论：豆包语音文字无法进入 IdeaCard 的 Tauri/WKWebView 输入区域。该问题已确认为兼容性问题并暂缓处理，完整记录见 `docs/2026-07-20-doubao-voice-input-compatibility.md`。

- [x] **Step 6: 更新 README**

用中文记录打开 `.app` 的路径、模型设置位置、支持供应商、图片格式和已知的豆包悬浮提示差异。

## 任务 11：AI 延伸与概念解释真流式回答

**Files:**
- Modify: `desktop/src-tauri/src/ai/provider.rs`
- Modify: `desktop/src-tauri/src/ai/openai.rs`
- Modify: `desktop/src-tauri/src/ai/openai_compatible.rs`
- Modify: `desktop/src-tauri/src/ai/anthropic.rs`
- Modify: `desktop/src-tauri/src/commands/ai_commands.rs`
- Modify: `desktop/src/services/desktop/tauriClient.ts`
- Modify: `desktop/src/features/ai/AiConversation.tsx`
- Modify: `desktop/src/features/inbox/CardDetail.tsx`
- Modify: `desktop/src/features/concepts/ConceptExplainDialog.tsx`
- Modify: corresponding Rust、组件与 E2E tests

- [x] **Step 1: 定义流式状态与保存边界**

用户消息发送后立即进入消息流；首段到达前显示轻量等待，首段后连续追加。只有收到供应商完成事件的完整结果才保存，取消、流中断和格式错误都丢弃临时结果。

- [x] **Step 2: 实现三类供应商 SSE**

官方 OpenAI 消费 Responses 增量和完成事件；OpenAI 兼容接口消费 Chat Completions 增量与完成标记；Anthropic 消费 Messages `text_delta` 与 `message_stop`。

- [x] **Step 3: 后台读取并桥接前端事件**

阻塞式 HTTP 长连接放入后台工作线程，按 `requestId` 向前端发送文本增量；取消命令保持可响应，前端监听器在请求结束后释放。

- [x] **Step 4: 实现 AI 延伸与概念解释状态**

AI 延伸乐观显示用户消息并清空 composer；失败或取消时恢复原问题。概念解释立即打开预览，完整生成前禁用保存，重新生成重新进入等待。

- [x] **Step 5: 全量验证与安装包**

Run:

```bash
npm --prefix desktop test
npm --prefix desktop run lint
npm --prefix desktop run build
npm --prefix desktop run test:rust
npm --prefix desktop run test:e2e
npm --prefix desktop run tauri build -- --bundles dmg
```

Expected: 全部退出码为 0；DMG 通过映像与签名校验。

## 参考资料

- [OpenAI Responses API](https://developers.openai.com/api/reference/resources/responses/methods/create)
- [OpenAI Web Search](https://developers.openai.com/api/docs/guides/tools-web-search)
- [Anthropic Messages API](https://platform.claude.com/docs/en/api/messages)
- [TipTap JSON 文档模型](https://tiptap.dev/docs/editor/core-concepts/introduction)
- [TipTap React NodeView](https://tiptap.dev/docs/editor/extensions/custom-extensions/node-views/react)
- [TipTap Image Extension](https://tiptap.dev/docs/editor/extensions/nodes/image)
