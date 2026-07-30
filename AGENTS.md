# MirrorIt 项目规则

## 项目文档

开始任何 MirrorIt 工作前，先读取权威文档索引：

`C:\Users\GMDI\OneDrive\文档\note_everything\all_note\HomeProjects\06projects\镜像管理软件\docs\index.md`

在不同设备上，不依赖该绝对路径。请优先读取环境变量 `MIRRORIT_DOCS_DIR`；未设置时，从 OneDrive 根目录按以下相对路径定位：

`note_everything/all_note/HomeProjects/06projects/镜像管理软件/docs`

当用户询问“下一步做什么”“工作计划是什么”“项目状态如何”或任何需要项目上下文的问题时，默认读取 `docs/index.md` 以及索引中相关文档后再回答，无需要求用户重复路径。

所有产品、设计、技术、测试、发布和决策文档只存放于该 `docs` 目录。新增、移动或废弃文档时，必须同步更新 `docs/index.md`。

## 当前实施基线

- 桌面端：Tauri。
- 前端：React、TypeScript、Vite、shadcn/ui、Tailwind CSS。
- 主题：浅色、深色、跟随系统均为首发能力。
- 设计方向：Apple 式的克制、精密与清晰层级；不得复制 Apple 品牌资产。
- 优先工具：npm、Maven、Flutter/Pub。
