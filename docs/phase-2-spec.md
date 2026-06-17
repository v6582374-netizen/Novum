# Phase 2 开发规格：文献库与真实 PDF 工作台

## 目标

Phase 2 的目标是把 Novum 从“可点击的界面壳”推进为第一条真实科研工作流闭环：

1. 用户从桌面选择 PDF。
2. Novum 将 PDF 复制到本地应用数据目录。
3. 文献信息写入本地 SQLite 文献库。
4. 左侧文献库展示真实导入记录。
5. 右侧 PDF 预览显示真实 PDF 内容。
6. 页码、缩放和阅读状态可保存并恢复。

本阶段不接入真实 PaperQA 和 science-skills 执行器，但必须为二者预留稳定 adapter 边界，避免后续重写数据流。

## 范围

本阶段必须实现：

- 本地文献库 CRUD 的最小闭环。
- PDF 导入、复制、去重、删除。
- 真实 PDF 渲染。
- 当前页、缩放比例保存。
- 中文空状态、错误状态和进度反馈。
- PaperQA、技能市场、模型配置继续保留中文占位，不展示假结果。

本阶段不实现：

- 真实 PaperQA 调用。
- 真实 science-skills 执行。
- 云同步。
- 账户系统。
- Windows 打包。
- Homebrew 发布。

## 数据与接口

本地数据由 Tauri/Rust 主导，前端通过 Tauri command 调用。

核心类型：

```ts
type DocumentRecord = {
  id: string
  title: string
  fileName: string
  storedPath: string
  fingerprint: string
  pageCount: number
  status: '已导入' | '已索引' | '导入失败'
  createdAt: string
  updatedAt: string
  lastOpenedPage: number
  lastZoom: number
}

type PdfBytes = {
  documentId: string
  fileName: string
  bytes: number[]
}
```

Tauri commands：

- `import_pdf_from_path(path: string): DocumentRecord`
- `list_documents(): DocumentRecord[]`
- `get_document(id: string): DocumentRecord`
- `get_document_pdf_bytes(id: string): PdfBytes`
- `update_reading_state(id: string, page: number, zoom: number): DocumentRecord`
- `delete_document(id: string): boolean`

预留 adapter 边界：

- `indexDocument(documentId)`
- `askDocument(documentId, question)`
- `listSkills()`
- `runSkill(skillId, input)`

Phase 2 只保留占位，不做真实研究执行。

## 用户体验

- 空文献库时显示导入入口，不再展示伪造论文列表。
- 导入成功后自动选中新文献并打开 PDF。
- 导入重复 PDF 时复用已有记录，并提示用户已存在。
- 删除当前文献后自动选择下一篇或回到空状态。
- PDF 预览支持上一页、下一页、页码跳转、放大、缩小。
- PaperQA 区域明确提示“论文问答引擎尚未接入”，不得生成模拟答案冒充真实结果。

## 验收标准

- `npm run build` 通过。
- `npm run lint` 通过。
- `cargo check` 通过。
- `npm run tauri:build` 通过。
- 手动运行 `npm run desktop:tauri` 后：
  - 可以导入本地 PDF。
  - 左侧出现真实文献记录。
  - 右侧能渲染真实 PDF 页面。
  - 页码/缩放修改后切换文献再回来仍保留。
  - 删除文献后本地记录和文件被移除。
  - 非 PDF 或损坏 PDF 有中文错误提示。

