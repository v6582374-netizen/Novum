# Novum

Novum 是一个面向长期科研工作的桌面端科研 IDE。它不是普通聊天机器人，也不是轻量 PDF 阅读器，而是一个本地优先、工具优先、适合高频人机协作的科研工作台。

首版目标是完成“长期科研模式”：用户可以在一个类似 IDE 的界面中管理论文、预览 PDF、围绕文献向论文问答引擎提问、调用科学技能，并把研究过程沉淀为可追踪的本地工作流。macOS 版本优先开发，Windows 版本在核心架构稳定后跟进。

## 当前状态

项目目前处于早期开发阶段，已经完成：

- 项目规格文档：[spec.md](./spec.md)
- Phase 2 开发规格：[docs/phase-2-spec.md](./docs/phase-2-spec.md)
- Phase 3 开发规格：[docs/phase-3-spec.md](./docs/phase-3-spec.md)
- 下一阶段开发规格：[docs/phase-4-spec.md](./docs/phase-4-spec.md)
- Tauri + React + TypeScript 桌面端工程骨架
- 三栏科研 IDE 初始界面
- 左侧真实本地文献库
- 中间论文问答/智能体工作台
- 右侧真实 PDF 预览
- PDF 导入、复制、删除与阅读状态保存
- OpenAI-compatible 模型服务配置
- DeepSeek 官方 API preset
- 本地测试中转站 preset 与本机密钥导入
- PaperQA 本地研究服务基础链路
- 当前文献索引、问答、引用回链与运行记录基础能力
- `google-deepmind/science-skills` 上游快照与 license/NOTICE 记录
- 科学技能注册表解析、搜索、筛选、详情页和 dry-run 运行记录
- macOS `.app` 和 `.dmg` 本地打包验证

接下来会从 Science Skills dry-run 继续推进到受控脚本执行器、参数 schema、真实产物落盘和更严格的依赖检查。

## 产品方向

Novum 的核心设计哲学：

- **桌面优先**：初版只考虑桌面端，macOS 先做，Windows 后续支持。
- **本地优先**：PDF、索引、笔记、对话和工具运行记录默认保存在本机。
- **工具优先**：将优秀开源科研项目嵌入为 Novum 的内置能力，而不是只包一层通用 chat UI。
- **智能体辅助，人类主导**：智能体用于检索、总结、比较、调用工具和生成候选路径，用户始终能查看来源与中间状态。
- **科研 IDE 气质**：界面应保持高密度、可键盘操作、可追踪、可长期使用，避免营销页式和消费级轻应用风格。

## 技术栈

桌面端位于 `apps/desktop`：

- Tauri 2
- React 19
- TypeScript
- Vite
- Rust
- lucide-react

计划中的科研能力集成：

