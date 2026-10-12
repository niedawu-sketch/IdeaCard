# IdeaCard Mac AI 实施计划

> **供智能执行者使用：** 必须使用 `superpowers:subagent-driven-development`（推荐）或 `superpowers:executing-plans`，逐项执行本计划。步骤使用复选框（`- [ ]`）跟踪。

**目标：** 实现 Keychain 密钥设置、OpenAI Responses API 流式延伸、可控联网查询、卡片与网页引用、连续追问和不可变结论快照。

**架构：** Rust 组装本地上下文、读取 Keychain、调用 OpenAI、解析 SSE 并持久化完整消息；前端只发送会话操作并消费类型化流事件。会话上下文完全由 SQLite 重建，请求设置 `store: false`。

**技术栈：** Rust、reqwest、eventsource-stream、tokio-util、secrecy、OpenAI Responses API、Swift Security framework、React、Vitest

---

## 文件结构

```text
desktop/
├── src/entities/analysis/model.ts
├── src/entities/insight/model.ts
├── src/features/settings/AISettings.tsx
├── src/features/analysis/AnalysisWorkspace.tsx
├── src/features/analysis/AnalysisResult.tsx
├── src/pages/InsightPage.tsx
├── src-tauri/native/Sources/IdeaCardNative/KeychainService.swift
├── src-tauri/src/ai/context.rs
├── src-tauri/src/ai/client.rs
├── src-tauri/src/ai/stream.rs
├── src-tauri/src/repository/analysis_repository.rs
├── src-tauri/src/repository/insight_repository.rs
└── src-tauri/src/commands/ai_commands.rs
```

### 任务 1: 定义 AI、消息、引用和结论契约

**文件：**
- 创建： `desktop/src/entities/analysis/model.ts`
- 创建： `desktop/src/entities/analysis/model.test.ts`
- 创建： `desktop/src/entities/insight/model.ts`
- 创建： `desktop/src-tauri/src/domain/analysis.rs`
- 创建： `desktop/src-tauri/src/domain/insight.rs`

- [ ] **步骤 1：编写上下文限制和流事件失败测试**

```ts
import { describe, expect, it } from 'vitest'
import { canStartAnalysis, reduceStreamEvent } from './model'

it('blocks more than 200000 context characters', () => {
  expect(canStartAnalysis({ cardCount: 2, contextCharacters: 200001, question: '继续吗' }).code).toBe('context_too_large')
})

it('appends text deltas without creating a durable message', () => {
  const next = reduceStreamEvent({ text: '', citations: [], status: 'running' }, { type: 'text_delta', delta: '回应' })
  expect(next.text).toBe('回应')
  expect(next.status).toBe('running')
})
```

- [ ] **步骤 2：运行并确认失败**

运行： `npm test -- src/entities/analysis/model.test.ts`

预期：失败，因为分析模型尚不存在。

- [ ] **步骤 3：实现前端契约**

定义以下固定联合类型：

```ts
export type AnalysisStatus = 'draft' | 'running' | 'completed' | 'failed' | 'cancelled'
export type MessageRole = 'user' | 'assistant'
export type AIStreamEvent =
  | { type: 'started'; requestId: string }
  | { type: 'text_delta'; delta: string }
  | { type: 'web_citation'; url: string; title: string; startIndex: number; endIndex: number }
  | { type: 'completed'; message: AnalysisMessage }
  | { type: 'cancelled' }
  | { type: 'failed'; error: DesktopErrorPayload }
```

定义 `AnalysisSession`、`AnalysisMessage`、`CardCitation`、`WebCitation`、`Insight`、`StartAnalysisInput`、`FollowUpInput`、`canStartAnalysis`，以及用 `never` 做穷尽检查的完整 `reduceStreamEvent`。

- [ ] **步骤 4：镜像 Rust 类型并测试 JSON 名称**

Rust 类型对持久化枚举使用 serde `snake_case`，对流事件使用 `tag = "type", rename_all = "snake_case"`。测试序列化 `TextDelta`、`WebCitation`、`Completed`、`Cancelled` 和 `Failed`，断言字段名与 TypeScript 预期完全一致。

- [ ] **步骤 5：验证并提交**

运行： `npm test -- src/entities/analysis/model.test.ts && npm run test:rust -- domain::analysis::tests`

预期：通过。

