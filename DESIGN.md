---
name: MirrorIt
description: 本地优先、来源可解释且变更可恢复的桌面配置工作台
colors:
  signal-blue-light: "#087ff5"
  signal-blue-dark: "#0a84ff"
  cold-fog: "#f2f3f5"
  paper-white: "#ffffff"
  graphite: "#1c1d1f"
  graphite-raised: "#27292c"
  ink: "#202124"
  ink-inverse: "#f3f4f6"
  muted-light: "#636871"
  muted-dark: "#aeb3bb"
  hairline-light: "rgb(32 33 36 / 10%)"
  hairline-dark: "rgb(255 255 255 / 11%)"
  success-light: "#25a95b"
  success-dark: "#30d158"
  warning-light: "#b25000"
  warning-dark: "#ff9f0a"
typography:
  headline:
    fontFamily: "-apple-system, BlinkMacSystemFont, Segoe UI, PingFang SC, Microsoft YaHei, sans-serif"
    fontSize: "20px"
    fontWeight: 600
    lineHeight: 1.5
    letterSpacing: "0"
  title:
    fontFamily: "-apple-system, BlinkMacSystemFont, Segoe UI, PingFang SC, Microsoft YaHei, sans-serif"
    fontSize: "16px"
    fontWeight: 600
    lineHeight: 1.5
    letterSpacing: "0"
  body:
    fontFamily: "-apple-system, BlinkMacSystemFont, Segoe UI, PingFang SC, Microsoft YaHei, sans-serif"
    fontSize: "14px"
    fontWeight: 400
    lineHeight: 1.5
    letterSpacing: "0"
  label:
    fontFamily: "-apple-system, BlinkMacSystemFont, Segoe UI, PingFang SC, Microsoft YaHei, sans-serif"
    fontSize: "12px"
    fontWeight: 600
    lineHeight: 1.5
    letterSpacing: "0"
  code:
    fontFamily: "JetBrains Mono, Consolas, monospace"
    fontSize: "12px"
    fontWeight: 400
    lineHeight: 1.5
    letterSpacing: "0"
rounded:
  compact: "4px"
  control: "6px"
spacing:
  xs: "4px"
  sm: "8px"
  md: "12px"
  lg: "16px"
  xl: "24px"
  xxl: "32px"
components:
  button-primary:
    backgroundColor: "{colors.signal-blue-light}"
    textColor: "{colors.paper-white}"
    rounded: "{rounded.control}"
    padding: "0 10px"
    height: "32px"
  button-outline:
    backgroundColor: "{colors.paper-white}"
    textColor: "{colors.ink}"
    rounded: "{rounded.control}"
    padding: "0 10px"
    height: "32px"
  input:
    backgroundColor: "{colors.paper-white}"
    textColor: "{colors.ink}"
    rounded: "{rounded.control}"
    padding: "0 12px"
    height: "36px"
---

# Design System: MirrorIt

## Overview

**Creative North Star: "The Signal Ledger"**

MirrorIt 像一张安静、可信的本机配置账本：界面先解释值从哪里来，再允许用户采取动作。冷雾浅色面与石墨深色面保持低噪声，系统蓝只标记主操作、当前选择和来源路径；半透明只用于标题栏、导航和检查器等结构层，不覆盖需要精确阅读的配置内容。

布局服务于长期桌面操作而不是展示。信息密度紧凑但不拥挤，来源优先级、覆盖关系、预览、快照和恢复始终形成连续的安全叙事。Apple 式原则体现在克制材质、清晰层级和自然反馈中，同时保留 Windows 用户熟悉的控件、键盘顺序和操作效率。

**Key Characteristics:**

- 冷雾与石墨双主题，单一系统蓝承担交互信号。
- 三栏桌面骨架，中央来源账本是视觉与任务中心。
- 细分隔线、紧凑圆角、极少阴影，内容面保持哑光清晰。
- 配置值使用等宽字体，长路径和 URL 截断但不破坏布局。
- 所有写入动作都与预览、快照和恢复状态相邻。

## Colors

色彩以冷中性色承载高密度信息，蓝色像信号轨一样稀疏出现；绿色、琥珀色和红色只表达可验证状态。

### Primary

- **Signal Blue:** 主操作、选中状态、焦点和来源轨迹；浅深主题分别使用对应令牌。

### Neutral

- **Cold Fog / Paper White:** 浅色主题的应用背景与中央内容面。
- **Graphite / Graphite Raised:** 深色主题的应用背景与抬高控件面。
- **Ink / Inverse Ink:** 浅色和深色主题的主要文本。
- **Muted Ink:** 辅助说明、路径元信息和未激活导航。
- **Hairline:** 栏目边界、内容分隔和轨迹结构线。

### Named Rules

**The Signal Rarity Rule.** 系统蓝只用于当前选择、主要命令、焦点和来源路径，不把整块内容面染成蓝色。

**The Semantic State Rule.** 成功、警告和错误必须同时包含图标或文字，不能只依赖颜色。

## Typography

**Display Font:** 系统无衬线字体栈
**Body Font:** 系统无衬线字体栈
**Label/Mono Font:** JetBrains Mono，回退到 Consolas 和 monospace

**Character:** 字体选择优先本机渲染质量和 Windows 中文可读性。层级依靠字号、字重、留白和分隔，而不是大幅字号跳跃或负字距。

### Hierarchy

