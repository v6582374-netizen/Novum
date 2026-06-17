# Phase 3 开发规格：PaperQA 真实问答闭环

## 目标

Phase 3 的目标是把 Novum 从“真实 PDF 工作台”推进为第一条真实科研问答闭环：

1. 用户配置自己的 OpenAI-compatible 模型服务。
2. 用户选择一篇已经导入的 PDF。
3. Novum 调用本地 Python 研究服务为文献建立 PaperQA 索引。
4. 用户围绕当前文献提问。
5. PaperQA 返回真实答案、引用和来源摘录。
6. 用户点击引用后，右侧 PDF 预览跳转到对应页。
7. 问答记录、引用、任务状态和错误日志保存到本地。

本阶段必须接入真实 PaperQA 调用，不允许用模拟回答冒充研究结果。

## 范围

本阶段必须实现：

- OpenAI-compatible provider 配置。
- API Key 本地安全存储。
- 本地 Python 研究服务。
- 当前文献 PaperQA 索引。
- 当前文献 PaperQA 问答。
- 答案、引用和来源摘录展示。
- 引用点击跳转 PDF 页码。
- 索引、问答、模型调用的运行状态和错误反馈。
- 本地持久化问答线程、引用和研究任务记录。

本阶段不实现：

- 多论文集合问答。
- science-skills 真实执行。
- GPT Researcher 深度研究报告流。
- 正式内置 Python runtime 打包。
- 云同步、账户系统、团队协作。
- Windows 打包。
- Homebrew 发布。

## 上游与运行时选择

PaperQA 来源：

- 上游仓库：<https://github.com/Future-House/paper-qa>
- Novum 前端和 Tauri 层不得直接依赖 PaperQA 内部类型。
- PaperQA 必须通过 Novum 自有 `services/research` adapter 调用。

本阶段运行时策略：

- 开发环境要求 Python 3.11+。
- 使用本地虚拟环境安装 PaperQA 及研究服务依赖。
- Tauri/Rust 在开发模式下启动或连接本地 Python HTTP 服务。
- 正式发布时如何内置 Python runtime 延后到发布规格中处理。

## 本地研究服务

新增目录：

```text
services/
  research/
    pyproject.toml
    README.md
    novum_research/
      app.py
      paperqa_adapter.py
      settings.py
      schemas.py
      storage.py
    tests/
```

服务形态：

- 使用本地 HTTP 服务作为 Rust/Tauri 与 Python 工具之间的边界。
- 开发阶段默认监听 `127.0.0.1` 的随机空闲端口。
- 端口由 Tauri 启动服务后记录，不写死。
- 服务仅接受本机连接。
- 所有请求和响应使用 Novum 自有 JSON schema。

最小 HTTP API：

```text
GET  /health
POST /documents/index
POST /documents/ask
GET  /runs/{runId}
```

`GET /health` 响应：

```ts
type ResearchHealth = {
  ok: boolean
  serviceVersion: string
  paperqaAvailable: boolean
}
```

`POST /documents/index` 请求：

```ts
type IndexDocumentRequest = {
  documentId: string
  pdfPath: string
  indexPath: string
  provider: OpenAICompatibleProvider
}
```

`POST /documents/index` 响应：

```ts
type ResearchRunCreated = {
  runId: string
  status: 'queued' | 'running'
}
```

`POST /documents/ask` 请求：

```ts
type AskDocumentRequest = {
  documentId: string
  pdfPath: string
  indexPath: string
  question: string
  provider: OpenAICompatibleProvider
}
```

`POST /documents/ask` 响应：

```ts
type AskDocumentResponse = {
  runId: string
  answer: string
  citations: QaCitation[]
}
```

`GET /runs/{runId}` 响应：

```ts
type ResearchRun = {
  id: string
  kind: 'index_document' | 'ask_document'
  status: 'queued' | 'running' | 'succeeded' | 'failed'
  documentId: string
  startedAt: string
  finishedAt: string | null
  error: string | null
  logs: ResearchRunLog[]
}

type ResearchRunLog = {
  timestamp: string
  level: 'info' | 'warning' | 'error'
  message: string
}
```

