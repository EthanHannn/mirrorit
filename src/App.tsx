import { invoke } from "@tauri-apps/api/core";
import {
  Activity,
  CheckCircle2,
  Circle,
  Download,
  FileDiff,
  FolderSearch,
  Layers3,
  LoaderCircle,
  LockKeyhole,
  PackageSearch,
  PanelRight,
  RefreshCw,
  RotateCcw,
  Save,
  ScanLine,
  ShieldCheck,
  TriangleAlert,
} from "lucide-react";
import { useState } from "react";
import { ThemeProvider } from "@/components/theme-provider";
import { ThemeToggle } from "@/components/theme-toggle";
import { Button } from "@/components/ui/button";

type ConfigScope =
  "system" | "user" | "project" | "virtual_environment" | "environment";

interface ConfigSource {
  scope: ConfigScope;
  location: string;
  priority: number;
  sensitive: boolean;
  value: string | null;
}

interface EffectiveValue {
  value: string | null;
  sources: ConfigSource[];
}

interface NpmReadResult {
  effective_config: {
    values: Record<string, EffectiveValue>;
  };
  diagnostics: string[];
}
type MavenReadResult = NpmReadResult;
type FlutterPubReadResult = NpmReadResult;
type GoReadResult = NpmReadResult;
type CargoReadResult = NpmReadResult;
type DockerReadResult = NpmReadResult;
type PnpmReadResult = NpmReadResult;
type YarnReadResult = NpmReadResult;
type PipReadResult = NpmReadResult;

interface ChangePlan {
  id: string;
  target_checksums: Record<string, string>;
  file_checksums: Record<string, string>;
  changes: PlannedChange[];
}

interface PlannedChange {
  file: string;
  field: string;
  previous_value: string | null;
  next_value: string | null;
  risk: string | null;
}

interface ApplyResult {
  snapshot: {
    id: string;
  };
}

interface HealthCheckResult {
  status:
    | "healthy"
    | "dns_failure"
    | "tls_failure"
    | "authentication_failure"
    | "timeout"
    | "http_failure";
  elapsed_ms: number;
  message: string | null;
}

interface ProfileExportDocument {
  format: "mirrorit-profile";
  version: number;
  profiles: Array<{
    tool: "npm";
    id: string;
    name: string;
    values: Record<string, string>;
  }>;
}

interface NpmProfileImportPreview {
  id: string;
  name: string;
  current_registry: string;
  imported_registry: string;
  changed: boolean;
}

type ProfileKind = "official" | "mirror" | "custom";
type TargetScope = "user" | "project";
type ToolId =
  | "npm"
  | "maven"
  | "flutter-pub"
  | "go"
  | "cargo"
  | "docker"
  | "pnpm"
  | "yarn"
  | "pip";

const toolNavigation: Array<{
  id: ToolId;
  label: string;
  mode: string;
  glyph: string;
}> = [
  { id: "npm", label: "npm", mode: "可管理", glyph: "N" },
  { id: "maven", label: "Maven", mode: "可管理", glyph: "M" },
  { id: "flutter-pub", label: "Flutter/Pub", mode: "可管理", glyph: "F" },
  { id: "go", label: "Go", mode: "只读", glyph: "Go" },
  { id: "cargo", label: "Cargo", mode: "只读", glyph: "C" },
  { id: "docker", label: "Docker", mode: "只读", glyph: "D" },
  { id: "pnpm", label: "pnpm", mode: "只读", glyph: "P" },
  { id: "yarn", label: "Yarn", mode: "只读", glyph: "Y" },
  { id: "pip", label: "pip", mode: "只读", glyph: "Py" },
];

const scopeLabels: Record<ConfigScope, string> = {
  system: "全局",
  user: "用户级",
  project: "项目级",
  virtual_environment: "虚拟环境",
  environment: "环境变量",
};

const profileDefinitions: Record<
  Exclude<ProfileKind, "custom">,
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

