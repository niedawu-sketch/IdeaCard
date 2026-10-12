# AI 回答分层呈现与操作实施规格

## 目标

修复 DeepSeek 联网回答缺少来源的问题，并让 AI 延伸清晰区分思考过程、最终回答和检索来源。最终回答应便于阅读、复制和重新生成，思考过程不得占据主要视觉注意力。

## 协议规则

- Responses API 的 `reasoning` 项进入思考过程。
- 最后一个非空 `message` 项作为最终回答；它之前的过程性 `message` 合并进思考过程。
- `output_text.annotations[].url_citation` 与 `web_search_call.action.sources[].url` 都进入来源列表，按 URL 保序去重。
- 来源缺少标题时使用 URL 域名；无效或非 HTTP(S) URL 不保存、不展示。
- 流式事件区分 `reasoning` 和 `answer`，完成事件的完整响应是最终持久化事实来源。

## 数据迁移

数据库升级为 v3：

- `card_ai_messages.reasoning_content TEXT NOT NULL DEFAULT ''`：保存 AI 思考过程。
- `card_ai_messages.web_search_enabled INTEGER NOT NULL DEFAULT 0`：保存每次用户提问的联网选择。
- 既有消息保持原内容与引用不变；既有思考为空、联网选择为关闭。

## 交互规则

- 最终回答使用安全 Markdown，只支持展示型语法，不执行原始 HTML。
- 思考过程使用原生 `details` 折叠，完成后默认关闭；只有思考内容存在时显示。
- 复制只复制最终回答的 Markdown 源文本，成功后按钮短暂显示「已复制」。
- 最新 AI 回答重新生成时原位替换该回答与来源，不重复插入用户问题。
- 历史 AI 回答重新生成时，把对应原问题作为新一轮追加到对话末尾，保留后续历史的一致性。
- 重新生成沿用原问题当时的联网选择；旧数据无法还原时默认不联网。

## 验收

- DeepSeek 联网回答至少能从 `web_search_call.action.sources` 保存并显示来源。
- reasoning 和最终回答在流式过程及重载后都保持分离。
- Markdown 不再显示裸 `##`、`**` 和反引号，危险链接与原始 HTML 不执行。
- 复制、最新回答原位重新生成、历史回答追加新尝试均有自动化测试。
- 数据库从 v2 升级后既有卡片、AI 消息和引用数量不变。