引用结构：

```ts
type QaCitation = {
  id: string
  documentId: string
  title: string
  page: number | null
  excerpt: string
  sourceLabel: string
  confidence: number | null
}
```

当 PaperQA 无法稳定给出页码时，`page` 可以为 `null`，但 UI 必须提示“来源页码暂不可用”，不得跳转到错误页。

## Tauri/Rust 接口

新增 Tauri commands：

```ts
get_provider_settings(): ProviderSettings
save_provider_settings(settings: ProviderSettingsInput): ProviderSettings
test_provider_connection(): ProviderConnectionResult
index_document(id: string): ResearchRun
ask_document(id: string, question: string): AskDocumentResult
get_research_run(id: string): ResearchRun
```

Provider 类型：

```ts
type ProviderSettings = {
  provider: 'openai-compatible'
  baseUrl: string
  model: string
  hasApiKey: boolean
  updatedAt: string | null
}

type ProviderSettingsInput = {
  provider: 'openai-compatible'
  baseUrl: string
  model: string
  apiKey?: string
}

type ProviderConnectionResult = {
  ok: boolean
  message: string
  checkedAt: string
}
```

问答结果类型：

```ts
type AskDocumentResult = {
  run: ResearchRun
  answer: QaAnswer
}

type QaAnswer = {
  id: string
  documentId: string
  question: string
  answer: string
  citations: QaCitation[]
  createdAt: string
}
```

命令行为：

- `save_provider_settings` 必须把 API Key 写入 Tauri Stronghold。
- SQLite 只能保存 `provider`、`baseUrl`、`model`、`updatedAt` 和 `hasApiKey`。
- `index_document` 必须检查文献是否存在、PDF 文件是否存在、provider 是否可用。
- `ask_document` 必须检查文献是否已索引；未索引时返回中文错误。
- 所有错误必须返回中文用户可读消息，同时在 `research_runs` 中记录技术日志。

## 本地数据

沿用现有 `documents` 表，并扩展状态语义：

```ts
type DocumentStatus =
  | '已导入'
  | '索引中'
  | '已索引'
  | '索引失败'
  | '问答失败'
```

新增 SQLite 表：

```sql
CREATE TABLE IF NOT EXISTS provider_settings (
  id TEXT PRIMARY KEY,
  provider TEXT NOT NULL,
  base_url TEXT NOT NULL,
  model TEXT NOT NULL,
  has_api_key INTEGER NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS document_indexes (
  document_id TEXT PRIMARY KEY,
  status TEXT NOT NULL,
  index_path TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  error TEXT,
  FOREIGN KEY(document_id) REFERENCES documents(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS research_runs (
  id TEXT PRIMARY KEY,
  kind TEXT NOT NULL,
  status TEXT NOT NULL,
  document_id TEXT NOT NULL,
  started_at TEXT NOT NULL,
  finished_at TEXT,
  error TEXT,
  logs_json TEXT NOT NULL,
  FOREIGN KEY(document_id) REFERENCES documents(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS qa_threads (
  id TEXT PRIMARY KEY,
  document_id TEXT NOT NULL,
  question TEXT NOT NULL,
  answer TEXT NOT NULL,
  run_id TEXT NOT NULL,
  created_at TEXT NOT NULL,
  FOREIGN KEY(document_id) REFERENCES documents(id) ON DELETE CASCADE,
  FOREIGN KEY(run_id) REFERENCES research_runs(id)
);

CREATE TABLE IF NOT EXISTS qa_citations (
  id TEXT PRIMARY KEY,
  thread_id TEXT NOT NULL,
  document_id TEXT NOT NULL,
  page INTEGER,
  excerpt TEXT NOT NULL,
  source_label TEXT NOT NULL,
  confidence REAL,
  position INTEGER NOT NULL,
  FOREIGN KEY(thread_id) REFERENCES qa_threads(id) ON DELETE CASCADE,
  FOREIGN KEY(document_id) REFERENCES documents(id) ON DELETE CASCADE
);
```