- **Headline** (600, 20px, 1.5): 当前工具工作区标题。
- **Title** (600, 16px, 1.5): 工作区区段标题。
- **Body** (400, 14px, 1.5): 描述、表单和主要状态文字。
- **Label** (600, 12px, 1.5): 紧凑导航、检查器元信息和字段标签。
- **Code** (400, 12px, 1.5): URL、文件路径、配置值和快照标识。

### Named Rules

**The Desktop Scale Rule.** 工作区不使用宣传页式超大标题；标题在 20px 封顶，结构由空间和位置建立。

**The Zero Tracking Rule.** 中文界面、按钮、导航和代码值保持零字距，不使用负字距压缩内容。

## Layout

默认桌面壳由 52px 标题栏和三栏工作区组成：220px 工具导航、可伸缩中央工作区、288px 上下文检查器。中央内容最大宽度 1120px，水平内边距 32px，垂直内边距从 28px 起；各任务区段以 20-28px 的垂直节奏和 1px 分隔线组织。

窗口低于 1100px 时，右侧检查器移到底部并限制在 120px 高，配置档选项保持两列；低于 760px 时导航收窄为图标列，标题栏隐藏次要文字。固定工具栏、导航项和状态区域必须有稳定尺寸，动态内容不得推动整体骨架跳动。

来源账本在 DOM 与视觉顺序上都紧跟工具标题，之后依次是扫描范围、显式检查、配置档预览、变更计划和恢复点。键盘焦点顺序必须与屏幕顺序一致。

## Elevation & Depth

系统总体平面化。中央内容使用哑光纯色，深度主要由相邻面色差和细分隔线建立；22-24px 模糊与适度饱和仅属于标题栏、侧栏和检查器。阴影只用于应用标记和主要按钮的微弱接触感，不用于堆叠卡片或浮动页面区段。

### Shadow Vocabulary

- **Contact:** `0 1px 2px rgb(0 0 0 / 12%)`，只用于主要按钮。
- **Mark:** `0 1px 3px rgb(0 0 0 / 18%)`，只用于应用标记。

### Named Rules

**The Matte Content Rule.** 配置、差异和来源轨迹所在的中央内容面不使用模糊、玻璃或装饰阴影。

## Shapes

控件和导航使用紧凑的 6px 圆角，分段控件的内部选项使用 4px 圆角。状态节点使用圆形，来源关系使用 1px 直线。内容区段不包裹成浮动卡片，也不嵌套卡片；边框服务于分组和焦点，而不是装饰。

## Components

### Buttons

- **Shape:** 紧凑 6px 圆角，默认高 32px，图标与文字间距 6px。
- **Primary:** Signal Blue 背景、白色文字和轻微接触阴影；每个工具只有一个明确主动作。
- **Hover / Focus:** 150ms 颜色过渡，焦点使用蓝色边框与半透明双像素环，按下缩放到 98%。
- **Outline / Ghost:** 次要命令使用细边框或透明背景，悬停只提升中性色对比。

### Chips

- **Style:** 仅用于“仅检测”“最终生效”等短状态，6px 或胶囊形轮廓，字号不低于 11px。
- **State:** 选中态使用低饱和蓝底和蓝字；普通态使用中性色边框。

### Cards / Containers

- **Corner Style:** 页面区段不做卡片；只有重复选择项和真正受框定的控件使用 0-6px 圆角。
- **Background:** 中央内容与外壳通过主题表面令牌区分。
- **Shadow Strategy:** 静态容器无阴影。
- **Border:** 1px hairline 分隔。
- **Internal Padding:** 12-16px 用于重复选择项，24-28px 用于区段节奏。

### Inputs / Fields

- **Style:** 36px 高、6px 圆角、纯色内容面和 1px 输入边框。
- **Focus:** 边框切换为 Signal Blue，并显示半透明 3px 焦点环。
- **Error / Disabled:** 错误使用语义红色边框并保留文字说明；禁用态降低不透明度但保持可辨识标签。

### Navigation

左侧工具项高 44px，包含稳定的 32px 工具字形区、名称、能力标签和读取状态。悬停只改变中性色面，激活项使用低饱和蓝面；按下反馈为轻微缩放。窄窗口只收起文字，不改变工具顺序。

### Source Ledger

来源账本是签名组件。每个配置项先显示最终值，再沿 1px 蓝色纵向轨道列出来源、作用域、位置和优先级；空心节点表示被覆盖来源，实心蓝节点与“最终生效”文字共同标记胜出来源。账本不可表现为可拖拽拓扑。

### Inspector

检查器只汇总当前工具、来源数量、变更计划、恢复点和操作状态。错误状态按问题、影响、恢复建议、折叠技术详情的顺序呈现；窄窗口中检查器移动到底部并隐藏重复标题。

## Do's and Don'ts

### Do:

- **Do** 先呈现来源与最终生效值，再呈现连接检查和写入操作。
- **Do** 让主要动作、状态和恢复路径在浅色与深色主题中都可扫描。
- **Do** 使用 Lucide 图标、明确文字和稳定控件尺寸表达熟悉的桌面命令。
- **Do** 在减少动态、减少透明度和高对比偏好下保留完整层级。

### Don't:

- **Don't** 把页面区段包装为一组浮动卡片或在卡片中嵌套卡片。
- **Don't** 使用大面积单色、渐变光球、装饰插画或宣传页式巨型标题。
- **Don't** 复制 Apple 商标、品牌资产、受限字体或 macOS 窗口控件。
- **Don't** 用颜色代替状态文字，也不要让视觉顺序与 DOM 焦点顺序分离。
- **Don't** 在日志、导出、来源轨迹或截图示例中暴露凭据。
