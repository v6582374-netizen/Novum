# Novum 桌面端

这是 Novum 的桌面端应用，使用 Tauri + React + TypeScript + Vite 构建。

当前界面实现了 `spec.md` 中定义的第一版产品骨架：左侧研究库和技能入口，中间论文问答/智能体工作台，右侧持久 PDF 预览。

## 常用命令

```sh
npm run dev
npm run build
npm run lint
npm run tauri:dev
npm run tauri:build
```

## 当前重点

- 完善真实 PDF 导入和预览链路
- 接入 PaperQA 适配器
- 建立本地文献库与索引
- 接入 science-skills 技能市场
