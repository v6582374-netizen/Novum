# Phase 4 开发规格：Science Skills 技能市场与本地运行器

## 目标

Phase 4 的目标是把 Novum 从“单篇文献问答工作台”推进到“可调用科学技能的科研 IDE”：

1. 将 `google-deepmind/science-skills` 作为首个技能来源接入 Novum。
2. 用户可以在应用内搜索、筛选、查看技能说明。
3. 用户可以通过点击或命令面板调用技能。
4. 技能执行走 Novum 本地研究服务，不允许前端直接执行任意脚本。
5. 技能输入、运行状态、日志、产物和错误信息保存到本地。
6. `scripts/` 与 `references/` 作为执行依赖保留，但不作为普通用户可浏览的目录树暴露。

本阶段的关键判断是：先完成可审计、可追踪、可失败恢复的技能执行闭环，再考虑更复杂的 GPT Researcher 报告流或用户自定义技能市场。

## 范围

本阶段必须实现：

- `science-skills` 上游来源记录。
- 技能目录导入策略。
- `SKILL.md` 元数据解析。
- 技能列表、搜索、筛选与详情页。
- 命令面板调用技能。
- 技能输入表单。
- 本地技能运行器。
- 运行状态、日志、错误和输出展示。
- 运行记录本地持久化。
- API Key、模型配置、环境变量缺失时的中文错误提示。

本阶段不实现：

- 云端技能商店。
- 用户自定义技能发布。
- 多用户权限系统。
- Docker/Firecracker 级别沙箱。
- GPT Researcher 深度研究报告流。
- Windows 打包。
- Homebrew 发布。

## 上游与来源记录

上游项目：

- 仓库：<https://github.com/google-deepmind/science-skills>
- 许可证：引入时必须记录上游仓库声明的 license。
- 导入方式：优先使用 `vendor/science-skills` 源码快照。

新增来源记录文件：

```text
vendor/
  science-skills/
licenses/
  science-skills/
    LICENSE
    NOTICE.md
patches/
  science-skills/
    README.md
```

`licenses/science-skills/NOTICE.md` 必须包含：

- 上游 URL
- commit SHA 或 release tag
- 导入日期
- license
- 本地改动说明
- 后续同步升级命令

Novum 不直接修改上游源码。确需改动时使用 `patches/science-skills/` 记录补丁原因、影响范围和回滚方式。

## 技能目录模型

上游 `science-skills` 的用户可见入口只来自 `skills/**/SKILL.md`。

需要隐藏的内容：

- `scripts/`
- `references/`
- 临时文件、缓存文件
- 上游测试与开发脚本

隐藏不等于删除。运行器可以读取这些内容作为依赖，但普通 UI 不把它们显示成文件浏览器。

技能解析后的 Novum 类型：

```ts
type ScienceSkill = {
  id: string
  name: string
  description: string
  domain: string
  source: 'science-skills'
  sourcePath: string
  upstreamCommit: string
  requiredInputs: SkillInputSpec[]
  requiredEnv: string[]
  executionMode: 'python' | 'prompt' | 'hybrid'
  status: '可用' | '缺少依赖' | '需要配置' | '不可用'
  updatedAt: string
}

type SkillInputSpec = {
  name: string
  label: string
  type: 'text' | 'textarea' | 'file' | 'select' | 'number' | 'boolean'
  required: boolean
  defaultValue: string | number | boolean | null
  help: string | null
}
```

解析策略：

- `SKILL.md` 的标题作为默认名称。
- 首段正文作为默认描述。
- 目录名作为稳定 `id` 的基础。
- 无法自动推断的输入先使用通用 `textarea`，由用户填写任务上下文。
- 对需要 API key、外部模型或 Python 依赖的技能标记为 `需要配置` 或 `缺少依赖`。

## 本地研究服务 API

扩展 `services/research`。

新增 HTTP API：

```text
GET  /skills
GET  /skills/{skillId}
POST /skills/{skillId}/run
GET  /skill-runs/{runId}
```

`GET /skills` 响应：

```ts
type ListSkillsResponse = {
  skills: ScienceSkill[]
}
```

`POST /skills/{skillId}/run` 请求：

```ts
type RunSkillRequest = {
  inputs: Record<string, unknown>
  context: {
    activeDocumentId: string | null
    activeDocumentPath: string | null
    selectedText: string | null
    provider: OpenAICompatibleProvider | null
  }
}
```

响应：

```ts
type RunSkillResponse = {
  run: SkillRun
}
```

运行记录：

```ts
type SkillRun = {
  id: string
  skillId: string
  skillName: string
  status: 'queued' | 'running' | 'succeeded' | 'failed'
  startedAt: string
  finishedAt: string | null
  error: string | null
  logs: SkillRunLog[]
  outputs: SkillRunOutput[]
}

type SkillRunLog = {
  timestamp: string
  level: 'info' | 'warning' | 'error'
  message: string
}

type SkillRunOutput = {
  id: string
  kind: 'markdown' | 'json' | 'file' | 'text'
  title: string
  content: string
  filePath: string | null
}
```