```bash
git add desktop/src/entities/analysis desktop/src/entities/insight desktop/src-tauri/src/domain
git commit -m "feat: define AI session contracts"
```

### 任务 2: 实现 macOS Keychain 服务

**文件：**
- 创建： `desktop/src-tauri/native/Sources/IdeaCardNative/KeychainService.swift`
- 创建： `desktop/src-tauri/native/Tests/IdeaCardNativeTests/KeychainServiceTests.swift`
- 修改： `desktop/src-tauri/native/Sources/IdeaCardNative/main.swift`

- [ ] **步骤 1：编写隔离 Keychain 失败测试**

使用服务名 `com.niedawu.ideacard.tests.` 加测试生成的 UUID，账户名为 `api-key`。测试新增与读取、更新现有值、删除、读取缺失项返回 `keychain_item_missing`；测试清理阶段必须删除测试项。

```swift
try service.set(secret: "sk-test-one", service: serviceName, account: account)
XCTAssertEqual(try service.get(service: serviceName, account: account), "sk-test-one")
try service.set(secret: "sk-test-two", service: serviceName, account: account)
XCTAssertEqual(try service.get(service: serviceName, account: account), "sk-test-two")
```

- [ ] **步骤 2：运行并确认失败**

运行： `npm run test:native -- --filter KeychainServiceTests`

预期：失败，因为服务尚不存在。

- [ ] **步骤 3：实现 Security framework 查询**

使用 `kSecClassGenericPassword`、`kSecAttrService`、`kSecAttrAccount`、`kSecValueData`、`SecItemAdd`、`SecItemCopyMatching`、`SecItemUpdate` 和 `SecItemDelete`。绝不打印查询字典或返回数据；把 `errSecItemNotFound`、转换失败及其他 OSStatus 映射为稳定错误码。

- [ ] **步骤 4：接入 sidecar 协议**

支持 `.keychainSet`、`.keychainGet` 和 `.keychainDelete`。服务名和账户名必填；设置操作还要求密钥非空。读取结果只允许通过 `NativeResponse.text` 返回，标准错误和错误信息均不得出现密钥。

- [ ] **步骤 5：验证并提交**

运行： `npm run test:native`

预期：全部 Swift 测试通过，测试密钥已移除。

```bash
git add desktop/src-tauri/native
git commit -m "feat: store OpenAI credentials in Keychain"
```

### 任务 3: 实现设置命令和连接测试

**文件：**
- 创建： `desktop/src-tauri/src/ai/mod.rs`
- 创建： `desktop/src-tauri/src/ai/credentials.rs`
- 创建： `desktop/src-tauri/src/commands/settings_commands.rs`
- 创建： `desktop/src/features/settings/AISettings.tsx`
- 创建： `desktop/src/features/settings/AISettings.test.tsx`
- 修改： `desktop/src/services/desktop/contracts.ts`

- [ ] **步骤 1：添加 HTTP 与秘密类型依赖**

运行：

```bash
cd desktop/src-tauri
cargo add reqwest --features json,stream,rustls-tls
cargo add secrecy async-trait futures-util eventsource-stream tokio-util regex
```

- [ ] **步骤 2：编写凭据和设置界面失败测试**

Rust 假原生传输层验证 `set_api_key`、`has_api_key` 和 `delete_api_key` 使用服务 `com.niedawu.ideacard.openai` 与账户 `api-key`，拒绝空密钥，且 `Debug` 输出绝不包含密钥。

React 测试断言密码输入框初始为空，已保存状态只显示 `已配置`，测试连接失败时保留新输入，保存成功后清空输入框，删除操作必须经过确认对话框。

- [ ] **步骤 3：运行并确认失败**

运行： `npm run test:rust -- ai::credentials::tests && npm test -- src/features/settings/AISettings.test.tsx`

预期：失败，因为凭证服务和设置界面尚不存在。

- [ ] **步骤 4：实现设置命令**

暴露以下命令：

```rust
set_openai_api_key(secret: secrecy::SecretString) -> Result<(), AppError>
has_openai_api_key() -> Result<bool, AppError>
delete_openai_api_key() -> Result<(), AppError>
test_openai_connection() -> Result<ConnectionTestResult, AppError>
```

`test_openai_connection` sends a non-streaming Responses request to `gpt-5.4-mini` with `store: false`, no tools, input `只回复 OK`, and a small output limit. Map authentication, permission/model access, quota, rate limit, timeout, and service errors separately. Do not return response body on error.

