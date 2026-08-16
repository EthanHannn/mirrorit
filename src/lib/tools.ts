import type { ConfigScope } from "@/lib/types";

export type ToolId =
  | "npm"
  | "maven"
  | "flutter-pub"
  | "go"
  | "cargo"
  | "docker"
  | "pnpm"
  | "yarn"
  | "pip";

export type WritableToolId = "npm" | "maven" | "flutter-pub";

export interface ToolMeta {
  id: ToolId;
  label: string;
  mode: "可管理" | "只读";
  glyph: string;
  title: string;
  description: string;
  scanLabel: string;
  emptyMessage: string;
  footnote?: string;
  scanCommand: string;
  acceptsProjectDirectory: boolean;
}

export const toolNavigation: ToolMeta[] = [
  {
    id: "npm",
    label: "npm",
    mode: "可管理",
    glyph: "N",
    title: "npm 配置来源",
    description:
      "查看 registry、scope registry 与代理的最终生效值；扫描不会改动本机文件。",
    scanLabel: "扫描配置",
    emptyMessage: "未发现受支持的 npm 配置项。",
    scanCommand: "scan_npm",
    acceptsProjectDirectory: true,
  },
  {
    id: "maven",
    label: "Maven",
    mode: "可管理",
    glyph: "M",
    title: "Maven settings.xml",
    description:
      "读取全局和用户级 mirrors、活动代理与 profile 仓库；仅可预览并更新用户级已有镜像的 URL。",
    scanLabel: "扫描 Maven",
    emptyMessage: "未发现受支持的 Maven 配置项。",
    scanCommand: "scan_maven",
    acceptsProjectDirectory: false,
  },
  {
    id: "flutter-pub",
    label: "Flutter/Pub",
    mode: "可管理",
    glyph: "F",
    title: "Flutter/Pub",
    description:
      "查看默认 hosted 源、代理与项目 pubspec.yaml 中显式的 hosted 依赖。",
    scanLabel: "扫描 Flutter/Pub",
    emptyMessage: "未发现受支持的 Flutter/Pub 配置项。",
    scanCommand: "scan_flutter_pub",
    acceptsProjectDirectory: true,
  },
  {
    id: "go",
    label: "Go",
    mode: "只读",
    glyph: "Go",
    title: "Go 模块环境",
    description: "查看模块代理、校验数据库与私有模块规则的有效值和来源轨迹。",
    scanLabel: "扫描 Go",
    emptyMessage: "未发现受支持的 Go 环境配置项。",
    footnote: "此阶段不会修改 GOENV、环境变量、模块缓存或项目文件。",
    scanCommand: "scan_go",
    acceptsProjectDirectory: false,
  },
  {
    id: "cargo",
    label: "Cargo",
    mode: "只读",
    glyph: "C",
    title: "Cargo registry",
    description:
      "查看 crates.io 替换、命名 registry、默认 registry 与 HTTP 代理的来源轨迹。",
    scanLabel: "扫描 Cargo",
    emptyMessage: "未发现受支持的 Cargo 配置项。",
    footnote:
      "只读取用户级与明确选择项目的配置；不会读取 token、凭据文件、缓存或 Cargo.lock。",
    scanCommand: "scan_cargo",
    acceptsProjectDirectory: true,
  },
  {
    id: "docker",
    label: "Docker",
    mode: "只读",
    glyph: "D",
    title: "Docker 镜像与代理",
    description:
      "查看 CLI 代理与 Linux、Windows 守护进程的 registry mirror 来源。",
    scanLabel: "扫描 Docker",
    emptyMessage: "未发现受支持的 Docker 配置项。",
    footnote:
      "不会读取认证信息、Docker Desktop 设置、镜像、容器或构建缓存；不会连接或启动 Docker 守护进程。",
    scanCommand: "scan_docker",
    acceptsProjectDirectory: false,
  },
  {
    id: "pnpm",
    label: "pnpm",
    mode: "只读",
    glyph: "P",
    title: "pnpm registry 与代理",
    description:
      "查看全局、用户、明确选择项目与环境变量中的 registry、scope registry 和代理来源。",
    scanLabel: "扫描 pnpm",
    emptyMessage: "未发现受支持的 pnpm 配置项。",
    footnote:
      "只读取允许的 .npmrc 字段与环境变量；不会读取 token、认证项、lockfile、store、缓存或项目工作区文件。",
    scanCommand: "scan_pnpm",
    acceptsProjectDirectory: true,
  },
  {
    id: "yarn",
    label: "Yarn",
    mode: "只读",
    glyph: "Y",
    title: "Yarn registry 与代理",
    description:
      "按检测到的 Yarn Classic 或 Berry 版本查看用户、明确选择项目与环境变量来源。",
    scanLabel: "扫描 Yarn",
    emptyMessage: "未发现受支持的 Yarn 配置项。",
    footnote:
      "只读取由已检测版本确定的配置格式和允许的环境变量；不会读取认证项、缓存、lockfile、工作区文件或依赖内容。",
    scanCommand: "scan_yarn",
    acceptsProjectDirectory: false,
  },
  {
    id: "pip",
    label: "pip",
    mode: "只读",
    glyph: "Py",
    title: "pip index 与代理",
    description:
      "查看 Windows 全局、用户、当前 Python 虚拟环境与环境变量中的 index 和代理来源。",
    scanLabel: "扫描 pip",
    emptyMessage: "未发现受支持的 pip 配置项。",
    footnote:
      "只读取约定的 pip.ini 路径与允许的 PIP_* 环境变量；不会读取认证项、缓存、requirements、已安装包或 PIP_CONFIG_FILE 指向的任意文件。",
    scanCommand: "scan_pip",
    acceptsProjectDirectory: false,
  },
];

export function getToolMeta(id: ToolId): ToolMeta {
  return toolNavigation.find((tool) => tool.id === id)!;
}

export const scopeLabels: Record<ConfigScope, string> = {
  system: "全局",
  user: "用户级",
  project: "项目级",
  virtual_environment: "虚拟环境",
  environment: "环境变量",
};

export const npmProfileDefinitions: Record<
  Exclude<import("@/lib/types").ProfileKind, "custom">,
  { id: string; name: string; registry: string; description: string }
> = {
  official: {
    id: "npm-official",
    name: "官方源",
    registry: "https://registry.npmjs.org/",
    description: "npm 官方 registry",
  },
  mirror: {
    id: "npmmirror",
    name: "公共镜像",
    registry: "https://registry.npmmirror.com/",
    description: "候选公共镜像，应用前建议检查连通性",
  },
};