## Tauri/Rust 接口

新增 Tauri commands：

```ts
list_skills(): ScienceSkill[]
get_skill(id: string): ScienceSkill
run_skill(id: string, inputs: Record<string, unknown>): SkillRun
get_skill_run(id: string): SkillRun
```

命令行为：

- Rust 层负责启动或连接本地研究服务。
- Rust 层负责读取当前文献上下文并传入研究服务。
- 前端不得拼接 shell 命令。
- 所有错误转换为中文用户可读消息。
- 技术日志写入本地数据库。

## 本地数据

新增 SQLite 表：

```sql
CREATE TABLE IF NOT EXISTS skills_cache (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  description TEXT NOT NULL,
  domain TEXT NOT NULL,
  source TEXT NOT NULL,
  source_path TEXT NOT NULL,
  upstream_commit TEXT NOT NULL,
  execution_mode TEXT NOT NULL,
  status TEXT NOT NULL,
  required_inputs_json TEXT NOT NULL,
  required_env_json TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS skill_runs (
  id TEXT PRIMARY KEY,
  skill_id TEXT NOT NULL,
  skill_name TEXT NOT NULL,
  status TEXT NOT NULL,
  inputs_json TEXT NOT NULL,
  context_json TEXT NOT NULL,
  started_at TEXT NOT NULL,
  finished_at TEXT,
  error TEXT,
  logs_json TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS skill_run_outputs (
  id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL,
  kind TEXT NOT NULL,
  title TEXT NOT NULL,
  content TEXT NOT NULL,
  file_path TEXT,
  position INTEGER NOT NULL,
  FOREIGN KEY(run_id) REFERENCES skill_runs(id) ON DELETE CASCADE
);
```

## UI/UX

左侧技能市场：

- 从真实技能 registry 渲染，不再使用硬编码技能数组。
- 支持按关键词搜索。
- 支持按领域筛选。
- 显示技能状态：`可用`、`需要配置`、`缺少依赖`、`不可用`。

中间工作台：

- 展示技能详情、输入表单和运行按钮。
- 运行中显示实时状态和日志。
- 运行完成后显示结构化输出。
- 失败时显示中文错误和可执行恢复步骤。

命令面板：

- 搜索技能名称。
- 支持直接打开技能详情。
- 支持对当前文献运行技能。

右侧 PDF：

- 如果技能运行依赖当前文献，右侧 PDF 保持当前上下文。
- 如果输出包含页码或引用，点击输出应跳转 PDF 页码。
- 如果技能不依赖 PDF，右侧 PDF 不被强制切换。

## 安全与边界

执行限制：

- 只允许运行已导入、已登记的技能。
- 不接受用户在 UI 中输入任意 shell 命令。
- 技能运行目录必须限制在 Novum 应用数据目录或受控工作目录内。
- 写文件必须记录产物路径。
- 日志必须脱敏 API Key、Authorization header 和 provider token。

依赖限制：

- 缺少 Python 包时返回可读错误，不自动联网安装。
- 本阶段不做后台自动升级上游技能。
- 上游源码更新必须走显式同步流程。

## 验收标准

功能验收：

- 应用启动后技能市场显示真实 `science-skills` 技能列表。
- 用户可以搜索技能。
- 用户可以打开技能详情。
- 用户可以从 UI 运行至少一个不依赖外部 API 的技能或 dry-run 技能。
- 运行记录、日志和输出可以在应用内查看。
- 缺少依赖、缺少 API Key、技能失败时显示中文错误。
- 原始 `scripts/` 和 `references/` 不出现在普通用户 UI 中。

技术验收：

```sh
npm run desktop:lint
npm run desktop:build
cd apps/desktop/src-tauri
cargo fmt --check
cargo check
cd ../../../services/research
python -m compileall novum_research tests
python -m pytest tests
```

打包验收：

```sh
cd apps/desktop
npm run tauri:build
```

如果本地 Python 环境未安装测试依赖，必须在提交说明或验收记录中明确标记。

## 开发顺序

1. 引入 `vendor/science-skills` 快照和 license 记录。
2. 在 `services/research` 实现技能扫描与 `SKILL.md` 解析。
3. 新增技能 API 与 Python 单元测试。
4. 在 Rust 层新增技能相关 Tauri commands。
5. 扩展 SQLite schema 保存技能缓存和运行记录。
6. 替换前端硬编码技能数组。
7. 实现技能详情、输入表单、运行状态和输出面板。
8. 接入命令面板技能搜索。
9. 完成 lint/build/cargo/pytest/tauri build 验证。

## 风险

- `SKILL.md` 结构不完全统一，自动推断输入可能不稳定。首版应允许通用任务上下文输入。
- 部分技能依赖外部 API 或特定 Python 包，必须先显示配置缺口。
- 直接运行上游脚本有安全风险，必须通过白名单 registry 和受控工作目录执行。
- 技能输出格式可能不一致，首版优先支持 markdown/text/json/file 四类结果。
