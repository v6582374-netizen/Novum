# Novum 桌面端

这是 Novum 的桌面端应用，使用 Tauri + React + TypeScript + Vite 构建。

当前界面实现了 `spec.md` 中定义的第一版产品骨架，并已接入本地文献库与真实 PDF 预览：左侧管理 PDF 文献，中间展示论文状态与后续研究入口，右侧持久渲染 PDF。

## 常用命令

```sh
npm run dev
npm run build
npm run lint
npm run tauri:dev
npm run tauri:build
```

## 当前重点

- 完善 PDF 搜索、高亮与引用跳转
- 接入 PaperQA 适配器
- 建立本地文献索引
- 接入 science-skills 技能市场