- [ ] **步骤 5：实现设置界面并验证**

使用密码输入框，绝不从 Keychain 回填现有值。显示配置状态、保存、测试连接、删除、录屏权限状态、快捷键注册状态、数据目录和应用版本。加入已确认的隐私说明：用户选择的材料会发送给 OpenAI，并可能出现在滥用监控日志中。

运行： `npm run test:rust -- ai::credentials::tests && npm test -- src/features/settings/AISettings.test.tsx`

预期：通过。

- [ ] **步骤 6：提交设置能力**

```bash
git add desktop/src-tauri/src/ai desktop/src-tauri/src/commands desktop/src/features/settings desktop/src/services/desktop/contracts.ts desktop/src-tauri/Cargo.toml desktop/src-tauri/Cargo.lock
git commit -m "feat: configure OpenAI securely"
```

### 任务 4: 实现本地 AI 会话与结论仓库

**文件：**
- 创建： `desktop/src-tauri/src/repository/analysis_repository.rs`
- 创建： `desktop/src-tauri/src/repository/insight_repository.rs`

- [ ] **步骤 1：编写会话事务失败测试**

测试必须证明：创建会话需要 1 至 20 张现有卡片；卡片位置保持稳定；问题更新会持久化；只写入完整消息；卡片引用和网页引用与消息共用事务；会话状态转换合法；插入失败会同时回滚消息及引用。

- [ ] **步骤 2：编写不可变结论失败测试**

创建含两条完整消息的会话并保存结论，再追加第三条会话消息；断言结论仍只包含映射的前两条消息且正文不变。另断言来源消息缺失时返回 `analysis_message_not_found`。

- [ ] **步骤 3：运行并确认失败**

运行： `npm run test:rust -- repository::analysis_repository::tests repository::insight_repository::tests`

预期：失败，因为仓库尚不存在。

- [ ] **步骤 4：实现仓库接口**

```rust
pub trait AnalysisRepository {
    fn create_session(&self, card_ids: &[String], question: &str, web_search_enabled: bool) -> Result<AnalysisSession, AppError>;
    fn get_session(&self, id: &str) -> Result<AnalysisSessionDetail, AppError>;
    fn list_sessions(&self) -> Result<Vec<AnalysisSession>, AppError>;
    fn set_status(&self, id: &str, status: AnalysisStatus) -> Result<(), AppError>;
    fn save_complete_message(&self, message: CompleteMessageInput) -> Result<AnalysisMessage, AppError>;
}

pub trait InsightRepository {
    fn save_snapshot(&self, session_id: &str, through_message_id: &str, title: &str) -> Result<Insight, AppError>;
    fn get(&self, id: &str) -> Result<InsightDetail, AppError>;
    fn list(&self) -> Result<Vec<Insight>, AppError>;
}
```

消息与引用共用事务，结论与 `insight_messages` 共用事务。绝不更新已有结论快照。

- [ ] **步骤 5：验证并提交**

运行： `npm run test:rust -- repository::analysis_repository::tests repository::insight_repository::tests`

预期：通过。

```bash
git add desktop/src-tauri/src/repository
git commit -m "feat: persist AI sessions and insights"
```

### 任务 5: 构建受控 AI 上下文和卡片引用

**文件：**
- 创建： `desktop/src-tauri/src/ai/context.rs`
- 创建： `desktop/src-tauri/src/ai/context_test.rs`

- [ ] **步骤 1：编写字符限制和提示词失败测试**

使用两张固定卡片，断言生成上下文包含稳定标签 `C1`/`C2`、标题/内容/笔记分隔、不包含未选择卡片，并明确只能引用 `[C1]` 或 `[C2]`。总字符数为 200001 时返回 `context_too_large`，并按字符数降序列出最大卡片 ID。

- [ ] **步骤 2：运行并确认失败**

运行： `npm run test:rust -- ai::context::tests`

预期：失败，因为上下文构建器尚不存在。

- [ ] **步骤 3：实现上下文构建器**

```rust
pub struct BuiltContext {
    pub developer_prompt: String,
    pub input_messages: Vec<ResponseInput>,
    pub card_labels: HashMap<String, String>,
    pub character_count: usize,
}

pub fn build_context(session: &AnalysisSessionDetail) -> Result<BuiltContext, AppError>;
```