function App() {
  const [activeTool, setActiveTool] = useState<ToolId>("npm");
  const [operationTool, setOperationTool] = useState<ToolId>("npm");
  const [projectDirectory, setProjectDirectory] = useState("");
  const [result, setResult] = useState<NpmReadResult | null>(null);
  const [mavenResult, setMavenResult] = useState<MavenReadResult | null>(null);
  const [flutterPubResult, setFlutterPubResult] =
    useState<FlutterPubReadResult | null>(null);
  const [goResult, setGoResult] = useState<GoReadResult | null>(null);
  const [cargoResult, setCargoResult] = useState<CargoReadResult | null>(null);
  const [dockerResult, setDockerResult] = useState<DockerReadResult | null>(
    null,
  );
  const [pnpmResult, setPnpmResult] = useState<PnpmReadResult | null>(null);
  const [yarnResult, setYarnResult] = useState<YarnReadResult | null>(null);
  const [pipResult, setPipResult] = useState<PipReadResult | null>(null);
  const [flutterPubProfile, setFlutterPubProfile] = useState<
    "official" | "custom"
  >("official");
  const [flutterPubHostedUrl, setFlutterPubHostedUrl] = useState("");
  const [flutterPubPlan, setFlutterPubPlan] = useState<ChangePlan | null>(null);
  const [flutterPubSnapshotId, setFlutterPubSnapshotId] = useState<
    string | null
  >(null);
  const [mavenMirrorId, setMavenMirrorId] = useState("");
  const [mavenMirrorUrl, setMavenMirrorUrl] = useState("");
  const [mavenPlan, setMavenPlan] = useState<ChangePlan | null>(null);
  const [mavenSnapshotId, setMavenSnapshotId] = useState<string | null>(null);
  const [profileKind, setProfileKind] = useState<ProfileKind>("official");
  const [targetScope, setTargetScope] = useState<TargetScope>("user");
  const [customRegistry, setCustomRegistry] = useState("");
  const [plan, setPlan] = useState<ChangePlan | null>(null);
  const [snapshotId, setSnapshotId] = useState<string | null>(null);
  const [healthTarget, setHealthTarget] = useState("");
  const [healthResult, setHealthResult] = useState<HealthCheckResult | null>(
    null,
  );
  const [exportedProfileName, setExportedProfileName] = useState<string | null>(
    null,
  );
  const [importContent, setImportContent] = useState<string | null>(null);
  const [importFileName, setImportFileName] = useState<string | null>(null);
  const [importPreview, setImportPreview] =
    useState<NpmProfileImportPreview | null>(null);
  const [scanState, setScanState] = useState<"idle" | "loading" | "error">(
    "idle",
  );
  const [errorMessage, setErrorMessage] = useState("");

  const configEntries = Object.entries(result?.effective_config.values ?? {});
  const toolResults: Record<ToolId, NpmReadResult | null> = {
    npm: result,
    maven: mavenResult,
    "flutter-pub": flutterPubResult,
    go: goResult,
    cargo: cargoResult,
    docker: dockerResult,
    pnpm: pnpmResult,
    yarn: yarnResult,
    pip: pipResult,
  };
  const activeToolResult = toolResults[activeTool];
  const activeToolMeta = toolNavigation.find((tool) => tool.id === activeTool)!;
  const activeToolEntries = Object.entries(
    activeToolResult?.effective_config.values ?? {},
  );
  const activeToolEntryCount = Object.keys(
    activeToolResult?.effective_config.values ?? {},
  ).length;
  const activeSourceCount = activeToolEntries.reduce(
    (count, [, value]) => count + value.sources.length,
    0,
  );
  const activePlan =
    activeTool === "npm"
      ? plan
      : activeTool === "maven"
        ? mavenPlan
        : activeTool === "flutter-pub"
          ? flutterPubPlan
          : null;
  const activeSnapshotId =
    activeTool === "npm"
      ? snapshotId
      : activeTool === "maven"
        ? mavenSnapshotId
        : activeTool === "flutter-pub"
          ? flutterPubSnapshotId
          : null;
  const mavenMirrorEntries = Object.entries(
    mavenResult?.effective_config.values ?? {},
  ).filter(
    ([key, value]) =>
      key.startsWith("mirror.") &&
      value.sources.some((source) => source.scope === "user"),
  );

  function beginOperation(tool: ToolId) {
    setOperationTool(tool);
    setScanState("loading");
    setErrorMessage("");
  }

  async function scan() {
    beginOperation("npm");

    try {
      const scanResult = await invoke<NpmReadResult>("scan_npm", {
        projectDirectory: projectDirectory.trim() || null,
      });
      setResult(scanResult);
      setPlan(null);
      setScanState("idle");
    } catch (error) {
      setScanState("error");
      setErrorMessage(String(error));
    }
  }

  async function scanMaven() {
    beginOperation("maven");
    try {
      const scanResult = await invoke<MavenReadResult>("scan_maven");
      const mirrorIds = Object.entries(scanResult.effective_config.values)
        .filter(
          ([key, value]) =>
            key.startsWith("mirror.") &&
            value.sources.some((source) => source.scope === "user"),
        )
        .map(([key]) => key.slice("mirror.".length));
      setMavenResult(scanResult);
      setMavenMirrorId((currentMirrorId) =>
        mirrorIds.includes(currentMirrorId)
          ? currentMirrorId
          : (mirrorIds[0] ?? ""),
      );
      setMavenPlan(null);
      setScanState("idle");
    } catch (error) {
      setScanState("error");
      setErrorMessage(String(error));
    }
  }

  async function scanFlutterPub() {
    beginOperation("flutter-pub");
    try {
      setFlutterPubResult(
        await invoke<FlutterPubReadResult>("scan_flutter_pub", {
          projectDirectory: projectDirectory.trim() || null,
        }),
      );
      setScanState("idle");
    } catch (error) {
      setScanState("error");
      setErrorMessage(String(error));
    }
  }

  async function scanGo() {
    beginOperation("go");
    try {
      setGoResult(await invoke<GoReadResult>("scan_go"));
      setScanState("idle");
    } catch (error) {
      setScanState("error");
      setErrorMessage(String(error));
    }
  }

  async function scanCargo() {
    beginOperation("cargo");
    try {
      setCargoResult(
        await invoke<CargoReadResult>("scan_cargo", {
          projectDirectory: projectDirectory.trim() || null,
        }),
      );
      setScanState("idle");
    } catch (error) {
      setScanState("error");
      setErrorMessage(String(error));
    }
  }

  async function scanDocker() {
    beginOperation("docker");
    try {
      setDockerResult(await invoke<DockerReadResult>("scan_docker"));
      setScanState("idle");
    } catch (error) {
      setScanState("error");
      setErrorMessage(String(error));
    }
  }

  async function scanPnpm() {
    beginOperation("pnpm");
    try {
      setPnpmResult(
        await invoke<PnpmReadResult>("scan_pnpm", {
          projectDirectory: projectDirectory.trim() || null,
        }),
      );
      setScanState("idle");
    } catch (error) {
      setScanState("error");
      setErrorMessage(String(error));
    }
  }

  async function scanYarn() {
    beginOperation("yarn");
    try {
      setYarnResult(
        await invoke<YarnReadResult>("scan_yarn", {
          projectDirectory: projectDirectory.trim() || null,
        }),
      );
      setScanState("idle");
    } catch (error) {
      setScanState("error");
      setErrorMessage(String(error));
    }
  }

  async function scanPip() {
    beginOperation("pip");
    try {
      setPipResult(await invoke<PipReadResult>("scan_pip"));
      setScanState("idle");
    } catch (error) {
      setScanState("error");
      setErrorMessage(String(error));
    }
  }

  async function previewFlutterPubHosted() {
    beginOperation("flutter-pub");
    try {
      const [scanResult, previewResult] = await Promise.all([
        invoke<FlutterPubReadResult>("scan_flutter_pub", {
          projectDirectory: projectDirectory.trim() || null,
        }),
        invoke<ChangePlan>("preview_flutter_pub_hosted_update", {
          request: {
            hostedUrl:
              flutterPubProfile === "official"
                ? null
                : flutterPubHostedUrl.trim(),
          },
        }),
      ]);
      setFlutterPubResult(scanResult);
      setFlutterPubPlan(previewResult);
      setScanState("idle");
    } catch (error) {
      setScanState("error");
      setErrorMessage(String(error));
    }
  }

  async function applyFlutterPubPreview() {
    if (
      !flutterPubPlan ||
      !window.confirm(
        "确认应用此预览？MirrorIt 将创建本地快照后更新用户级 PUB_HOSTED_URL。重新启动 Flutter、终端和 MirrorIt 后才会生效。",
      )
    ) {
      return;
    }

    beginOperation("flutter-pub");
    try {
      const applied = await invoke<ApplyResult>("apply_flutter_pub_preview", {
        planId: flutterPubPlan.id,
      });
      setFlutterPubResult(
        await invoke<FlutterPubReadResult>("scan_flutter_pub", {
          projectDirectory: projectDirectory.trim() || null,
        }),
      );
      setFlutterPubSnapshotId(applied.snapshot.id);
      setFlutterPubPlan(null);
      setScanState("idle");
    } catch (error) {
      setScanState("error");
      setErrorMessage(String(error));
    }
  }

  async function rollbackFlutterPubSnapshot() {
    if (
      !flutterPubSnapshotId ||
      !window.confirm(
        "确认从此快照恢复 PUB_HOSTED_URL？重新启动 Flutter、终端和 MirrorIt 后才会生效。",
      )
    ) {
      return;
    }

    beginOperation("flutter-pub");
    try {
      await invoke("rollback_flutter_pub_snapshot", {
        snapshotId: flutterPubSnapshotId,
      });
      setFlutterPubResult(
        await invoke<FlutterPubReadResult>("scan_flutter_pub", {
          projectDirectory: projectDirectory.trim() || null,
        }),
      );
      setFlutterPubSnapshotId(null);
      setScanState("idle");
    } catch (error) {
      setScanState("error");
      setErrorMessage(String(error));
    }
  }

  async function previewMavenMirror() {
    beginOperation("maven");
    try {
      const [scanResult, previewResult] = await Promise.all([
        invoke<MavenReadResult>("scan_maven"),
        invoke<ChangePlan>("preview_maven_mirror_update", {
          request: {
            mirrorId: mavenMirrorId,
            url: mavenMirrorUrl.trim(),
          },
        }),
      ]);
      setMavenResult(scanResult);
      setMavenPlan(previewResult);
      setScanState("idle");
    } catch (error) {
      setScanState("error");
      setErrorMessage(String(error));
    }
  }

  async function applyMavenPreview() {
    if (
      !mavenPlan ||
      !window.confirm(
        "确认应用此预览？MirrorIt 将创建本地快照后更新用户级 Maven settings.xml。",
      )
    ) {
      return;
    }

    beginOperation("maven");
    try {
      const applied = await invoke<ApplyResult>("apply_maven_preview", {
        planId: mavenPlan.id,
      });
      setMavenResult(await invoke<MavenReadResult>("scan_maven"));
      setMavenSnapshotId(applied.snapshot.id);
      setMavenPlan(null);
      setScanState("idle");
    } catch (error) {
      setScanState("error");
      setErrorMessage(String(error));
    }
  }

  async function rollbackMavenSnapshot() {
    if (
      !mavenSnapshotId ||
      !window.confirm(
        "确认从此快照恢复 Maven settings.xml？当前目标文件将被替换。",
      )
    ) {
      return;
    }

    beginOperation("maven");
    try {
      await invoke("rollback_maven_snapshot", { snapshotId: mavenSnapshotId });
      setMavenResult(await invoke<MavenReadResult>("scan_maven"));
      setMavenSnapshotId(null);
      setScanState("idle");
    } catch (error) {
      setScanState("error");
      setErrorMessage(String(error));
    }
  }

  async function previewProfile() {
    const selectedProfile = selectedNpmProfile();

    beginOperation("npm");
    try {
      const [scanResult, previewResult] = await Promise.all([
        invoke<NpmReadResult>("scan_npm", {
          projectDirectory: projectDirectory.trim() || null,
        }),
        invoke<ChangePlan>("preview_npm_profile", {
          request: {
            projectDirectory: projectDirectory.trim() || null,
            targetScope,
            profile: selectedProfile,
          },
        }),
      ]);
      setResult(scanResult);
      setPlan(previewResult);
      setScanState("idle");
    } catch (error) {
      setScanState("error");
      setErrorMessage(String(error));
    }
  }

  function selectedNpmProfile() {
    return profileKind === "custom"
      ? {
          id: "custom-registry",
          name: "自定义源",
          registry: customRegistry.trim(),
        }
      : profileDefinitions[profileKind];
  }

  async function exportNpmProfile() {
    const selectedProfile = selectedNpmProfile();
    beginOperation("npm");
    try {
      const exportDocument = await invoke<ProfileExportDocument>(
        "export_npm_profile",
        {
          request: { profile: selectedProfile },
        },
      );
      const blob = new Blob([`${JSON.stringify(exportDocument, null, 2)}\n`], {
        type: "application/json",
      });
      const url = URL.createObjectURL(blob);
      const link = document.createElement("a");
      link.href = url;
      link.download = `${exportDocument.profiles[0].id}.mirrorit-profile.json`;
      link.click();
      window.setTimeout(() => URL.revokeObjectURL(url), 0);
      setExportedProfileName(exportDocument.profiles[0].name);
      setScanState("idle");
    } catch (error) {
      setScanState("error");
      setErrorMessage(String(error));
    }
  }

  async function selectImportFile(file: File | undefined) {
    setImportPreview(null);
    setErrorMessage("");
    if (!file) {
      setImportContent(null);
      setImportFileName(null);
      return;
    }
    try {
      setImportContent(await file.text());
      setImportFileName(file.name);
    } catch {
      setImportContent(null);
      setImportFileName(null);
      setErrorMessage("无法读取所选配置档文件。");
    }
  }

  async function previewImportedProfile() {
    if (!importContent) {
      return;
    }
    beginOperation("npm");
    try {
      const currentProfile = selectedNpmProfile();
      setImportPreview(
        await invoke<NpmProfileImportPreview>("preview_npm_profile_import", {
          request: {
            content: importContent,
            currentRegistry: currentProfile.registry,
          },
        }),
      );
      setScanState("idle");
    } catch (error) {
      setScanState("error");
      setErrorMessage(String(error));
    }
  }

  async function applyPreview() {
    if (
      !plan ||
      !window.confirm(
        "确认应用此预览？MirrorIt 将创建本地快照后修改目标 .npmrc。",
      )
    ) {
      return;
    }

    beginOperation("npm");
    try {
      const applied = await invoke<ApplyResult>("apply_npm_preview", {
        planId: plan.id,
      });
      const scanResult = await invoke<NpmReadResult>("scan_npm", {
        projectDirectory: projectDirectory.trim() || null,
      });
      setResult(scanResult);
      setSnapshotId(applied.snapshot.id);
      setPlan(null);
      setScanState("idle");
    } catch (error) {
      setScanState("error");
      setErrorMessage(String(error));
    }
  }

  async function rollbackSnapshot() {
    if (
      !snapshotId ||
      !window.confirm("确认从此快照恢复 npm 配置？当前目标文件将被替换。")
    ) {
      return;
    }

    beginOperation("npm");
    try {
      await invoke("rollback_npm_snapshot", { snapshotId });
      const scanResult = await invoke<NpmReadResult>("scan_npm", {
        projectDirectory: projectDirectory.trim() || null,
      });
      setResult(scanResult);
      setSnapshotId(null);
      setScanState("idle");
    } catch (error) {
      setScanState("error");
      setErrorMessage(String(error));
    }
  }

  async function checkHealth() {
    beginOperation("npm");
    try {
      const result = await invoke<HealthCheckResult>("check_npm_health", {
        address: healthTarget.trim(),
      });
      setHealthResult(result);
      setScanState("idle");
    } catch (error) {
      setScanState("error");
      setErrorMessage(String(error));
    }
  }

  async function scanActiveTool() {
    switch (activeTool) {
      case "npm":
        await scan();
        break;
      case "maven":
        await scanMaven();
        break;
      case "flutter-pub":
        await scanFlutterPub();
        break;
      case "go":
        await scanGo();
        break;
      case "cargo":
        await scanCargo();
        break;
      case "docker":
        await scanDocker();
        break;
      case "pnpm":
        await scanPnpm();
        break;
      case "yarn":
        await scanYarn();
        break;
      case "pip":
        await scanPip();
        break;
    }
  }

  return (
    <ThemeProvider>
      <div className="app-shell">
        <header className="app-titlebar" data-tauri-drag-region>
          <div className="app-identity" data-tauri-drag-region>
            <div
              aria-hidden="true"
              className="app-mark"
            >
              M
            </div>
            <div>
              <p className="app-name">MirrorIt</p>
              <p className="app-subtitle">本机配置中心</p>
            </div>
          </div>
          <div className="titlebar-tools">
            <span className="local-status">
              <CheckCircle2 aria-hidden="true" />
              本地模式
            </span>
            <ThemeToggle />
          </div>
        </header>

        <div className="app-workspace">
          <aside className="app-sidebar">
            <div className="sidebar-heading">
              <span>开发工具</span>
              <span>{toolNavigation.length}</span>
            </div>
            <nav aria-label="工具导航" className="tool-navigation">
              {toolNavigation.map((tool) => (
                <button
                  aria-current={activeTool === tool.id ? "page" : undefined}
                  className="tool-nav-item"
                  data-active={activeTool === tool.id}
                  data-tool={tool.id}
                  key={tool.id}
                  onClick={() => setActiveTool(tool.id)}
                  type="button"
                >
                  <span aria-hidden="true" className="tool-glyph">
                    {tool.glyph}
                  </span>
                  <span className="tool-copy">
                    <span>{tool.label}</span>
                    <span>{tool.mode}</span>
                  </span>
                  <span
                    aria-label={toolResults[tool.id] ? "已读取" : "未读取"}
                    className="tool-read-state"
                    data-ready={Boolean(toolResults[tool.id])}
                  />
                </button>
              ))}
            </nav>
            <div className="sidebar-trust">
              <ShieldCheck aria-hidden="true" />
              <p>所有写入都先预览，并自动准备恢复点。</p>
            </div>
          </aside>

          <main className="app-main">
            {activeTool === "npm" ? (
              <>
                <section
                  aria-labelledby="npm-heading"
                  className="flex flex-wrap items-end justify-between gap-5 border-b border-border pb-7"
                >
                  <div>
                    <p className="text-xs font-medium text-muted-foreground">
                      工具配置 / 只读扫描
                    </p>
                    <h1
                      id="npm-heading"
                      className="mt-1 flex items-center gap-2 text-xl font-semibold"
                    >
                      npm 配置来源
                      <span className="rounded-full border border-border px-2 py-0.5 text-xs font-medium text-muted-foreground">
                        仅检测
                      </span>
                    </h1>
                    <p className="mt-2 max-w-2xl text-sm text-muted-foreground">
                      查看 registry、scope registry
                      与代理的最终生效值；扫描不会改动本机文件。
                    </p>
                  </div>
                  <Button disabled={scanState === "loading"} onClick={scan}>
                    {scanState === "loading" ? (
                      <LoaderCircle
                        aria-hidden="true"
                        className="animate-spin"
                      />
                    ) : (
                      <RefreshCw aria-hidden="true" />
                    )}
                    扫描配置
                  </Button>
                </section>

                <section
                  aria-labelledby="health-heading"
                  className="border-b border-border py-6"
                >
                  <div className="flex flex-wrap items-end justify-between gap-4">
                    <div>
                      <p className="text-xs font-medium text-muted-foreground">
                        显式检查
                      </p>
                      <h2
                        id="health-heading"
                        className="mt-1 text-base font-semibold"
                      >
                        npm 源连通性
                      </h2>
                    </div>
                    <Button
                      disabled={scanState === "loading" || !healthTarget.trim()}
                      onClick={checkHealth}
                      variant="outline"
                    >
                      <Activity aria-hidden="true" />
                      检查连接
                    </Button>
                  </div>
                  <div className="mt-4 grid gap-3 lg:grid-cols-[1fr_auto]">
                    <input
                      aria-label="检查地址"
                      className="h-9 rounded-md border border-input bg-card px-3 text-sm outline-none placeholder:text-muted-foreground focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/30"
                      onChange={(event) => setHealthTarget(event.target.value)}
                      placeholder="https://registry.npmjs.org/"
                      value={healthTarget}
                    />
                    {healthResult ? (
                      <p className="self-center text-sm text-muted-foreground">
                        {healthResult.status === "healthy"
                          ? "连接正常"
                          : healthResult.status.replace(/_/g, " ")}{" "}
                        · {healthResult.elapsed_ms} ms
                      </p>
                    ) : null}
                  </div>
                </section>

                <section
                  aria-label="扫描范围"
                  className="grid gap-3 border-b border-border py-5 lg:grid-cols-[1fr_auto]"
                >
                  <label className="grid gap-1.5 text-sm font-medium">
                    <span>项目目录（可选）</span>
                    <div className="relative">
                      <FolderSearch
                        aria-hidden="true"
                        className="pointer-events-none absolute top-1/2 left-2.5 size-4 -translate-y-1/2 text-muted-foreground"
                      />
                      <input
                        className="h-9 w-full rounded-md border border-input bg-card py-1 pr-3 pl-8 text-sm outline-none placeholder:text-muted-foreground focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/30"
                        onChange={(event) =>
                          setProjectDirectory(event.target.value)
                        }
                        placeholder="例如 C:\work\my-project"
                        value={projectDirectory}
                      />
                    </div>
                  </label>
                  <p className="self-end pb-2 text-xs text-muted-foreground">
                    未填写时只读取用户级和环境变量配置
                  </p>
                </section>

                <section
                  aria-labelledby="profile-heading"
                  className="border-b border-border py-6"
                >
                  <div className="flex flex-wrap items-end justify-between gap-4">
                    <div>
                      <p className="text-xs font-medium text-muted-foreground">
                        下一步
                      </p>
                      <h2
                        id="profile-heading"
                        className="mt-1 text-base font-semibold"
                      >
                        预览配置档
                      </h2>
                    </div>
                    <div className="flex flex-wrap gap-2">
                      <Button
                        disabled={scanState === "loading"}
                        onClick={exportNpmProfile}
                        variant="outline"
                      >
                        <Download aria-hidden="true" />
                        导出 JSON
                      </Button>
                      <Button
                        disabled={scanState === "loading"}
                        onClick={previewProfile}
                        variant="outline"
                      >
                        <FileDiff aria-hidden="true" />
                        查看差异
                      </Button>
                    </div>
                  </div>

                  <div className="mt-4 grid gap-3 lg:grid-cols-3">
                    {(
                      Object.keys(profileDefinitions) as Array<
                        Exclude<ProfileKind, "custom">
                      >
                    ).map((kind) => {
                      const profile = profileDefinitions[kind];
                      const selected = profileKind === kind;
                      return (
                        <button
                          aria-pressed={selected}
                          className={
                            selected
                              ? "min-h-24 border border-primary bg-primary/5 p-3 text-left outline-none focus-visible:ring-3 focus-visible:ring-ring/30"
                              : "min-h-24 border border-border bg-card p-3 text-left outline-none transition-colors hover:bg-muted focus-visible:ring-3 focus-visible:ring-ring/30"
                          }
                          key={kind}
                          onClick={() => setProfileKind(kind)}
                          type="button"
                        >
                          <p className="text-sm font-medium">{profile.name}</p>
                          <code className="mt-2 block truncate font-mono text-xs text-muted-foreground">
                            {profile.registry}
                          </code>
                          <p className="mt-2 text-xs leading-4 text-muted-foreground">
                            {profile.description}
                          </p>
                        </button>
                      );
                    })}
                    <button
                      aria-pressed={profileKind === "custom"}
                      className={
                        profileKind === "custom"
                          ? "min-h-24 border border-primary bg-primary/5 p-3 text-left outline-none focus-visible:ring-3 focus-visible:ring-ring/30"
                          : "min-h-24 border border-border bg-card p-3 text-left outline-none transition-colors hover:bg-muted focus-visible:ring-3 focus-visible:ring-ring/30"
                      }
                      onClick={() => setProfileKind("custom")}
                      type="button"
                    >
                      <p className="text-sm font-medium">自定义源</p>
                      <p className="mt-2 text-xs leading-4 text-muted-foreground">
                        使用未含凭据的 HTTPS registry 地址。
                      </p>
                    </button>
                  </div>

                  <div className="mt-4 grid gap-3 lg:grid-cols-2">
                    {profileKind === "custom" ? (
                      <label className="grid gap-1.5 text-sm font-medium">
                        <span>自定义 registry</span>
                        <input
                          className="h-9 rounded-md border border-input bg-card px-3 text-sm outline-none placeholder:text-muted-foreground focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/30"
                          onChange={(event) =>
                            setCustomRegistry(event.target.value)
                          }
                          placeholder="https://registry.example.com/"
                          value={customRegistry}
                        />
                      </label>
                    ) : (
                      <div className="grid content-end text-sm">
                        <p className="text-muted-foreground">
                          预览只生成差异，不会修改任何配置文件。
                        </p>
                      </div>
                    )}
                    <label className="grid gap-1.5 text-sm font-medium">
                      <span>目标作用域</span>
                      <select
                        className="h-9 rounded-md border border-input bg-card px-3 text-sm outline-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/30"
                        onChange={(event) =>
                          setTargetScope(event.target.value as TargetScope)
                        }
                        value={targetScope}
                      >
                        <option value="user">用户级 .npmrc</option>
                        <option value="project">项目级 .npmrc</option>
                      </select>
                    </label>
                  </div>

                  {exportedProfileName ? (
                    <p className="mt-3 text-xs text-muted-foreground">
                      已导出 {exportedProfileName} 的非敏感 JSON 配置档。
                    </p>
                  ) : null}

                  <div className="mt-5 border-t border-border pt-5">
                    <div className="flex flex-wrap items-end justify-between gap-3">
                      <div>
                        <p className="text-xs font-medium text-muted-foreground">
                          导入预览
                        </p>
                        <p className="mt-1 text-sm text-muted-foreground">
                          仅比较所选 JSON 配置档，不会应用任何更改。
                        </p>
                      </div>
                      <Button
                        disabled={!importContent || scanState === "loading"}
                        onClick={previewImportedProfile}
                        variant="outline"
                      >
                        <FileDiff aria-hidden="true" />
                        预览导入
                      </Button>
                    </div>
                    <label className="mt-3 grid gap-1.5 text-sm font-medium">
                      <span>导入配置档</span>
                      <input
                        accept=".json,application/json"
                        className="block w-full text-sm text-muted-foreground file:mr-3 file:h-9 file:rounded-md file:border-0 file:bg-muted file:px-3 file:text-sm file:font-medium file:text-foreground hover:file:bg-muted/80"
                        onChange={(event) =>
                          void selectImportFile(event.target.files?.[0])
                        }
                        type="file"
                      />
                    </label>
                    {importFileName ? (
                      <p className="mt-2 text-xs text-muted-foreground">
                        已选择 {importFileName}，尚未应用。
                      </p>
                    ) : null}
                    {importPreview ? (
                      <div className="mt-3 grid gap-2 border-l-2 border-warning bg-warning/5 px-4 py-3 text-sm">
                        <p className="font-medium">
                          {importPreview.name}{" "}
                          {importPreview.changed
                            ? "将替换当前 registry"
                            : "与当前 registry 相同"}
                        </p>
                        <code className="truncate font-mono text-xs text-muted-foreground">
                          当前：{importPreview.current_registry}
                        </code>
                        <code className="truncate font-mono text-xs text-muted-foreground">
                          导入：{importPreview.imported_registry}
                        </code>
                        <p className="text-xs text-muted-foreground">
                          此预览未生成写入计划，未修改任何配置。
                        </p>
                      </div>
                    ) : null}
                  </div>
                </section>

                {!result && scanState !== "loading" ? (
                  <section className="grid min-h-64 place-items-center border-b border-border py-10 text-center">
                    <div>
                      <PackageSearch
                        aria-hidden="true"
                        className="mx-auto size-6 text-muted-foreground"
                      />
                      <p className="mt-3 text-sm font-medium">
                        尚未扫描 npm 配置
                      </p>
                      <p className="mt-1 text-sm text-muted-foreground">
                        结果会按实际优先级保留来源轨迹。
                      </p>
                    </div>
                  </section>
                ) : null}

                {result ? (
                  <section aria-labelledby="effective-heading" className="pt-7">
                    <div className="flex items-baseline justify-between">
                      <div>
                        <p className="text-xs font-medium text-muted-foreground">
                          扫描结果
                        </p>
                        <h2
                          id="effective-heading"
                          className="mt-1 text-base font-semibold"
                        >
                          生效配置
                        </h2>
                      </div>
                      <span className="text-xs text-muted-foreground">
                        {configEntries.length} 项已识别
                      </span>
                    </div>

                    {configEntries.length ? (
                      <div className="mt-4 divide-y divide-border border-y border-border">
                        {configEntries.map(([key, value]) => (
                          <article
                            className="grid gap-4 py-4 lg:grid-cols-[10rem_minmax(0,1fr)]"
                            key={key}
                          >
                            <div>
                              <p className="font-mono text-sm font-medium">
                                {key}
                              </p>
                              <p className="mt-1 text-xs text-muted-foreground">
                                最终生效值
                              </p>
                            </div>
                            <div className="min-w-0">
                              <code className="block truncate rounded-md bg-muted px-2.5 py-2 font-mono text-xs text-foreground">
                                {value.value ?? "未设置"}
                              </code>
                              <div className="mt-4 flex items-center gap-2">
                                <span className="h-px w-5 bg-border" />
                                <p className="text-xs font-medium text-muted-foreground">
                                  来源轨迹
                                </p>
                              </div>
                              <ol className="mt-3 grid gap-2">
                                {value.sources.map((source, index) => (
                                  <li
                                    className="grid gap-2 border-l-2 border-border py-1.5 pl-3 text-xs"
                                    key={`${source.location}-${source.priority}-${index}`}
                                  >
                                    <div className="flex flex-wrap items-center gap-2">
                                      <span className="font-medium">
                                        {scopeLabels[source.scope]} ·{" "}
                                        {source.location}
                                      </span>
                                      <span
                                        className={
                                          index === value.sources.length - 1
                                            ? "rounded-full bg-primary/10 px-1.5 py-0.5 font-medium text-primary"
                                            : "rounded-full bg-muted px-1.5 py-0.5 text-muted-foreground"
                                        }
                                      >
                                        {index === value.sources.length - 1
                                          ? "最终生效"
                                          : "被后续来源覆盖"}
                                      </span>
                                      {source.sensitive ? (
                                        <span className="inline-flex items-center gap-1 text-warning">
                                          <ShieldCheck
                                            aria-hidden="true"
                                            className="size-3"
                                          />
                                          凭据已掩盖
                                        </span>
                                      ) : null}
                                    </div>
                                    <code className="block truncate font-mono text-muted-foreground">
                                      {source.value ?? "未设置"}
                                    </code>
                                  </li>
                                ))}
                              </ol>
                            </div>
                          </article>
                        ))}
                      </div>
                    ) : (
                      <div className="mt-4 border-y border-border py-8 text-sm text-muted-foreground">
                        未发现受支持的 npm 配置项。
                      </div>
                    )}

                    {result.diagnostics.length ? (
                      <section
                        aria-labelledby="diagnostic-heading"
                        className="mt-6 border-l-2 border-warning bg-warning/5 px-4 py-3"
                      >
                        <h3
                          id="diagnostic-heading"
                          className="text-sm font-medium"
                        >
                          需要注意
                        </h3>
                        <ul className="mt-2 grid gap-1 text-sm text-muted-foreground">
                          {result.diagnostics.map((diagnostic) => (
                            <li key={diagnostic}>{diagnostic}</li>
                          ))}
                        </ul>
                      </section>
                    ) : null}
                  </section>
                ) : null}

                {plan ? (
                  <section aria-labelledby="plan-heading" className="mt-8">
                    <div className="flex flex-wrap items-end justify-between gap-3">
                      <div>
                        <p className="text-xs font-medium text-muted-foreground">
                          只读预览
                        </p>
                        <h2
                          id="plan-heading"
                          className="mt-1 text-base font-semibold"
                        >
                          将要发生的变更
                        </h2>
                      </div>
                      <div className="flex items-center gap-3">
                        <span className="text-xs text-muted-foreground">
                          {plan.changes.length} 项字段
                        </span>
                        <Button
                          disabled={scanState === "loading"}
                          onClick={applyPreview}
                        >
                          <Save aria-hidden="true" />
                          确认应用
                        </Button>
                      </div>
                    </div>
                    <div className="mt-4 divide-y divide-border border-y border-border">
                      {plan.changes.map((change) => (
                        <article
                          className="py-4"
                          key={`${change.file}-${change.field}`}
                        >
                          <div className="flex flex-wrap items-center justify-between gap-2">
                            <p className="font-mono text-sm font-medium">
                              {change.field}
                            </p>
                            <p className="truncate font-mono text-xs text-muted-foreground">
                              {change.file}
                            </p>
                          </div>
                          <div className="mt-3 grid gap-2 text-xs md:grid-cols-2">
                            <div className="rounded-md bg-muted px-2.5 py-2">
                              <p className="mb-1 text-muted-foreground">
                                当前值
                              </p>
                              <code className="block truncate font-mono">
                                {change.previous_value ?? "未设置"}
                              </code>
                            </div>
                            <div className="rounded-md bg-primary/5 px-2.5 py-2">
                              <p className="mb-1 text-primary">新值</p>
                              <code className="block truncate font-mono">
                                {change.next_value ?? "移除"}
                              </code>
                            </div>
                          </div>
                          {change.risk ? (
                            <p className="mt-3 border-l-2 border-warning pl-2 text-xs text-warning">
                              {change.risk}
                            </p>
                          ) : null}
                        </article>
                      ))}
                    </div>
                  </section>
                ) : null}

                {snapshotId ? (
                  <section className="mt-6 flex flex-wrap items-center justify-between gap-3 border-l-2 border-primary bg-primary/5 px-4 py-3">
                    <div>
                      <p className="text-sm font-medium">已创建可恢复快照</p>
                      <p className="mt-1 font-mono text-xs text-muted-foreground">
                        {snapshotId}
                      </p>
                    </div>
                    <Button
                      disabled={scanState === "loading"}
                      onClick={rollbackSnapshot}
                      variant="outline"
                    >
                      <RotateCcw aria-hidden="true" />
                      从快照恢复
                    </Button>
                  </section>
                ) : null}
              </>
            ) : null}

            {activeTool === "maven" ? (
              <section
                id="maven"
                aria-labelledby="maven-heading"
                className="pb-8"
              >
                <div className="flex flex-wrap items-end justify-between gap-4">
                  <div>
                    <p className="text-xs font-medium text-muted-foreground">
                      工具配置 / 只读扫描
                    </p>
                    <h2
                      id="maven-heading"
                      className="mt-1 text-xl font-semibold"
                    >
                      Maven settings.xml
                    </h2>
                    <p className="mt-2 text-sm text-muted-foreground">
                      读取全局和用户级 mirrors、活动代理与 profile
                      仓库；仅可预览并更新用户级已有镜像的 URL。
                    </p>
                  </div>
                  <Button
                    disabled={scanState === "loading"}
                    onClick={scanMaven}
                    variant="outline"
                  >
                    <RefreshCw aria-hidden="true" />
                    扫描 Maven
                  </Button>
                </div>
                {mavenResult ? (
                  <>
                    <div className="mt-5 divide-y divide-border border-y border-border">
                      {Object.entries(mavenResult.effective_config.values).map(
                        ([key, value]) => (
                          <article
                            className="grid gap-3 py-4 md:grid-cols-[14rem_minmax(0,1fr)]"
                            key={key}
                          >
                            <p className="font-mono text-sm font-medium">
                              {key}
                            </p>
                            <div className="min-w-0">
                              <code className="block truncate rounded-md bg-muted px-2.5 py-2 font-mono text-xs">
                                {value.value ?? "未设置"}
                              </code>
                              <ol className="mt-2 flex flex-wrap gap-1.5">
                                {value.sources.map((source, index) => (
                                  <li
                                    className="rounded-full border border-border px-2 py-1 text-xs text-muted-foreground"
                                    key={`${source.location}-${index}`}
                                  >
                                    {scopeLabels[source.scope]} ·{" "}
                                    {source.location}
                                    {index === value.sources.length - 1
                                      ? " · 最终生效"
                                      : ""}
                                  </li>
                                ))}
                              </ol>
                            </div>
                          </article>
                        ),
                      )}
                      {!Object.keys(mavenResult.effective_config.values)
                        .length ? (
                        <p className="py-6 text-sm text-muted-foreground">
                          未发现受支持的 Maven 配置项。
                        </p>
                      ) : null}
                    </div>

                    {mavenMirrorEntries.length ? (
                      <section
                        aria-labelledby="maven-edit-heading"
                        className="border-b border-border py-6"
                      >
                        <div className="flex flex-wrap items-end justify-between gap-4">
                          <div>
                            <p className="text-xs font-medium text-muted-foreground">
                              安全变更
                            </p>
                            <h3
                              id="maven-edit-heading"
                              className="mt-1 text-base font-semibold"
                            >
                              更新镜像 URL
                            </h3>
                          </div>
                          <Button
                            disabled={
                              scanState === "loading" ||
                              !mavenMirrorId ||
                              !mavenMirrorUrl.trim()
                            }
                            onClick={previewMavenMirror}
                            variant="outline"
                          >
                            <FileDiff aria-hidden="true" />
                            查看差异
                          </Button>
                        </div>
                        <div className="mt-4 grid gap-3 lg:grid-cols-2">
                          <label className="grid gap-1.5 text-sm font-medium">
                            <span>目标镜像</span>
                            <select
                              className="h-9 rounded-md border border-input bg-card px-3 text-sm outline-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/30"
                              onChange={(event) =>
                                setMavenMirrorId(event.target.value)
                              }
                              value={mavenMirrorId}
                            >
                              {mavenMirrorEntries.map(([key, value]) => (
                                <option
                                  key={key}
                                  value={key.slice("mirror.".length)}
                                >
                                  {key.slice("mirror.".length)} ·{" "}
                                  {value.value ?? "未设置"}
                                </option>
                              ))}
                            </select>
                          </label>
                          <label className="grid gap-1.5 text-sm font-medium">
                            <span>新镜像 URL</span>
                            <input
                              className="h-9 rounded-md border border-input bg-card px-3 text-sm outline-none placeholder:text-muted-foreground focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/30"
                              onChange={(event) =>
                                setMavenMirrorUrl(event.target.value)
                              }
                              placeholder="https://repo.example.com/maven/"
                              value={mavenMirrorUrl}
                            />
                          </label>
                        </div>
                        <p className="mt-3 text-xs text-muted-foreground">
                          只接受未含凭据的 HTTPS 地址。预览会复核 XML
                          结构和文件校验和，不会写入文件。
                        </p>
                      </section>
                    ) : null}

                    {mavenPlan ? (
                      <section
                        aria-labelledby="maven-plan-heading"
                        className="border-b border-border py-6"
                      >
                        <div className="flex flex-wrap items-end justify-between gap-3">
                          <div>
                            <p className="text-xs font-medium text-muted-foreground">
                              只读预览
                            </p>
                            <h3
                              id="maven-plan-heading"
                              className="mt-1 text-base font-semibold"
                            >
                              将要发生的变更
                            </h3>
                          </div>
                          <Button
                            disabled={scanState === "loading"}
                            onClick={applyMavenPreview}
                          >
                            <Save aria-hidden="true" />
                            确认应用
                          </Button>
                        </div>
                        <div className="mt-4 divide-y divide-border border-y border-border">
                          {mavenPlan.changes.map((change) => (
                            <article
                              className="py-4"
                              key={`${change.file}-${change.field}`}
                            >
                              <div className="flex flex-wrap items-center justify-between gap-2">
                                <p className="font-mono text-sm font-medium">
                                  {change.field}
                                </p>
                                <p className="truncate font-mono text-xs text-muted-foreground">
                                  {change.file}
                                </p>
                              </div>
                              <div className="mt-3 grid gap-2 text-xs md:grid-cols-2">
                                <div className="rounded-md bg-muted px-2.5 py-2">
                                  <p className="mb-1 text-muted-foreground">
                                    当前值
                                  </p>
                                  <code className="block truncate font-mono">
                                    {change.previous_value ?? "未设置"}
                                  </code>
                                </div>
                                <div className="rounded-md bg-primary/5 px-2.5 py-2">
                                  <p className="mb-1 text-primary">新值</p>
                                  <code className="block truncate font-mono">
                                    {change.next_value ?? "移除"}
                                  </code>
                                </div>
                              </div>
                            </article>
                          ))}
                        </div>
                      </section>
                    ) : null}

                    {mavenSnapshotId ? (
                      <section className="mt-6 flex flex-wrap items-center justify-between gap-3 border-l-2 border-primary bg-primary/5 px-4 py-3">
                        <div>
                          <p className="text-sm font-medium">
                            已创建 Maven 可恢复快照
                          </p>
                          <p className="mt-1 font-mono text-xs text-muted-foreground">
                            {mavenSnapshotId}
                          </p>
                        </div>
                        <Button
                          disabled={scanState === "loading"}
                          onClick={rollbackMavenSnapshot}
                          variant="outline"
                        >
                          <RotateCcw aria-hidden="true" />
                          从快照恢复
                        </Button>
                      </section>
                    ) : null}
                  </>
                ) : null}
              </section>
            ) : null}

            {activeTool === "go" ? (
              <section id="go" aria-labelledby="go-heading" className="pb-8">
                <div className="flex flex-wrap items-end justify-between gap-4">
                  <div>
                    <p className="text-xs font-medium text-muted-foreground">
                      工具配置 / 只读扫描
                    </p>
                    <h2 id="go-heading" className="mt-1 text-xl font-semibold">
                      Go 模块环境
                    </h2>
                    <p className="mt-2 text-sm text-muted-foreground">
                      查看模块代理、校验数据库与私有模块规则的有效值和来源轨迹。
                    </p>
                  </div>
                  <Button
                    disabled={scanState === "loading"}
                    onClick={scanGo}
                    variant="outline"
                  >
                    <RefreshCw aria-hidden="true" />
                    扫描 Go
                  </Button>
                </div>

                {goResult ? (
                  <>
                    <div className="mt-5 divide-y divide-border border-y border-border">
                      {Object.entries(goResult.effective_config.values).map(
                        ([key, value]) => (
                          <article
                            className="grid gap-3 py-4 md:grid-cols-[14rem_minmax(0,1fr)]"
                            key={key}
                          >
                            <p className="font-mono text-sm font-medium">
                              {key}
                            </p>
                            <div className="min-w-0">
                              <code className="block truncate rounded-md bg-muted px-2.5 py-2 font-mono text-xs">
                                {value.value || "未设置"}
                              </code>
                              <ol className="mt-2 flex flex-wrap gap-1.5">
                                {value.sources.map((source, index) => (
                                  <li
                                    className="rounded-full border border-border px-2 py-1 text-xs text-muted-foreground"
                                    key={`${source.location}-${index}`}
                                  >
                                    {scopeLabels[source.scope]} ·{" "}
                                    {source.location}
                                    {index === value.sources.length - 1
                                      ? " · 最终生效"
                                      : ""}
                                    {source.sensitive ? " · 凭据已掩盖" : ""}
                                  </li>
                                ))}
                              </ol>
                            </div>
                          </article>
                        ),
                      )}
                      {!Object.keys(goResult.effective_config.values).length ? (
                        <p className="py-6 text-sm text-muted-foreground">
                          未发现受支持的 Go 环境配置项。
                        </p>
                      ) : null}
                    </div>

                    {goResult.diagnostics.length ? (
                      <section className="mt-5 border-l-2 border-warning bg-warning/5 px-4 py-3">
                        <h3 className="text-sm font-medium">需要注意</h3>
                        <ul className="mt-2 grid gap-1 text-sm text-muted-foreground">
                          {goResult.diagnostics.map((diagnostic) => (
                            <li key={diagnostic}>{diagnostic}</li>
                          ))}
                        </ul>
                      </section>
                    ) : null}
                  </>
                ) : null}

                <p className="mt-5 border-l-2 border-border px-4 py-3 text-xs text-muted-foreground">
                  此阶段不会修改 GOENV、环境变量、模块缓存或项目文件。
                </p>
              </section>
            ) : null}

            {activeTool === "cargo" ? (
              <section
                id="cargo"
                aria-labelledby="cargo-heading"
                className="pb-8"
              >
                <div className="flex flex-wrap items-end justify-between gap-4">
                  <div>
                    <p className="text-xs font-medium text-muted-foreground">
                      工具配置 / 只读扫描
                    </p>
                    <h2
                      id="cargo-heading"
                      className="mt-1 text-xl font-semibold"
                    >
                      Cargo registry
                    </h2>
                    <p className="mt-2 text-sm text-muted-foreground">
                      查看 crates.io 替换、命名 registry、默认 registry 与 HTTP
                      代理的来源轨迹。
                    </p>
                  </div>
                  <Button
                    disabled={scanState === "loading"}
                    onClick={scanCargo}
                    variant="outline"
                  >
                    <RefreshCw aria-hidden="true" />
                    扫描 Cargo
                  </Button>
                </div>

                {cargoResult ? (
                  <>
                    <div className="mt-5 divide-y divide-border border-y border-border">
                      {Object.entries(cargoResult.effective_config.values).map(
                        ([key, value]) => (
                          <article
                            className="grid gap-3 py-4 md:grid-cols-[14rem_minmax(0,1fr)]"
                            key={key}
                          >
                            <p className="font-mono text-sm font-medium">
                              {key}
                            </p>
                            <div className="min-w-0">
                              <code className="block truncate rounded-md bg-muted px-2.5 py-2 font-mono text-xs">
                                {value.value || "未设置"}
                              </code>
                              <ol className="mt-2 flex flex-wrap gap-1.5">
                                {value.sources.map((source, index) => (
                                  <li
                                    className="rounded-full border border-border px-2 py-1 text-xs text-muted-foreground"
                                    key={`${source.location}-${index}`}
                                  >
                                    {scopeLabels[source.scope]} ·{" "}
                                    {source.location}
                                    {index === value.sources.length - 1
                                      ? " · 最终生效"
                                      : ""}
                                    {source.sensitive ? " · 凭据已掩盖" : ""}
                                  </li>
                                ))}
                              </ol>
                            </div>
                          </article>
                        ),
                      )}
                      {!Object.keys(cargoResult.effective_config.values)
                        .length ? (
                        <p className="py-6 text-sm text-muted-foreground">
                          未发现受支持的 Cargo 配置项。
                        </p>
                      ) : null}
                    </div>

                    {cargoResult.diagnostics.length ? (
                      <section className="mt-5 border-l-2 border-warning bg-warning/5 px-4 py-3">
                        <h3 className="text-sm font-medium">需要注意</h3>
                        <ul className="mt-2 grid gap-1 text-sm text-muted-foreground">
                          {cargoResult.diagnostics.map((diagnostic) => (
                            <li key={diagnostic}>{diagnostic}</li>
                          ))}
                        </ul>
                      </section>
                    ) : null}
                  </>
                ) : null}

                <p className="mt-5 border-l-2 border-border px-4 py-3 text-xs text-muted-foreground">
                  只读取用户级与明确选择项目的配置；不会读取
                  token、凭据文件、缓存或 Cargo.lock。
                </p>
              </section>
            ) : null}

            {activeTool === "docker" ? (
              <section
                id="docker"
                aria-labelledby="docker-heading"
                className="pb-8"
              >
                <div className="flex flex-wrap items-end justify-between gap-4">
                  <div>
                    <p className="text-xs font-medium text-muted-foreground">
                      工具配置 / 只读扫描
                    </p>
                    <h2
                      id="docker-heading"
                      className="mt-1 text-xl font-semibold"
                    >
                      Docker 镜像与代理
                    </h2>
                    <p className="mt-2 text-sm text-muted-foreground">
                      查看 CLI 代理与 Linux、Windows 守护进程的 registry mirror
                      来源。
                    </p>
                  </div>
                  <Button
                    disabled={scanState === "loading"}
                    onClick={scanDocker}
                    variant="outline"
                  >
                    <RefreshCw aria-hidden="true" />
                    扫描 Docker
                  </Button>
                </div>

                {dockerResult ? (
                  <>
                    <div className="mt-5 divide-y divide-border border-y border-border">
                      {Object.entries(dockerResult.effective_config.values).map(
                        ([key, value]) => (
                          <article
                            className="grid gap-3 py-4 md:grid-cols-[14rem_minmax(0,1fr)]"
                            key={key}
                          >
                            <p className="font-mono text-sm font-medium">
                              {key}
                            </p>
                            <div className="min-w-0">
                              <code className="block truncate rounded-md bg-muted px-2.5 py-2 font-mono text-xs">
                                {value.value || "未设置"}
                              </code>
                              <ol className="mt-2 flex flex-wrap gap-1.5">
                                {value.sources.map((source, index) => (
                                  <li
                                    className="rounded-full border border-border px-2 py-1 text-xs text-muted-foreground"
                                    key={`${source.location}-${index}`}
                                  >
                                    {scopeLabels[source.scope]} ·{" "}
                                    {source.location}
                                    {index === value.sources.length - 1
                                      ? " · 最终生效"
                                      : ""}
                                    {source.sensitive ? " · 凭据已掩盖" : ""}
                                  </li>
                                ))}
                              </ol>
                            </div>
                          </article>
                        ),
                      )}
                      {!Object.keys(dockerResult.effective_config.values)
                        .length ? (
                        <p className="py-6 text-sm text-muted-foreground">
                          未发现受支持的 Docker 配置项。
                        </p>
                      ) : null}
                    </div>

                    {dockerResult.diagnostics.length ? (
                      <section className="mt-5 border-l-2 border-warning bg-warning/5 px-4 py-3">
                        <h3 className="text-sm font-medium">需要注意</h3>
                        <ul className="mt-2 grid gap-1 text-sm text-muted-foreground">
                          {dockerResult.diagnostics.map((diagnostic) => (
                            <li key={diagnostic}>{diagnostic}</li>
                          ))}
                        </ul>
                      </section>
                    ) : null}
                  </>
                ) : null}

                <p className="mt-5 border-l-2 border-border px-4 py-3 text-xs text-muted-foreground">
                  不会读取认证信息、Docker Desktop
                  设置、镜像、容器或构建缓存；不会连接或启动 Docker 守护进程。
                </p>
              </section>
            ) : null}

            {activeTool === "pnpm" ? (
              <section
                id="pnpm"
                aria-labelledby="pnpm-heading"
                className="pb-8"
              >
                <div className="flex flex-wrap items-end justify-between gap-4">
                  <div>
                    <p className="text-xs font-medium text-muted-foreground">
                      工具配置 / 只读扫描
                    </p>
                    <h2
                      id="pnpm-heading"
                      className="mt-1 text-xl font-semibold"
                    >
                      pnpm registry 与代理
                    </h2>
                    <p className="mt-2 text-sm text-muted-foreground">
                      查看全局、用户、明确选择项目与环境变量中的 registry、scope
                      registry 和代理来源。
                    </p>
                  </div>
                  <Button
                    disabled={scanState === "loading"}
                    onClick={scanPnpm}
                    variant="outline"
                  >
                    <RefreshCw aria-hidden="true" />
                    扫描 pnpm
                  </Button>
                </div>

                {pnpmResult ? (
                  <>
                    <div className="mt-5 divide-y divide-border border-y border-border">
                      {Object.entries(pnpmResult.effective_config.values).map(
                        ([key, value]) => (
                          <article
                            className="grid gap-3 py-4 md:grid-cols-[14rem_minmax(0,1fr)]"
                            key={key}
                          >
                            <p className="font-mono text-sm font-medium">
                              {key}
                            </p>
                            <div className="min-w-0">
                              <code className="block truncate rounded-md bg-muted px-2.5 py-2 font-mono text-xs">
                                {value.value || "未设置"}
                              </code>
                              <ol className="mt-2 flex flex-wrap gap-1.5">
                                {value.sources.map((source, index) => (
                                  <li
                                    className="rounded-full border border-border px-2 py-1 text-xs text-muted-foreground"
                                    key={`${source.location}-${index}`}
                                  >
                                    {scopeLabels[source.scope]} ·{" "}
                                    {source.location}
                                    {index === value.sources.length - 1
                                      ? " · 最终生效"
                                      : ""}
                                    {source.sensitive ? " · 凭据已掩盖" : ""}
                                  </li>
                                ))}
                              </ol>
                            </div>
                          </article>
                        ),
                      )}
                      {!Object.keys(pnpmResult.effective_config.values)
                        .length ? (
                        <p className="py-6 text-sm text-muted-foreground">
                          未发现受支持的 pnpm 配置项。
                        </p>
                      ) : null}
                    </div>

                    {pnpmResult.diagnostics.length ? (
                      <section className="mt-5 border-l-2 border-warning bg-warning/5 px-4 py-3">
                        <h3 className="text-sm font-medium">需要注意</h3>
                        <ul className="mt-2 grid gap-1 text-sm text-muted-foreground">
                          {pnpmResult.diagnostics.map((diagnostic) => (
                            <li key={diagnostic}>{diagnostic}</li>
                          ))}
                        </ul>
                      </section>
                    ) : null}
                  </>
                ) : null}

                <p className="mt-5 border-l-2 border-border px-4 py-3 text-xs text-muted-foreground">
                  只读取允许的 .npmrc 字段与环境变量；不会读取 token、认证项、
                  lockfile、store、缓存或项目工作区文件。
                </p>
              </section>
            ) : null}

            {activeTool === "yarn" ? (
              <section
                id="yarn"
                aria-labelledby="yarn-heading"
                className="pb-8"
              >
                <div className="flex flex-wrap items-end justify-between gap-4">
                  <div>
                    <p className="text-xs font-medium text-muted-foreground">
                      工具配置 / 只读扫描
                    </p>
                    <h2
                      id="yarn-heading"
                      className="mt-1 text-xl font-semibold"
                    >
                      Yarn registry 与代理
                    </h2>
                    <p className="mt-2 text-sm text-muted-foreground">
                      按检测到的 Yarn Classic 或 Berry
                      版本查看用户、明确选择项目与环境变量来源。
                    </p>
                  </div>
                  <Button
                    disabled={scanState === "loading"}
                    onClick={scanYarn}
                    variant="outline"
                  >
                    <RefreshCw aria-hidden="true" />
                    扫描 Yarn
                  </Button>
                </div>

                {yarnResult ? (
                  <>
                    <div className="mt-5 divide-y divide-border border-y border-border">
                      {Object.entries(yarnResult.effective_config.values).map(
                        ([key, value]) => (
                          <article
                            className="grid gap-3 py-4 md:grid-cols-[14rem_minmax(0,1fr)]"
                            key={key}
                          >
                            <p className="font-mono text-sm font-medium">
                              {key}
                            </p>
                            <div className="min-w-0">
                              <code className="block truncate rounded-md bg-muted px-2.5 py-2 font-mono text-xs">
                                {value.value || "未设置"}
                              </code>
                              <ol className="mt-2 flex flex-wrap gap-1.5">
                                {value.sources.map((source, index) => (
                                  <li
                                    className="rounded-full border border-border px-2 py-1 text-xs text-muted-foreground"
                                    key={`${source.location}-${index}`}
                                  >
                                    {scopeLabels[source.scope]} ·{" "}
                                    {source.location}
                                    {index === value.sources.length - 1
                                      ? " · 最终生效"
                                      : ""}
                                    {source.sensitive ? " · 凭据已掩盖" : ""}
                                  </li>
                                ))}
                              </ol>
                            </div>
                          </article>
                        ),
                      )}
                      {!Object.keys(yarnResult.effective_config.values)
                        .length ? (
                        <p className="py-6 text-sm text-muted-foreground">
                          未发现受支持的 Yarn 配置项。
                        </p>
                      ) : null}
                    </div>

                    {yarnResult.diagnostics.length ? (
                      <section className="mt-5 border-l-2 border-warning bg-warning/5 px-4 py-3">
                        <h3 className="text-sm font-medium">需要注意</h3>
                        <ul className="mt-2 grid gap-1 text-sm text-muted-foreground">
                          {yarnResult.diagnostics.map((diagnostic) => (
                            <li key={diagnostic}>{diagnostic}</li>
                          ))}
                        </ul>
                      </section>
                    ) : null}
                  </>
                ) : null}

                <p className="mt-5 border-l-2 border-border px-4 py-3 text-xs text-muted-foreground">
                  只读取由已检测版本确定的配置格式和允许的环境变量；不会读取认证项、
                  缓存、lockfile、工作区文件或依赖内容。
                </p>
              </section>
            ) : null}

            {activeTool === "pip" ? (
              <section id="pip" aria-labelledby="pip-heading" className="pb-8">
                <div className="flex flex-wrap items-end justify-between gap-4">
                  <div>
                    <p className="text-xs font-medium text-muted-foreground">
                      工具配置 / 只读扫描
                    </p>
                    <h2 id="pip-heading" className="mt-1 text-xl font-semibold">
                      pip index 与代理
                    </h2>
                    <p className="mt-2 text-sm text-muted-foreground">
                      查看 Windows 全局、用户、当前 Python
                      虚拟环境与环境变量中的 index 和代理来源。
                    </p>
                  </div>
                  <Button
                    disabled={scanState === "loading"}
                    onClick={scanPip}
                    variant="outline"
                  >
                    <RefreshCw aria-hidden="true" />
                    扫描 pip
                  </Button>
                </div>

                {pipResult ? (
                  <>
                    <div className="mt-5 divide-y divide-border border-y border-border">
                      {Object.entries(pipResult.effective_config.values).map(
                        ([key, value]) => (
                          <article
                            className="grid gap-3 py-4 md:grid-cols-[14rem_minmax(0,1fr)]"
                            key={key}
                          >
                            <p className="font-mono text-sm font-medium">
                              {key}
                            </p>
                            <div className="min-w-0">
                              <code className="block truncate rounded-md bg-muted px-2.5 py-2 font-mono text-xs">
                                {value.value || "未设置"}
                              </code>
                              <ol className="mt-2 flex flex-wrap gap-1.5">
                                {value.sources.map((source, index) => (
                                  <li
                                    className="rounded-full border border-border px-2 py-1 text-xs text-muted-foreground"
                                    key={`${source.location}-${index}`}
                                  >
                                    {scopeLabels[source.scope]} ·{" "}
                                    {source.location}
                                    {index === value.sources.length - 1
                                      ? " · 最终生效"
                                      : ""}
                                    {source.sensitive ? " · 凭据已掩盖" : ""}
                                  </li>
                                ))}
                              </ol>
                            </div>
                          </article>
                        ),
                      )}
                      {!Object.keys(pipResult.effective_config.values)
                        .length ? (
                        <p className="py-6 text-sm text-muted-foreground">
                          未发现受支持的 pip 配置项。
                        </p>
                      ) : null}
                    </div>

                    {pipResult.diagnostics.length ? (
                      <section className="mt-5 border-l-2 border-warning bg-warning/5 px-4 py-3">
                        <h3 className="text-sm font-medium">需要注意</h3>
                        <ul className="mt-2 grid gap-1 text-sm text-muted-foreground">
                          {pipResult.diagnostics.map((diagnostic) => (
                            <li key={diagnostic}>{diagnostic}</li>
                          ))}
                        </ul>
                      </section>
                    ) : null}
                  </>
                ) : null}

                <p className="mt-5 border-l-2 border-border px-4 py-3 text-xs text-muted-foreground">
                  只读取约定的 pip.ini 路径与允许的 PIP_*
                  环境变量；不会读取认证项、 缓存、requirements、已安装包或
                  PIP_CONFIG_FILE 指向的任意文件。
                </p>
              </section>
            ) : null}

            {activeTool === "flutter-pub" ? (
              <section
                id="flutter-pub"
                aria-labelledby="flutter-pub-heading"
                className="pb-8"
              >
                <div className="flex flex-wrap items-end justify-between gap-4">
                  <div>
                    <p className="text-xs font-medium text-muted-foreground">
                      工具配置 / 只读扫描
                    </p>
                    <h2
                      id="flutter-pub-heading"
                      className="mt-1 text-xl font-semibold"
                    >
                      Flutter/Pub
                    </h2>
                    <p className="mt-2 text-sm text-muted-foreground">
                      查看默认 hosted 源、代理与项目 pubspec.yaml 中显式的
                      hosted 依赖。
                    </p>
                  </div>
                  <Button
                    disabled={scanState === "loading"}
                    onClick={scanFlutterPub}
                    variant="outline"
                  >
                    <RefreshCw aria-hidden="true" />
                    扫描 Flutter/Pub
                  </Button>
                </div>

                {flutterPubResult ? (
                  <>
                    <div className="mt-5 divide-y divide-border border-y border-border">
                      {Object.entries(
                        flutterPubResult.effective_config.values,
                      ).map(([key, value]) => (
                        <article
                          className="grid gap-3 py-4 md:grid-cols-[14rem_minmax(0,1fr)]"
                          key={key}
                        >
                          <p className="font-mono text-sm font-medium">{key}</p>
                          <div className="min-w-0">
                            <code className="block truncate rounded-md bg-muted px-2.5 py-2 font-mono text-xs">
                              {value.value ?? "未设置"}
                            </code>
                            <ol className="mt-2 flex flex-wrap gap-1.5">
                              {value.sources.map((source, index) => (
                                <li
                                  className="rounded-full border border-border px-2 py-1 text-xs text-muted-foreground"
                                  key={`${source.location}-${index}`}
                                >
                                  {scopeLabels[source.scope]} ·{" "}
                                  {source.location}
                                  {index === value.sources.length - 1
                                    ? " · 最终生效"
                                    : ""}
                                  {source.sensitive ? " · 凭据已掩盖" : ""}
                                </li>
                              ))}
                            </ol>
                          </div>
                        </article>
                      ))}
                      {!Object.keys(flutterPubResult.effective_config.values)
                        .length ? (
                        <p className="py-6 text-sm text-muted-foreground">
                          未发现受支持的 Flutter/Pub 配置项。
                        </p>
                      ) : null}
                    </div>

                    {flutterPubResult.diagnostics.length ? (
                      <section className="mt-5 border-l-2 border-warning bg-warning/5 px-4 py-3">
                        <h3 className="text-sm font-medium">需要注意</h3>
                        <ul className="mt-2 grid gap-1 text-sm text-muted-foreground">
                          {flutterPubResult.diagnostics.map((diagnostic) => (
                            <li key={diagnostic}>{diagnostic}</li>
                          ))}
                        </ul>
                      </section>
                    ) : null}
                  </>
                ) : null}

                <section
                  aria-labelledby="flutter-pub-edit-heading"
                  className="border-b border-border py-6"
                >
                  <div className="flex flex-wrap items-end justify-between gap-4">
                    <div>
                      <p className="text-xs font-medium text-muted-foreground">
                        安全变更
                      </p>
                      <h3
                        id="flutter-pub-edit-heading"
                        className="mt-1 text-base font-semibold"
                      >
                        用户级 hosted 源
                      </h3>
                    </div>
                    <Button
                      disabled={
                        scanState === "loading" ||
                        (flutterPubProfile === "custom" &&
                          !flutterPubHostedUrl.trim())
                      }
                      onClick={previewFlutterPubHosted}
                      variant="outline"
                    >
                      <FileDiff aria-hidden="true" />
                      查看差异
                    </Button>
                  </div>
                  <div className="mt-4 grid gap-3 lg:grid-cols-2">
                    <button
                      aria-pressed={flutterPubProfile === "official"}
                      className={
                        flutterPubProfile === "official"
                          ? "border border-primary bg-primary/5 p-3 text-left outline-none focus-visible:ring-3 focus-visible:ring-ring/30"
                          : "border border-border bg-card p-3 text-left outline-none hover:bg-muted focus-visible:ring-3 focus-visible:ring-ring/30"
                      }
                      onClick={() => setFlutterPubProfile("official")}
                      type="button"
                    >
                      <p className="text-sm font-medium">官方源</p>
                      <code className="mt-2 block font-mono text-xs text-muted-foreground">
                        https://pub.dev
                      </code>
                      <p className="mt-2 text-xs text-muted-foreground">
                        移除用户级 PUB_HOSTED_URL 覆盖。
                      </p>
                    </button>
                    <button
                      aria-pressed={flutterPubProfile === "custom"}
                      className={
                        flutterPubProfile === "custom"
                          ? "border border-primary bg-primary/5 p-3 text-left outline-none focus-visible:ring-3 focus-visible:ring-ring/30"
                          : "border border-border bg-card p-3 text-left outline-none hover:bg-muted focus-visible:ring-3 focus-visible:ring-ring/30"
                      }
                      onClick={() => setFlutterPubProfile("custom")}
                      type="button"
                    >
                      <p className="text-sm font-medium">自定义源</p>
                      <p className="mt-2 text-xs text-muted-foreground">
                        设置用户级未含凭据的 HTTPS hosted 源。
                      </p>
                    </button>
                  </div>
                  {flutterPubProfile === "custom" ? (
                    <label className="mt-4 grid gap-1.5 text-sm font-medium">
                      <span>自定义 hosted URL</span>
                      <input
                        className="h-9 rounded-md border border-input bg-card px-3 text-sm outline-none placeholder:text-muted-foreground focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/30"
                        onChange={(event) =>
                          setFlutterPubHostedUrl(event.target.value)
                        }
                        placeholder="https://packages.example.com/"
                        value={flutterPubHostedUrl}
                      />
                    </label>
                  ) : null}
                  <p className="mt-3 text-xs text-muted-foreground">
                    不会修改 pubspec.yaml、锁文件或 .dart_tool。应用后需重新启动
                    Flutter、终端和 MirrorIt。
                  </p>
                </section>

                {flutterPubPlan ? (
                  <section
                    aria-labelledby="flutter-pub-plan-heading"
                    className="border-b border-border py-6"
                  >
                    <div className="flex flex-wrap items-end justify-between gap-3">
                      <div>
                        <p className="text-xs font-medium text-muted-foreground">
                          只读预览
                        </p>
                        <h3
                          id="flutter-pub-plan-heading"
                          className="mt-1 text-base font-semibold"
                        >
                          将要发生的变更
                        </h3>
                      </div>
                      <Button
                        disabled={scanState === "loading"}
                        onClick={applyFlutterPubPreview}
                      >
                        <Save aria-hidden="true" />
                        确认应用
                      </Button>
                    </div>
                    {flutterPubPlan.changes.map((change) => (
                      <article
                        className="mt-4 border-y border-border py-4"
                        key={`${change.file}-${change.field}`}
                      >
                        <p className="font-mono text-sm font-medium">
                          {change.field}
                        </p>
                        <p className="mt-1 truncate font-mono text-xs text-muted-foreground">
                          {change.file}
                        </p>
                        <div className="mt-3 grid gap-2 text-xs md:grid-cols-2">
                          <div className="rounded-md bg-muted px-2.5 py-2">
                            <p className="mb-1 text-muted-foreground">当前值</p>
                            <code className="block truncate font-mono">
                              {change.previous_value ?? "未设置"}
                            </code>
                          </div>
                          <div className="rounded-md bg-primary/5 px-2.5 py-2">
                            <p className="mb-1 text-primary">新值</p>
                            <code className="block truncate font-mono">
                              {change.next_value ?? "移除覆盖"}
                            </code>
                          </div>
                        </div>
                        {change.risk ? (
                          <p className="mt-3 border-l-2 border-warning pl-2 text-xs text-warning">
                            {change.risk}
                          </p>
                        ) : null}
                      </article>
                    ))}
                  </section>
                ) : null}

                {flutterPubSnapshotId ? (
                  <section className="mt-6 flex flex-wrap items-center justify-between gap-3 border-l-2 border-primary bg-primary/5 px-4 py-3">
                    <div>
                      <p className="text-sm font-medium">
                        已创建 Flutter/Pub 可恢复快照
                      </p>
                      <p className="mt-1 font-mono text-xs text-muted-foreground">
                        {flutterPubSnapshotId}
                      </p>
                    </div>
                    <Button
                      disabled={scanState === "loading"}
                      onClick={rollbackFlutterPubSnapshot}
                      variant="outline"
                    >
                      <RotateCcw aria-hidden="true" />
                      从快照恢复
                    </Button>
                  </section>
                ) : null}
              </section>
            ) : null}
          </main>

          <aside aria-label="当前工具检查器" className="app-inspector">
            <header className="inspector-header">
              <PanelRight aria-hidden="true" />
              <h2>检查器</h2>
            </header>

            <div className="inspector-scroll">
              <section className="inspector-section">
                <div className="inspector-tool">
                  <span
                    aria-hidden="true"
                    className="tool-glyph"
                    data-tool={activeTool}
                  >
                    {activeToolMeta.glyph}
                  </span>
                  <div>
                    <h3>{activeToolMeta.label}</h3>
                    <p>{activeToolMeta.mode}</p>
                  </div>
                </div>

                <div aria-live="polite" className="operation-status">
                  {scanState === "loading" && operationTool === activeTool ? (
                    <>
                      <LoaderCircle aria-hidden="true" className="animate-spin" />
                      <span>正在读取本机配置…</span>
                    </>
                  ) : scanState === "error" && operationTool === activeTool ? (
                    <>
                      <TriangleAlert aria-hidden="true" />
                      <span>{errorMessage}</span>
                    </>
                  ) : activeToolResult ? (
                    <>
                      <CheckCircle2 aria-hidden="true" />
                      <span>已读取 {activeToolEntryCount} 项配置</span>
                    </>
                  ) : (
                    <>
                      <Circle aria-hidden="true" />
                      <span>尚未扫描此工具</span>
                    </>
                  )}
                </div>

                <Button
                  className="w-full"
                  disabled={scanState === "loading"}
                  onClick={() => void scanActiveTool()}
                >
                  {scanState === "loading" && operationTool === activeTool ? (
                    <LoaderCircle aria-hidden="true" className="animate-spin" />
                  ) : (
                    <ScanLine aria-hidden="true" />
                  )}
                  扫描配置
                </Button>
              </section>

              <section className="inspector-section">
                <div className="inspector-section-heading">
                  <h3>来源概览</h3>
                  <span>{activeSourceCount} 条轨迹</span>
                </div>
                {activeToolEntries.length ? (
                  <ol className="source-preview">
                    {activeToolEntries.slice(0, 5).map(([key, value]) => (
                      <li key={key}>
                        <span className="source-preview-dot" />
                        <div>
                          <code>{key}</code>
                          <p title={value.value ?? "未设置"}>
                            {value.value ?? "未设置"}
                          </p>
                        </div>
                        <span>{value.sources.length}</span>
                      </li>
                    ))}
                  </ol>
                ) : (
                  <p className="inspector-empty">
                    扫描后，这里会汇总生效值及其来源数量。
                  </p>
                )}
                {activeToolEntries.length > 5 ? (
                  <p className="inspector-footnote">
                    另有 {activeToolEntries.length - 5} 项配置显示在工作区中。
                  </p>
                ) : null}
              </section>

              {activePlan ? (
                <section className="inspector-section">
                  <div className="inspector-section-heading">
                    <h3>变更预览</h3>
                    <span>{activePlan.changes.length} 项</span>
                  </div>
                  <div className="plan-summary">
                    <Layers3 aria-hidden="true" />
                    <p>
                      已生成只读差异。确认前不会写入任何配置文件。
                    </p>
                  </div>
                </section>
              ) : null}

              {activeSnapshotId ? (
                <section className="inspector-section">
                  <div className="inspector-section-heading">
                    <h3>恢复点</h3>
                    <CheckCircle2 aria-hidden="true" />
                  </div>
                  <code className="snapshot-code">{activeSnapshotId}</code>
                </section>
              ) : null}

              {activeToolResult?.diagnostics.length ? (
                <section className="inspector-section">
                  <div className="inspector-section-heading">
                    <h3>需要注意</h3>
                    <TriangleAlert aria-hidden="true" />
                  </div>
                  <ul className="diagnostic-list">
                    {activeToolResult.diagnostics.slice(0, 3).map((item) => (
                      <li key={item}>{item}</li>
                    ))}
                  </ul>
                </section>
              ) : null}

              {activeTool === "npm" && healthResult ? (
                <section className="inspector-section">
                  <div className="inspector-section-heading">
                    <h3>连接检查</h3>
                    <span>{healthResult.elapsed_ms} ms</span>
                  </div>
                  <p className="inspector-empty">
                    {healthResult.status === "healthy"
                      ? "目标地址连接正常。"
                      : healthResult.message ||
                        healthResult.status.replace(/_/g, " ")}
                  </p>
                </section>
              ) : null}
            </div>

            <footer className="inspector-safety">
              <LockKeyhole aria-hidden="true" />
              <p>凭据不会出现在导出、日志或来源预览中。</p>
            </footer>
          </aside>
        </div>
      </div>
    </ThemeProvider>
  );
}

export default App;
