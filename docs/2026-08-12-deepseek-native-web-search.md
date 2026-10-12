# DeepSeek 原生联网查询实施计划

## 目标

让使用 DeepSeek API 的用户可以在「AI 延伸」中显式开启联网查询，并继续沿用现有流式回复、网页引用展示和本地对话持久化链路。

## 第一阶段边界

- 增加独立的 `DeepSeek` 供应商，不再要求用户伪装成 OpenAI 接入。
- 默认接口地址为 `https://api.deepseek.com`，默认模型为 `deepseek-v4-flash`。
- 只为精确匹配的 `deepseek-v4-flash` 开放联网查询；`deepseek-v4-pro` 与其他 DeepSeek 模型不进入本阶段。
- 不引入第三方搜索服务，不增加新的 API Key 种类。
- 不修改数据库 schema；AI 设置继续以 JSON 保存在现有 `app_settings` 中。
- 不自动迁移旧的 OpenAI 兼容接口密钥；用户首次切换到 DeepSeek 时重新输入密钥。

## 实现方案

1. TypeScript 与 Rust 的供应商枚举增加 `deepSeek` / `DeepSeek`。
2. 在领域层建立统一联网能力判断：OpenAI 支持；DeepSeek 仅 `deepseek-v4-flash` 支持；其他接入不支持。
3. DeepSeek 复用现有 Responses API 请求、SSE 流解析和 `url_citation` 引用解析。启用联网时发送 `tools: [{"type":"web_search"}]`，并为 DeepSeek 设置 `tool_choice: {"type":"web_search"}`。
4. 模型设置页中增加 DeepSeek 选项和默认值；不支持的 DeepSeek 模型禁用联网开关并给出短说明。
5. AI 延伸中的联网开关只在「全局允许 + 供应商/模型支持」时可用，消除界面可勾选但后端静默忽略的状态。

## 验收标准

- DeepSeek 默认设置精确为 `https://api.deepseek.com` / `deepseek-v4-flash`。
- DeepSeek 联网请求发往 `/responses`，包含 `web_search` 工具和强制工具选择。
- DeepSeek 流式回复与引用解析通过自动化测试。
- 将 DeepSeek 模型改为其他名称后，全局联网开关立即关闭并禁用，保存时不会夹带无效联网配置。
- 全局联网未开启时，AI 延伸中的单次联网选项不可用。
- OpenAI、OpenAI 兼容接口和 Anthropic 的现有请求协议不受影响。