统计卡片标题、内容、笔记和本地历史中的 Unicode 标量值，绝不截断。开发者提示词要求中文回应包含：直接回应、可延伸方向、查询到的补充信息、下一步行动、尚未解决或需要确认的问题；禁止虚构卡片标签，并要求网页事实保留工具引用。

- [ ] **步骤 4：实现卡片引用解析**

只解析 `\[(C[1-9][0-9]*)\]`；通过 `card_labels` 映射标签；保留首次出现顺序；忽略未知标签；返回清理后的显示文本，以及引用记录和位置。

- [ ] **步骤 5：验证并提交**

运行： `npm run test:rust -- ai::context::tests`

预期：通过。

```bash
git add desktop/src-tauri/src/ai/context.rs desktop/src-tauri/src/ai/context_test.rs
git commit -m "feat: build traceable AI context"
```

### 任务 6: 实现 OpenAI Responses 流式客户端与取消

**文件：**
- 创建： `desktop/src-tauri/src/ai/client.rs`
- 创建： `desktop/src-tauri/src/ai/stream.rs`
- 创建： `desktop/src-tauri/src/ai/client_test.rs`

- [ ] **步骤 1：编写请求体失败测试**

断言 JSON 字段精确为：模型 `gpt-5.4-mini`、`store: false`、`stream: true` 和输入消息；关闭联网时不存在 `tools` 字段，开启时 `tools` 等于 `[{"type":"web_search"}]`。API Key 只能出现在 Authorization 请求头中。

- [ ] **步骤 2：编写 SSE 解析失败测试**

依次输入确定性的 `response.created`、两个 `response.output_text.delta`、一个 URL 引用注解、`response.completed`、API 错误、畸形事件和流结束事件。断言文本、引用位置、请求 ID 和终止事件；完成前遇到流结束时返回 `ai_stream_interrupted`。

- [ ] **步骤 3：运行并确认失败**

运行： `npm run test:rust -- ai::client_test ai::stream::tests`

预期：失败，因为客户端和解析器尚不存在。

- [ ] **步骤 4：实现请求和流解析**

向 `https://api.openai.com/v1/responses` 发起 POST，请求包含从 Keychain 读取的 Bearer 密钥、JSON 正文、超时时间和 `X-Client-Request-Id`。使用 `eventsource-stream`，处理已知事件并安全忽略新增的未知事件类型。输出在 `response.completed` 前只保留在内存中；向界面发送增量，但只持久化完整最终文本和引用。

- [ ] **步骤 5：实现取消注册表**

在异步互斥锁后保存 `HashMap<session_id, CancellationToken>`。拒绝同一会话的第二个并发请求。取消命令触发 token、把状态改为 cancelled、移除注册项，且绝不持久化助手的部分输出；通过类似 finally 的守卫保证注册项最终被移除。

- [ ] **步骤 6：验证并提交**

运行： `npm run test:rust -- ai::client_test ai::stream::tests ai::cancellation::tests`

预期：通过，且测试不访问网络。

```bash
git add desktop/src-tauri/src/ai desktop/src-tauri/Cargo.toml desktop/src-tauri/Cargo.lock
git commit -m "feat: stream OpenAI idea extensions"
```

### 任务 7: 暴露 AI 命令和前端流服务

**文件：**
- 创建： `desktop/src-tauri/src/commands/ai_commands.rs`
- 修改： `desktop/src-tauri/src/lib.rs`
- 修改： `desktop/src/services/desktop/contracts.ts`
- 修改： `desktop/src/services/desktop/tauriClient.ts`
- 创建： `desktop/src/services/desktop/aiStream.test.ts`

- [ ] **步骤 1：定义命令接口**

```rust
create_analysis_session(card_ids, question, web_search_enabled)
get_analysis_session(id)
start_analysis(id, channel: tauri::ipc::Channel<AIStreamEvent>)
cancel_analysis(id)
ask_follow_up(id, question, channel)
save_insight(id, through_message_id, title)
list_insights()
get_insight(id)
```

- [ ] **步骤 2：编写前端流失败测试**

mock `invoke` 和 Channel 回调；断言 started、增量、引用、completed、cancelled、failed 事件按顺序各传递一次。只有状态为 running 时，组件卸载才调用取消。

- [ ] **步骤 3：运行并确认失败**

运行： `npm test -- src/services/desktop/aiStream.test.ts`