本地目录约定：

```text
<app-data>/
  documents/
  indexes/
    paperqa/
      <document-id>/
  runs/
  novum.sqlite3
```

## 用户体验

模型配置：

- “模型配置”面板从占位改为真实表单。
- 字段包含 Base URL、Model、API Key。
- API Key 输入框默认不回显已保存值，只显示“已保存密钥”状态。
- 保存后显示中文成功/失败提示。
- “测试连接”必须调用真实 provider 测试，失败时展示 provider 返回的关键信息。

文献索引：

- 文献列表显示索引状态。
- 当前文献未索引时，问答区主按钮显示“索引当前文献”。
- 索引中时显示进度/日志摘要，并禁用重复索引按钮。
- 索引失败时显示“重试索引”。

论文问答：

- 当前文献已索引后，问答区允许输入问题。
- 提问按钮触发真实 PaperQA 调用。
- 问答运行中显示状态，不允许重复提交同一问题。
- 返回后展示答案正文、引用列表和运行日志入口。
- 引用列表中的每条引用必须可点击。
- 有页码的引用点击后跳转右侧 PDF；无页码的引用点击后只高亮引用卡片并提示页码不可用。

PDF 回链：

- 保留当前右侧连续 PDF 预览。
- 点击引用后更新当前页。
- 当前引用页的页码标签需要有明显高亮状态。
- 不因为引用跳转改变用户选择的文献。

错误状态：

- 未配置 provider：提示先到模型配置填写 API Key。
- Provider 测试失败：提示检查 Base URL、模型名和 API Key。
- Python 服务未启动：提示研究服务不可用，并提供重试。
- PaperQA 索引失败：显示中文摘要，技术细节进入运行日志。
- 问答失败：保留用户问题输入，允许重试。

## 安全与凭据

- API Key 必须使用 Tauri Stronghold 保存。
- API Key 不得写入 SQLite、日志、前端状态持久化或错误消息。
- 前端永远只接收 `hasApiKey`，不得接收完整 API Key。
- 调用 Python 服务时，Rust/Tauri 层负责注入运行所需密钥。
- 运行日志必须过滤常见密钥字段：`api_key`、`authorization`、`bearer`、`token`。

## 验收标准

自动检查：

- `npm run desktop:build` 通过。
- `npm run desktop:lint` 通过。
- `cargo check` 通过。
- `pytest services/research/tests` 通过。

手动验收：

- 启动 `npm run desktop:tauri` 后，模型配置表单可保存 OpenAI-compatible provider。
- 重启应用后，Base URL 和模型名仍存在，API Key 不明文显示。
- 未配置 provider 时，索引和问答给出中文提示。
- 导入 PDF 后可以索引当前文献，索引状态持久化为 `已索引`。
- 对已索引文献提问后，返回真实 PaperQA 答案和引用。
- 点击有页码引用后，右侧 PDF 跳转到对应页。
- 删除文献后，相关索引记录、问答记录和引用记录不再显示。
- 断网、API Key 错误、模型名错误、Python 服务崩溃时，应用不白屏，并显示中文可恢复错误。

## 开发顺序

建议按以下顺序实现：

1. 新增 `services/research` Python 服务骨架和 `/health`。
2. 在 Tauri 中增加研究服务启动、健康检查和错误处理。
3. 增加 Stronghold provider 设置保存与读取。
4. 扩展 SQLite 表和 Rust 数据类型。
5. 接入 PaperQA 索引当前文献。
6. 接入 PaperQA 当前文献问答。
7. 实现前端模型配置、索引状态、问答区和引用列表。
8. 实现引用点击跳转右侧 PDF。
9. 补齐失败场景、日志过滤和自动测试。

## 后续阶段预留

Phase 3 完成后，下一阶段可以继续推进：

- 多论文集合问答。
- 文献内搜索与引用片段定位高亮。
- science-skills 技能市场真实注册表。
- GPT Researcher 风格深度研究任务流。
- 正式内置 Python runtime 和 macOS 分发链路。