- [Future-House/paper-qa](https://github.com/Future-House/paper-qa)：文献问答、引用与科学文档 RAG
- [GPT Researcher](https://github.com/assafelovic/gpt-researcher)：深度研究任务 UI 与研究流程参考
- [google-deepmind/science-skills](https://github.com/google-deepmind/science-skills)：科学技能市场
- [Warp](https://github.com/warpdotdev/warp)：开发者工具界面风格参考；仅复用许可允许的部分，其余重新实现

## 环境要求

推荐环境：

- macOS
- Node.js 20 或更新版本
- npm
- Rust toolchain
- Xcode Command Line Tools

检查命令：

```sh
node --version
npm --version
cargo --version
rustc --version
```

如果缺少 Rust：

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

如果缺少 Xcode Command Line Tools：

```sh
xcode-select --install
```

## 安装依赖

从仓库根目录执行：

```sh
npm --prefix apps/desktop install
```

Phase 3 PaperQA 本地研究服务需要单独准备 Python 环境：

```sh
cd services/research
python3 -m venv .venv
source .venv/bin/activate
python -m pip install -e ".[test]"
```

## 本地运行

除特别说明外，以下命令均从仓库根目录执行。

## 本地密钥

API Key 不应写入仓库。Novum 会把用户在 UI 中输入的密钥保存到 Tauri Stronghold；开发测试 preset 也可以从本机环境或被 Git 忽略的 `secrets/` 目录读取：

```text
secrets/openai-compatible-api-key.txt
secrets/deepseek-api-key.txt
secrets/test-relay-api-key.txt
```

可选环境变量：

```text
OPENAI_API_KEY
DEEPSEEK_API_KEY
NOVUM_TEST_RELAY_API_KEY
NOVUM_TEST_PROVIDER_API_KEY
```

运行 Web 预览：

```sh
npm run desktop:dev
```

打开地址：

```text
http://127.0.0.1:5173/
```

运行原生桌面壳：

```sh
npm run desktop:tauri
```

也可以进入桌面端目录执行：

```sh
cd apps/desktop
npm run tauri:dev
```

## 构建与检查

前端构建：

```sh
npm run desktop:build
```

代码检查：

```sh
npm run desktop:lint
```

Rust/Tauri 检查：

```sh
cd apps/desktop/src-tauri
cargo check
```

构建 macOS 桌面应用：

```sh
cd apps/desktop
npm run tauri:build
```

构建成功后，本地产物位于：

```text
apps/desktop/src-tauri/target/release/bundle/macos/Novum.app
apps/desktop/src-tauri/target/release/bundle/dmg/Novum_0.1.0_aarch64.dmg
```

当前产物还未完成签名和 notarization，正式发布前需要补齐 macOS 分发链路。

## 使用指南

当前版本仍是开发预览版，但已经具备第一条真实本地工作流。

启动后可以看到三栏界面：

- 左侧：本地文献库、论文问答、技能市场、模型配置入口
- 中间：长期科研模式工作台、论文状态、智能体输出占位、下一步任务卡片
- 右侧：真实 PDF 预览区域，支持页码跳转与缩放

当前可以导入本地 PDF、查看真实 PDF、保存阅读页码与缩放、删除文献，配置 OpenAI-compatible、DeepSeek 官方或本地测试中转站模型服务，并围绕当前文献发起 PaperQA 索引与问答。科学技能市场已经读取真实 `science-skills` 注册表，支持搜索、筛选、查看详情和 dry-run 运行记录。

## 目录结构

```text
.
├── apps/
│   └── desktop/              # Tauri + React 桌面端
│       ├── src/              # 前端界面
│       └── src-tauri/        # Tauri/Rust 原生层
├── services/
│   └── research/             # PaperQA、本地研究服务与后续技能运行器
├── docs/                     # 分阶段开发规格
├── vendor/
│   └── science-skills/       # google-deepmind/science-skills 上游快照
├── licenses/                 # 上游许可证与 NOTICE
├── patches/                  # 上游项目本地 patch 记录
├── spec.md                   # 产品与架构规格
├── package.json              # 根目录开发脚本
└── README.md                 # 项目说明
```

## 开源项目嵌入策略

Novum 会将关键科研项目以内嵌源码快照的方式纳入项目，而不是简单依赖外部服务。每个上游项目都必须记录：

- 上游 URL
- commit SHA 或 release tag
- 导入日期
- license/NOTICE
- 本地 patch 说明
- 后续同步升级步骤

前端不应直接依赖上游项目内部实现，而应通过 Novum 自己的 adapter 层调用，便于后续升级和替换。

## 后续开发计划

近期路线：

1. **本地文献库**
   - PDF 导入
   - 文献元数据存储
   - 阅读位置保存
   - 本地应用数据目录规划

2. **PDF 预览真实链路**
   - 接入真实 PDF renderer
   - 页码跳转
   - 缩放、搜索、高亮
   - citation 点击后定位到对应页面

3. **PaperQA 适配器**
   - 已建立 `services/research` 本地服务
   - 已封装 PaperQA 调用边界
   - 已接入 active paper 索引与问答入口
   - 已接入引用回链到 PDF 预览

4. **模型服务设置**
   - 已支持用户自带 API Key
   - 已使用 Stronghold 做本地密钥存储
   - 已支持模型服务连通性验证
   - 已支持默认模型选择
   - 已支持 DeepSeek 官方 preset
   - 已支持测试中转站 preset

5. **科学技能市场**
   - 已导入 `science-skills` 上游快照
   - 已解析 `SKILL.md`
   - 已隐藏 `scripts/` 和 `references/` 原始结构
   - 已支持 UI 点击选择技能
   - 已支持命令面板打开技能
   - 已支持 dry-run 运行日志与结果展示

6. **发布与升级**
   - macOS 签名与 notarization
   - Homebrew formula
   - `brew upgrade novum`
   - Windows winget/Scoop 方案调研

中长期方向：

- 多论文集合问答
- GPT Researcher 深度研究报告流
- 短期竞赛模式
- 本地模型接入
- 团队协作与云同步
- 数据集与代码执行工作区
- 实验追踪与可复现实验记录

## 贡献与开发约定

当前项目仍处于创始开发阶段，优先保证架构边界清晰：

- UI 先服务长期科研工作流，不做营销页。
- 本地数据和凭据默认不出本机。
- 新增上游项目时必须同步许可证和版本来源。
- 不直接复制受 AGPL 限制的 Warp 源码；只复用许可允许部分或重新实现视觉/交互语言。
- 任何科研回答能力都必须保留来源、引用和中间状态。

更多产品与架构细节见 [spec.md](./spec.md)。
