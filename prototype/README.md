# IdeaCard 交互原型

`prototype/` 是 IdeaCard 最初的浏览器交互原型，用于验证统一收集箱、快速捕获、卡片整理和多卡片 AI 分析流程。它使用 React、TypeScript、Vite 与 Zustand，数据和 AI 结果均为演示用途。

真实 macOS 应用在 `../desktop/`；原型不连接正式应用的 SQLite 数据库或 macOS 原生能力。

## 运行

在本目录执行，使用 Node.js 22.12+ 的 22.x 或 Node.js 24.x，以及 npm 10+：

```bash
npm ci
npm run dev
```

打开启动输出中的本地地址，默认是 `http://localhost:5173`。

## 验证

```bash
npm run lint
npm test
npm run build
```

当前保留了 Playwright 端到端测试配置和 `test:e2e` 脚本，但尚无测试用例，运行该脚本会提示 `No tests found`。后续补充用例时使用 Playwright Chromium，测试服务端口为 `4173`。

## 开发约定

遵循根目录的 [项目规则](../AGENTS.md)、[产品上下文](../PRODUCT.md) 与 [设计规范](../DESIGN.md)。原型与桌面端分别验证；原型中存在的探索功能不代表桌面端已实现。