预期：失败，因为流客户端尚不存在。

- [ ] **步骤 4：实现命令与客户端**

命令负责校验 ID 和去除问题首尾空白、更新状态、构建本地上下文；只有校验完成后才读取 Keychain，再运行客户端、持久化完整消息，并且只发送一次终止事件。前端在 `DesktopService.startAnalysis` 内构造 Tauri Channel，并暴露取消订阅和取消执行句柄。

- [ ] **步骤 5：验证并提交**

运行： `npm test -- src/services/desktop/aiStream.test.ts && npm run test:rust -- commands::ai_commands::tests`

预期：通过。

```bash
git add desktop/src-tauri/src/commands desktop/src-tauri/src/lib.rs desktop/src/services/desktop
git commit -m "feat: expose streaming AI commands"
```

### 任务 8: 实现 AI 工作台、追问和结论页面

**文件：**
- 创建： `desktop/src/features/analysis/AnalysisWorkspace.tsx`
- 创建： `desktop/src/features/analysis/AnalysisWorkspace.test.tsx`
- 创建： `desktop/src/features/analysis/AnalysisResult.tsx`
- 创建： `desktop/src/pages/AnalysisPage.tsx`
- 创建： `desktop/src/pages/InsightPage.tsx`
- 创建： `desktop/src/pages/InsightPage.test.tsx`
- 修改： `desktop/src/app/App.tsx`
- 修改： `desktop/src/pages/InboxPage.tsx`

- [ ] **步骤 1：编写 AI 工作台失败测试**

测试 1 至 20 张卡片选择、空问题禁用、200001 字符时阻止并列出最大卡片、联网开关默认开启、运行中显示增量、取消后保留问题、重试、卡片引用跳转、外部网页引用打开、追问，以及只有完整助手消息产生后才允许保存结论。

- [ ] **步骤 2：编写结论快照失败测试**

渲染包含两条映射消息和两类引用的结论。断言标题、问题、正文、卡片跳转、网页 URL 和继续会话动作；再追加一条更晚的假会话消息，断言结论视图中不存在该消息。

- [ ] **步骤 3：运行并确认失败**

运行： `npm test -- src/features/analysis src/pages/InsightPage.test.tsx`

预期：失败，因为页面尚不存在。

- [ ] **步骤 4：实现 AI 页面**

从原型迁移已验证的双栏材料/工作区布局，不保留分析模式分类。提供明确的联网搜索开关、实时状态、取消/重试、五个回应区段；卡片引用使用内部按钮，网页引用通过系统浏览器打开，追问输入框绑定当前会话。

- [ ] **步骤 5：实现结论页**

只渲染结论映射的快照消息，绝不通过查询“会话最新消息”补全结论。继续会话时返回原会话，不修改已保存结论。

- [ ] **步骤 6：验证并提交**

运行： `npm test && npm run lint && npm run build && npm run test:rust && npm run test:native`

预期：通过。

```bash
git add desktop/src
git commit -m "feat: add AI extension and insight workflow"
```

### 任务 9: 验证阶段 3 真实 OpenAI 流程

**文件：**
- 修改： `desktop/README.md`

- [ ] **步骤 1：运行无网络自动验证**

运行： `npm test && npm run lint && npm run build && npm run test:rust && npm run test:native`

预期：通过；HTTP 使用 mock，Keychain 测试项相互隔离。

- [ ] **步骤 2：在设置中录入真实 API Key**

只在应用界面输入 Key。确认它从未进入 `.env`、shell 历史、仓库文件、截图、测试夹具或日志。运行测试连接并确认成功。

- [ ] **步骤 3：验收真实会话**

至少选择两张卡片，分别在关闭和开启联网搜索时各执行一次回应；验证流式输出和网页引用，追问一次，取消一次执行，重试一次失败，保存结论后继续会话，并验证先前结论保持不变。

- [ ] **步骤 4：重启并复核**

退出并重新打开 `.app`；验证 API 已配置状态、会话历史、引用和结论。随后在设置中移除 Key，验证新的 AI 请求被阻止，同时本地业务数据仍保留。

- [ ] **步骤 5：记录并提交验收**

记录 pass/fail and non-secret request IDs in `desktop/README.md` under `阶段 3 验收`.

```bash
git add desktop/README.md
git commit -m "docs: record AI workflow verification"
```
